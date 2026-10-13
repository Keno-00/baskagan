use crate::ui_controls::UiButton;
use dioxus::prelude::*;
use serde::Deserialize;
use serde_json::json;

#[derive(Clone, Default, Deserialize, PartialEq)]
struct State {
    revision: u64,
    popup: Option<Popup>,
    selection: Option<Rect>,
    controls: Vec<Control>,
}
#[derive(Clone, Default, Deserialize, PartialEq)]
struct Rect {
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
    width: f64,
    height: f64,
}
#[derive(Clone, Deserialize, PartialEq)]
struct Popup {
    #[serde(default)]
    idea: bool,
    kind: String,
    rect: Rect,
    items: Vec<Item>,
    #[serde(default)]
    fresh: bool,
    readonly: bool,
}
#[derive(Clone, Deserialize, PartialEq)]
struct Item {
    id: String,
    label: String,
    quote: String,
    text: String,
}
#[derive(Clone, Deserialize, PartialEq)]
struct Control {
    key: String,
    kind: String,
    rect: Rect,
    count: usize,
    text: String,
    preview: String,
}
fn command(action: &str, key: &str, value: &str) {
    let _ = document::eval(&format!(
        "window.dadoComments?.command({},{},{});",
        json!(action),
        json!(key),
        json!(value)
    ));
}
fn position(rect: &Rect) -> String {
    format!("left:{}px;top:{}px;", rect.left, rect.top)
}

