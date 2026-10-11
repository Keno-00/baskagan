use crate::{Status, open_folder};
use dioxus::prelude::*;
use std::path::PathBuf;

#[component]
pub fn TitleBar(
    mut project: Signal<Option<PathBuf>>,
    mut creating: Signal<bool>,
    mut name: Signal<String>,
    mut error: Signal<String>,
) -> Element {
    let mut status = use_context::<Status>();
    use_future(move || async move {
        let mut bridge = document::eval(include_str!("titlebar.js"));
        let mut zoom = 1.0_f64;
        while let Ok(message) = bridge.recv::<serde_json::Value>().await {
            let desktop = dioxus::desktop::window();
            match message["action"].as_str().unwrap_or_default() {
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
                    zoom = match message["action"].as_str().unwrap() {
                        "zoom-in" => (zoom * 1.1).min(2.0),
                        "zoom-out" => (zoom / 1.1).max(0.5),
                        _ => 1.0,
                    };
                    desktop.set_zoom_level(zoom);
                }
                "hint" => status.hint.set(message["text"].as_str().map(String::from)),
                _ => {}
            }
        }
    });
    rsx! { header { id: "titlebar", class: "title-bar", aria_label: "Window title and menus" } }
}
