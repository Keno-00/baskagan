use crate::Status;
use dioxus::prelude::*;

#[component]
pub fn Icon(name: String) -> Element {
    let shape = match name.as_str() {
        "back" => "m14 6-6 6 6 6M8 12h12",
        "forward" => "m10 6 6 6-6 6M4 12h12",
        "sidebar" => "M4 5h16v14H4zM9 5v14",
        "minimize" => "M5 12h14",
        "maximize" => "M5 5h14v14H5z",
        "window-close" => "m6 6 12 12M18 6 6 18",
        "new-chat" => "M13 5H5v14h14v-8M12 12l8-8M17 4h3v3",
        "profile" => "M8 8a4 4 0 1 0 8 0 4 4 0 0 0-8 0M4 21v-2a8 8 0 0 1 16 0v2",
        "settings" => {
            "M12 8a4 4 0 1 0 0 8 4 4 0 0 0 0-8M9 3h6l1 3 3 1 2 5-2 5-3 1-1 3H9l-1-3-3-1-2-5 2-5 3-1z"
        }
        "appearance" => "M12 3a9 9 0 1 0 0 18V3zM12 3a9 9 0 0 1 0 18",
        "model" => "m9 6-6 6 6 6M15 6l6 6-6 6M14 4l-4 16",
        "external" => "M14 4h6v6M20 4l-9 9M10 5H4v15h15v-6",
        "refresh" => "M20 7v5h-5M20 12a8 8 0 1 0-2 6",
        "usage" => "M4 17a9 9 0 1 1 16 0M12 14l4-6",
        "layout" => "M5 12h.01M12 12h.01M19 12h.01",
        "terminal" => "M4 5h16v14H4zM7 9l3 3-3 3m5 0h4",
        "editor" => "M7 3h7l4 4v14H7zM14 3v5h4M10 12h5m-5 4h5",
        "folder" => "M3 7h7l2 2h9v11H3zM3 7V4h7l2 3",
        "workflow" => "M3 9h5v6H3zM16 9h5v6h-5zM8 12h8m-3-3 3 3-3 3",
        "mental" => "M9 3h6v5H9zM3 16h6v5H3zM15 16h6v5h-6zM12 8v4M6 16v-4h12v4",
        "save" => "M5 4h13l2 2v14H4V4zM8 4v6h8V4M8 20v-7h8v7",
        "plus" => "M12 5v14M5 12h14",
        "close" => "M7 7l10 10M17 7L7 17",
        "chevron" => "M9 5l7 7-7 7",
        "idea" => "M9 18h6M10 21h4M8 14a6 6 0 1 1 8 0l-1 4H9z",
        "comment" => "M4 4h16v12H9l-5 4z",
        "edit" => "m5 15 10-10 4 4-10 10-5 1zM13 7l4 4",
        "warning" => "M12 3 2 21h20L12 3zM12 9v5m0 3v1",
        "check" => "m5 12 4 4 10-10",
        "send" => "m6 12 6-6 6 6M12 6v13",
        "stop" => "M6 6h12v12H6z",
        "skill" => "M5 4h14v16H5zM8 8h8M8 12h8M8 16h5",
        "delete" => "M4 6h16M9 6V3h6v3M6 6l1 15h10l1-15M10 10v7m4-7v7",
        _ => "",
    };
    rsx! { svg { view_box: "0 0 24 24", "aria-hidden": "true", path { d: shape } } }
}

/// Compact controls share geometry, focus treatment and bottom-strip hints.
#[component]
pub fn UiButton(
    label: String,
    #[props(default)] hint: Option<String>,
    #[props(default)] icon: &'static str,
    #[props(default = "workbench-icon")] class: &'static str,
    #[props(default)] text: Option<String>,
    #[props(default)] disabled: bool,
    #[props(default)] id: Option<String>,
    #[props(default)] style: String,
    #[props(default)] preserve_selection: bool,
    #[props(default)] expanded: Option<bool>,
    #[props(default)] pressed: Option<bool>,
    #[props(default)] controls: Option<String>,
    #[props(default)] popup: Option<String>,
    onpress: EventHandler<MouseEvent>,
) -> Element {
    let mut status = use_context::<Status>();
    let hover_hint = hint.unwrap_or_else(|| label.clone());
    let focus_hint = hover_hint.clone();
    rsx! {
        button {
            id, class, style, r#type: "button", aria_label: label, disabled,
            aria_expanded: expanded, aria_pressed:pressed, aria_controls: controls, aria_haspopup: popup,
            onmouseenter: move |_| status.hint.set(Some(hover_hint.clone())),
            onmouseleave: move |_| status.hint.set(None),
            onfocus: move |_| status.hint.set(Some(focus_hint.clone())),
            onblur: move |_| status.hint.set(None),
            onmousedown: move |event| { if preserve_selection { event.prevent_default(); } },
            onclick: move |event| onpress.call(event),
            if !icon.is_empty() { Icon { name: icon } }
            if let Some(text) = text { span { "{text}" } }
        }
    }
}