#[component]
pub fn Annotations() -> Element {
    let mut state = use_signal(State::default);
    use_future(move || async move {
        let mut bridge = document::eval(
            "const send=event=>dioxus.send(event.detail);document.addEventListener('dado-annotations',send);if(window.dadoAnnotationState)dioxus.send(window.dadoAnnotationState);window.dadoComments?.publish();",
        );
        while let Ok(next) = bridge.recv::<State>().await {
            if *state.peek() != next {
                state.set(next);
            }
        }
    });
    let snapshot = state.read().clone();
    rsx! {
        style { {include_str!("annotations.css")} }
        div { id:"annotation-ui",
            for control in snapshot.controls {
                Summary {key:"{control.key}",control}
            }
            if let Some(rect)=snapshot.selection {
                UiButton {label:"Comment on selected text",icon:"comment",class:"annotation-trigger chat-selection-comment",style:format!("left:{}px;top:{}px;",(rect.right-30.0).max(8.0),(rect.top-36.0).max(8.0)),preserve_selection:true,onpress:move |_|command("select","","")}
            }
            if let Some(popup)=snapshot.popup {
                AnnotationPopup {key:"{snapshot.revision}",popup}
            }
        }
    }
}
#[component]
fn Summary(control: Control) -> Element {
    let control_key = control.key.clone();
    let clear_key = control_key.clone();
    let marker = control.kind == "marker";
    let ideas = control.kind.starts_with("ideas");
    let badge = matches!(
        control.kind.as_str(),
        "summary" | "ideas-summary" | "sent" | "ideas-sent"
    );
    let button_class: &'static str = if marker {
        "comment-marker"
    } else if badge {
        "comment-badge"
    } else {
        "comment-badge annotation-count-bubble"
    };
    let icon: &'static str = if ideas {
        "idea"
    } else if marker {
        ""
    } else {
        "comment"
    };
    let label = if ideas {
        format!("View ideas ({})", control.count)
    } else if marker {
        format!("Comment {}", control.text)
    } else {
        format!("View comments ({})", control.count)
    };
    rsx! {
        div {class:if marker {"annotation-floating annotation-marker"} else if control.kind=="sent" || control.kind=="ideas-sent" {"annotation-floating annotation-summary-control annotation-sent-summary"} else {"annotation-floating annotation-summary-control"},style:position(&control.rect),
            UiButton {label,icon,hint:if ideas {Some("An uncertain idea for something new — you don’t know what yet, but this folder feels like where it could belong.".into())}else{None},class:button_class,text:Some(if marker || badge {control.text.clone()}else{control.count.to_string()}),onpress:move |_|command("open",&control_key,"")}
            if marker {span {class:"comment-preview","{control.preview}"}}
            if control.kind=="summary" || control.kind=="ideas-summary" {
                UiButton {label:if ideas {"Remove ideas from message"}else{"Remove annotations from message"},icon:"close",class:"annotation-clear",onpress:move |_|command("clear",&clear_key,"")}
            }
        }
    }
}
#[component]
fn AnnotationPopup(popup: Popup) -> Element {
    let mut text = use_signal(|| {
        popup
            .items
            .first()
            .map(|item| item.text.clone())
            .unwrap_or_default()
    });
    let editing = popup.kind == "edit";
    let heading = if popup.idea {
        format!("{} Ideas/Things", popup.items.len())
    } else {
        format!(
            "{} annotation{}",
            popup.items.len(),
            if popup.items.len() == 1 { "" } else { "s" }
        )
    };
    let first = popup.items.first().cloned();
    let top = if editing {
        popup.rect.bottom + 8.0
    } else {
        popup.rect.top - 8.0
    };
    let width = if editing {
        380.0
    } else {
        popup.rect.width.max(300.0)
    };
    let style = if !editing && top > 180.0 {
        format!(
            "left:clamp(16px,{}px,calc(100vw - min({width}px, calc(100vw - 32px)) - 16px));top:{top}px;width:min({width}px,calc(100vw - 32px));transform:translateY(-100%);max-height:min(55vh,{}px);",
            popup.rect.left,
            top - 16.0
        )
    } else {
        format!(
            "left:clamp(16px,{}px,calc(100vw - min({width}px, calc(100vw - 32px)) - 16px));top:clamp(16px,{}px,calc(100vh - 230px));width:min({width}px,calc(100vw - 32px));",
            popup.rect.left,
            if editing {
                top
            } else {
                popup.rect.bottom + 8.0
            }
        )
    };
    rsx! {
        section {class:if editing {"annotation-popup annotation-edit"} else {"annotation-popup annotation-list"},style,role:"dialog",aria_label:if editing && popup.idea {"New Idea/Thing"}else if editing {"Comment"} else {"Comments"},
            if editing {
                if let Some(item)=first {
                    div {class:"annotation-quote",if popup.idea {crate::ui_controls::Icon {name:"idea"}} "{item.quote}"}
                    textarea {id:"annotation-input",class:"annotation-input",aria_label:if popup.idea {"Idea text"}else{"Comment text"},placeholder:if popup.idea {"Something could go here…"}else{"Leave a comment"},value:"{text}",
                        onmounted:move |_|{let _=document::eval("document.getElementById('annotation-input')?.focus();");},
                        oninput:move |event|text.set(event.value()),
                        onkeydown:move |event|{if event.key()==Key::Enter && (event.modifiers().ctrl() || event.modifiers().meta()) {event.prevent_default();command("save","",&text());}}
                    }
                    div {class:"annotation-actions",
                        if !popup.fresh {UiButton {label:"Delete",icon:"delete",class:"annotation-action",onpress:move |_|command("delete",&item.id,"")}}
                        UiButton {label:"Cancel",class:"annotation-action",text:"Cancel",onpress:move |_|command("close","","")}
                        UiButton {label:"Save",icon:"check",class:"annotation-action annotation-confirm",disabled:text.read().trim().is_empty(),onpress:move |_|command("save","",&text())}
                    }
                }
            } else {
                div {class:"annotation-list-header",span {"{heading}"} UiButton {label:"Close comments",icon:"close",class:"annotation-action",onpress:move |_|command("close","","")} }
                for (index,item) in popup.items.iter().enumerate() {
                    div {class:"annotation-entry",key:"{item.id}",
                        div {class:"annotation-quote annotation-summary","{index+1}. {item.label}"}
                        div {class:"annotation-comment","{item.text}"}
                        if !popup.readonly {
                            div {class:"annotation-entry-actions",
                                UiButton {label:"Edit comment",icon:"edit",class:"annotation-edit-icon",onpress:{let id=item.id.clone();move |_|command("edit",&id,"")}}
                                UiButton {label:"Delete comment",icon:"delete",class:"annotation-delete-icon",onpress:{let id=item.id.clone();move |_|command("delete",&id,"")}}
                            }
                        }
                    }
                }
            }
        }
    }
}
