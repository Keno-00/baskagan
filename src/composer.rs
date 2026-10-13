use crate::ui_controls::{Icon, UiButton};
use dioxus::prelude::*;
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct ComposerState {
    #[serde(default)]
    files: Vec<Chip>,
    #[serde(default)]
    skills: Vec<Chip>,
    pending_count: usize,
    #[serde(default)]
    idea_count: usize,
    disabled: bool,
    picking: bool,
    running: bool,
}
#[derive(Clone, Default, Deserialize, PartialEq)]
struct Chip {
    #[serde(default)]
    id: String,
    name: String,
    #[serde(default)]
    kind: String,
    #[serde(default)]
    path: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    scope: String,
}
#[derive(Clone, Default, Deserialize, PartialEq)]
struct MenuState {
    shown: bool,
    mode: String,
    query: String,
    active: i64,
    rect: Rect,
    rows: Vec<Row>,
    empty: String,
}
#[derive(Clone, Default, Deserialize, PartialEq)]
struct Rect {
    left: f64,
    top: f64,
    width: f64,
}
#[derive(Clone, Deserialize, PartialEq)]
struct Row {
    id: String,
    label: String,
    icon: String,
    enabled: bool,
    help: String,
}
fn command(action: &str, value: &str) {
    let _ = document::eval(&format!(
        "window.dadoChat?.composer({},{})",
        json!(action),
        json!(value)
    ));
}
#[component]
pub fn Composer() -> Element {
    let mut state = use_signal(ComposerState::default);
    let mut actions = use_signal(MenuState::default);
    let mut skills = use_signal(MenuState::default);
    use_future(move || async move {
        let mut bridge = document::eval(
            "window.dadoComposer={restore:(kind,state)=>dioxus.send({kind,state})};window.dadoChat?.composerReady();",
        );
        while let Ok(message) = bridge.recv::<Value>().await {
            match message["kind"].as_str() {
                Some("composer") => {
                    if let Ok(next) = serde_json::from_value(message["state"].clone())
                        && *state.peek() != next
                    {
                        state.set(next);
                    }
                }
                Some("actions") => {
                    if let Ok(next) = serde_json::from_value(message["state"].clone())
                        && *actions.peek() != next
                    {
                        actions.set(next);
                    }
                }
                Some("skills") => {
                    if let Ok(next) = serde_json::from_value(message["state"].clone())
                        && *skills.peek() != next
                    {
                        skills.set(next);
                    }
                }
                _ => {}
            }
        }
    });
    let snapshot = state.read().clone();
    let empty = snapshot.skills.is_empty();
    let files_empty = snapshot.files.is_empty();
    let send_icon = if snapshot.running { "stop" } else { "send" };
    rsx! {
        style { {include_str!("composer.css")} }
        form {class:"chat-composer",
            div {class:"chat-context-row",
                div {id:"pending-annotations",class:"annotation-summary-anchor",hidden:snapshot.pending_count==0,aria_hidden:true,Icon {name:"comment"} span {{format!("{} annotation{}",snapshot.pending_count,if snapshot.pending_count==1 {""}else{"s"})}}}
                div {id:"pending-ideas",class:"annotation-summary-anchor",hidden:snapshot.idea_count==0,aria_hidden:true,Icon {name:"idea"} span {"{snapshot.idea_count} Ideas/Things"}}
                div {class:"chat-attachments",hidden:empty,
                    for skill in snapshot.skills {
                        ComposerChip {key:"skill-{skill.id}",chip:skill,skill:true}
                    }
                }
            }
            div {class:"composer-attachments",hidden:files_empty,role:"list",aria_label:"Attachments",
                for file in snapshot.files {AttachmentCard {key:"file-{file.path}",file}}
            }
            // The uncontrolled input keeps native selection and undo while Rust owns its DOM.
            textarea {aria_label:"Message",placeholder:"Message",rows:1}
            div {class:"chat-actions",
                UiButton {label:"Chat actions",icon:"plus",class:"chat-attach",disabled:snapshot.picking,expanded:Some(actions.read().shown),controls:"chat-action-menu",popup:"dialog",onpress:move |_|command("actions","")}
                crate::permission_selector::PermissionSelector {}
                crate::model_selector::ModelSelector {}
                UiButton {label:if snapshot.running {"Stop response"} else {"Send message"},icon:send_icon,class:"chat-send",disabled:snapshot.disabled,onpress:move |_|command("send","")}
            }
        }
        Picker {state:actions(),skill:false}
        Picker {state:skills(),skill:true}
    }
}
fn image_preview(path: &str) -> Option<String> {
    use std::fmt::Write;
    let path = std::path::Path::new(path);
    let mime = match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "avif" => "image/avif",
        _ => return None,
    };
    // Bound preview reads; larger images retain the ordinary file card.
    use std::io::Read;
    let file = std::fs::File::open(path).ok()?;
    if file.metadata().ok()?.len() > 5 * 1024 * 1024 {
        return None;
    }
    let mut bytes = Vec::new();
    file.take(5 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() > 5 * 1024 * 1024 {
        return None;
    }
    let mut data = format!("data:{mime},");
    for byte in bytes {
        write!(data, "%{byte:02X}").ok()?;
    }
    Some(data)
}
#[component]
fn AttachmentCard(file: Chip) -> Element {
    let path = file.path.clone();
    let preview = use_resource(move || {
        let path = path.clone();
        async move {
            tokio::task::spawn_blocking(move || image_preview(&path))
                .await
                .ok()
                .flatten()
        }
    });
    let mut failed = use_signal(|| false);
    let mut status = use_context::<crate::Status>();
    let hover = file.path.clone();
    let focus = hover.clone();
    let remove = file.path.clone();
    let image = preview.read().as_ref().and_then(|src| src.clone());
    let folder = file.kind == "folder";
    rsx! {
        div {class:"attachment-card",role:"listitem",tabindex:0,aria_label:format!("{}: {}",if folder {"Folder"}else{"File"},file.name),
            onmouseenter:move |_|status.hint.set(Some(hover.clone())),onmouseleave:move |_|status.hint.set(None),
            onfocus:move |_|status.hint.set(Some(focus.clone())),onblur:move |_|status.hint.set(None),
            onkeydown:{let path=file.path.clone();move |event|if event.key()==Key::Delete {event.prevent_default();command("remove-file",&path);}},
            div {class:"attachment-preview",
                if let Some(src)=image.filter(|_|!failed()) {img {src,alt:file.name.clone(),loading:"lazy",onerror:move |_|failed.set(true)}}
                else {Icon {name:if folder {"folder"}else{"editor"}}}
            }
            div {class:"attachment-name","{file.name}"}
            UiButton {label:format!("Remove {}",file.name),icon:"close",class:"attachment-remove",preserve_selection:true,onpress:move |_|command("remove-file",&remove)}
        }
    }
}
#[component]
fn ComposerChip(chip: Chip, skill: bool) -> Element {
    let id = if skill {
        chip.id.clone()
    } else {
        chip.path.clone()
    };
    let hint = if skill {
        format!("Skill · {} · {}", chip.description, chip.scope)
    } else {
        chip.path.clone()
    };
    let mut status = use_context::<crate::Status>();
    let focus = hint.clone();
    rsx! {
        span {class:if skill {"chat-file chat-skill-chip"} else {"chat-file"},
            onmouseenter:move |_|status.hint.set(Some(hint.clone())),onmouseleave:move |_|status.hint.set(None),
            onfocusin:move |_|status.hint.set(Some(focus.clone())),onfocusout:move |_|status.hint.set(None),
            Icon {name:if skill {"skill"} else {"editor"}}
            span {"{chip.name}"}
            UiButton {label:format!("Remove {}",chip.name),icon:"close",class:"composer-chip-remove",onpress:move |_|command(if skill {"remove-skill"} else {"remove-file"},&id)}
        }
    }
}
#[component]
fn Picker(state: MenuState, skill: bool) -> Element {
    let id = if skill {
        "chat-skill-menu"
    } else {
        "chat-action-menu"
    };
    let filter_id = if skill {
        "chat-skill-search"
    } else {
        "chat-action-search"
    };
    let mut was_shown = use_signal(|| false);
    let shown = state.shown;
    let mode = state.mode.clone();
    use_effect(use_reactive((&shown, &mode), move |(shown, mode)| {
        let closing = *was_shown.peek() && !shown;
        was_shown.set(shown);
        if closing {
            let _ = document::eval(&format!(
                "if(document.activeElement===document.body || document.activeElement?.id==={}) document.querySelector('.chat-composer textarea')?.focus();",
                json!(filter_id)
            ));
        }
        if shown && mode != "slash" {
            let _ = document::eval(&format!(
                "document.getElementById({})?.focus();",
                json!(filter_id)
            ));
        }
    }));
    let label = if skill { "Skills" } else { "Chat actions" };
    let placeholder = if skill {
        "Find skills"
    } else {
        "Search actions"
    };
    let prefix = if skill { "skills-" } else { "actions-" };
    let row_prefix = if skill { "chat-skill" } else { "chat-command" };
    let mut status = use_context::<crate::Status>();
    let style = format!(
        "left:{}px;width:{}px;bottom:calc(100vh - {}px + 8px);max-height:{}px;",
        state.rect.left,
        state.rect.width,
        state.rect.top,
        (state.rect.top - 16.0).clamp(40.0, 320.0)
    );
    rsx! {
        div {id,class:if skill {"chat-action-menu chat-skill-menu"} else {"chat-action-menu"},style,hidden:!state.shown,role:"dialog",aria_label:label,
            input {id:filter_id,class:"chat-action-search",aria_label:placeholder,placeholder,value:state.query,hidden:state.mode=="slash",autocomplete:"off",aria_controls:format!("{id}-results"),aria_autocomplete:"list",aria_expanded:state.shown,
                aria_activedescendant:if state.active>=0 {Some(format!("{row_prefix}-{}",state.active))} else {None},
                oninput:move |event|command(&format!("{prefix}filter"),&event.value()),
                onmounted:move |_|{let _=document::eval("window.dadoChat?.composerReady();");}
            }
            div {id:format!("{id}-results"),role:"listbox",aria_label:if skill {"Available skills"} else {"Available actions"},
                for (index,row) in state.rows.iter().enumerate() {
                    button {id:format!("{row_prefix}-{index}"),key:"{row.id}",r#type:"button",role:"option",tabindex:-1,aria_selected:state.active==index as i64,aria_disabled:!row.enabled,
                        onmousedown:move |event|event.prevent_default(),
                        onmouseenter:{let help=row.help.clone();move |_|{status.hint.set(Some(help.clone()));command(&format!("{prefix}select"),&index.to_string());}},
                        onmouseleave:move |_|status.hint.set(None),
                        onclick:move |_|command(&format!("{prefix}choose"),&index.to_string()),
                        svg {view_box:"0 0 24 24","aria-hidden":"true",path {d:row.icon.clone()}}
                        span {"{row.label}"}
                    }
                }
                if state.rows.is_empty() {div {class:"chat-action-empty","{state.empty}"}}
            }
        }
    }
}
