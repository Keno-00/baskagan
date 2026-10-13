use crate::{Status, open_folder, ui_controls::UiButton};
use dioxus::prelude::*;
use std::path::PathBuf;

fn command(action: &str) {
    let _ = document::eval(&format!(
        "window.dadoTitlebar?.action({});",
        serde_json::json!(action)
    ));
}
fn menu(name: &str) {
    let _ = document::eval(&format!(
        "window.dadoTitlebar?.menu({});",
        serde_json::json!(name)
    ));
}
fn items(name: &str) -> &'static [(&'static str, &'static str, &'static str)] {
    match name {
        "File" => &[
            ("New Project", "", "new"),
            ("Open Folder…", "", "open"),
            ("Close Project", "", "close-project"),
            ("Exit", "Alt+F4", "quit"),
        ],
        "Edit" => &[
            ("Undo", "Ctrl+Z", "edit:undo"),
            ("Redo", "Ctrl+Y", "edit:redo"),
            ("Cut", "Ctrl+X", "edit:cut"),
            ("Copy", "Ctrl+C", "edit:copy"),
            ("Paste", "Ctrl+V", "edit:paste"),
            ("Delete", "Del", "edit:delete"),
            ("Select All", "Ctrl+A", "edit:selectAll"),
        ],
        "View" => &[
            ("Zoom In", "Ctrl+Shift+=", "zoom-in"),
            ("Zoom Out", "Ctrl+-", "zoom-out"),
            ("Actual Size", "Ctrl+0", "zoom-reset"),
            ("Toggle Sidebar", "", "sidebar-toggle"),
        ],
        "Help" => &[("About ᜊᜐ᜔ᜃᜄᜈ᜔", "", "about")],
        _ => &[],
    }
}
#[component]
pub fn TitleBar(
    mut project: Signal<Option<PathBuf>>,
    mut creating: Signal<bool>,
    mut name: Signal<String>,
    mut error: Signal<String>,
    mut sidebar: Signal<bool>,
) -> Element {
    let mut status = use_context::<Status>();
    let mut opened = use_signal(|| None::<String>);
    let mut left = use_signal(|| 0.0);
    let mut navigation = use_signal(|| (false, false));
    let mut about = use_signal(|| false);
    use_future(move || async move {
        let mut bridge = document::eval(include_str!("titlebar.js"));
        let mut zoom = 1.0_f64;
        while let Ok(message) = bridge.recv::<serde_json::Value>().await {
            let desktop = dioxus::desktop::window();
            let action = message["action"].as_str().unwrap_or_default();
            match action {
                "navigation" => navigation.set((
                    message["back"].as_bool().unwrap_or(false),
                    message["forward"].as_bool().unwrap_or(false),
                )),
                "sidebar-restore" => sidebar.set(message["shown"].as_bool().unwrap_or(false)),
                "sidebar-toggle" => {
                    if project.peek().is_some() {
                        let shown = !*sidebar.peek();
                        sidebar.set(shown);
                        let _ = document::eval(&format!(
                            "document.dispatchEvent(new CustomEvent('dado-settings-change',{{detail:{{leftSidebar:{shown}}}}}));"
                        ));
                    }
                }
                "menu" => {
                    let value = message["name"].as_str().unwrap_or_default();
                    if opened.peek().as_deref() == Some(value) {
                        opened.set(None);
                    } else {
                        left.set(message["left"].as_f64().unwrap_or_default());
                        opened.set(Some(value.into()));
                    }
                }
                "menu-close" => opened.set(None),
                "about" => about.set(true),
                "new" => {
                    name.set(String::new());
                    error.set(String::new());
                    creating.set(true);
                }
                "open" => open_folder(project, creating),
                "close-project" => {
                    project.set(None);
                    creating.set(false);
                }
                "drag" => desktop.drag(),
                "minimize" => desktop.window.set_minimized(true),
                "maximize" => desktop.toggle_maximized(),
                "quit" => desktop.close(),
                "zoom-in" | "zoom-out" | "zoom-reset" => {
                    zoom = match action {
                        "zoom-in" => (zoom * 1.1).min(2.0),
                        "zoom-out" => (zoom / 1.1).max(0.5),
                        _ => 1.0,
                    };
                    desktop.set_zoom_level(zoom);
                }
                _ => {}
            }
        }
    });
    use_effect(move || {
        if opened.read().is_some() {
            let _ = document::eval(
                "requestAnimationFrame(()=>document.querySelector('#titlebar [role=menuitem]')?.focus());",
            );
        }
    });
    rsx! {
     header {id:"titlebar",class:"title-bar",aria_label:"Window title and menus",
      UiButton {label:"Back",icon:"back",class:"title-icon",disabled:!navigation().0,onpress:move |_|command("back")}
      UiButton {label:"Forward",icon:"forward",class:"title-icon",disabled:!navigation().1,onpress:move |_|command("forward")}
      UiButton {label:"Toggle left sidebar",icon:"sidebar",pressed:sidebar(),class:"title-icon",onpress:move |_|command("sidebar-toggle")}
      for label in ["File","Edit","View","Help"] {
       button {id:format!("title-menu-{label}"),class:"title-menu-trigger",r#type:"button",aria_haspopup:"menu",aria_expanded:opened.read().as_deref()==Some(label),
        onmouseenter:move |_|status.hint.set(Some(label.into())),onmouseleave:move |_|status.hint.set(None),
        onclick:move |_|menu(label),onkeydown:move |e|{if e.key()==Key::ArrowDown{e.prevent_default();menu(label);}},"{label}"}
      }
      div {class:"title-drag",onmousedown:move |e|{if e.trigger_button()==Some(dioxus::html::input_data::MouseButton::Primary){command("drag");}},ondoubleclick:move |_|command("maximize")}
      UiButton {label:"Minimize",icon:"minimize",class:"window-control minimize",onpress:move |_|command("minimize")}
      UiButton {label:"Maximize or restore",icon:"maximize",class:"window-control maximize",onpress:move |_|command("maximize")}
      UiButton {label:"Close window",icon:"window-close",class:"window-control quit",onpress:move |_|command("quit")}
      if let Some(active)=opened.read().as_ref() {
       div {class:"title-menu",role:"menu",aria_label:active.clone(),style:format!("left:{}px;",left()),
        for &(label,shortcut,action) in items(active) {
         button {r#type:"button",role:"menuitem",aria_label:label,onmouseenter:move |_|status.hint.set(Some(label.into())),onmouseleave:move |_|status.hint.set(None),onfocus:move |_|status.hint.set(Some(label.into())),onblur:move |_|status.hint.set(None),onclick:move |_|{opened.set(None);command(action);},span {"{label}"} span {"{shortcut}"}}
        }
       }
      }
     }
     if about() {
      dialog {class:"about-dialog",aria_label:"About SDE",onmounted:move |_|{let _=document::eval("document.querySelector('.about-dialog')?.showModal();");},oncancel:move |e|{e.prevent_default();about.set(false);},
       div {class:"sheet-heading",h2 {"ᜊᜐ᜔ᜃᜄᜈ᜔"} UiButton {label:"Close About",icon:"close",class:"about-close",onpress:move |_|{let _=document::eval("document.querySelector('.about-dialog')?.close();");about.set(false);}}}
       p {"SDE"} p {"Version ",{env!("CARGO_PKG_VERSION")}}
      }
     }
    }
}
