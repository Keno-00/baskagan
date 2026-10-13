use crate::{
    Status,
    ui_controls::{Icon, UiButton},
};
use dioxus::prelude::*;
use serde::Deserialize;
use serde_json::json;
use std::path::PathBuf;

#[derive(Clone, Default, Deserialize, PartialEq)]
pub struct PanelState {
    pub active: Option<u64>,
    pub tabs: Vec<TabState>,
    pub notice: Option<Notice>,
}

#[derive(Clone, Deserialize, PartialEq)]
pub struct Notice {
    pub text: String,
    pub tab: Option<u64>,
    #[serde(default)]
    pub deleting: bool,
}

#[derive(Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TabState {
    pub id: u64,
    pub kind: String,
    pub title: String,
    pub path: Option<String>,
    pub dirty: bool,
    pub browser: bool,
    pub loading: bool,
    pub saving: bool,
    pub locked: bool,
    #[serde(default)]
    pub operation: Option<String>,
    pub comment_top: Option<f64>,
    pub markers: Vec<CommentMarker>,
}

#[derive(Clone, Deserialize, PartialEq)]
pub struct CommentMarker {
    pub top: f64,
    pub ids: String,
}

pub fn command(action: &str, id: u64, value: &str) {
    let _ = document::eval(&format!(
        "window.dadoWorkbench?.command({}, {}, {});",
        json!(action),
        id,
        json!(value)
    ));
}

#[component]
pub fn PanelContents(state: Signal<PanelState>, root: Option<PathBuf>) -> Element {
    let snapshot = state.read().clone();
    rsx! {
        div { class: "tab-bar",
            div { class: "tab-list", role: "tablist", aria_label: "Editor and terminal tabs",
                for tab in &snapshot.tabs {
                    TabHeader { key: "{tab.id}", tab: tab.clone(), state }
                }
            }
            UiButton { label: "New tab", icon: "plus", onpress: move |_| command("new", 0, "") }
        }
        div { class: "workbench-notices",
            if let Some(notice) = &snapshot.notice {
                div { class: if notice.tab.is_some() { "unsaved-notice" } else { "editor-notice" }, role: "alert",
                    span { "{notice.text}" }
                    if let Some(id) = notice.tab {
                        UiButton {
                            label: if notice.deleting { "Delete & close" } else { "Save & close" },
                            text: if notice.deleting { "Delete & close" } else { "Save & close" }, class: "workbench-text",
                            disabled: snapshot.tabs.iter().any(|tab| tab.id == id && (tab.saving || tab.locked)),
                            onpress: move |_| command("save-close", id, "")
                        }
                        UiButton { label: "Discard", text: "Discard", class: "workbench-text", onpress: move |_| command("discard", id, "") }
                        UiButton { label: "Cancel", text: "Cancel", class: "workbench-text", onpress: move |_| command("dismiss", 0, "") }
                    } else {
                        UiButton { label: "Dismiss message", icon: "close", onpress: move |_| command("dismiss", 0, "") }
                    }
                }
            }
        }
        div { class: "tab-bodies",
            for tab in snapshot.tabs {
                TabBody { key: "{tab.id}", active: snapshot.active == Some(tab.id), tab, root: root.clone() }
            }
        }
    }
}

#[component]
fn TabHeader(tab: TabState, state: Signal<PanelState>) -> Element {
    let mut status = use_context::<Status>();
    let id = tab.id;
    let active = state.read().active == Some(id);
    let hint = tab.path.clone().unwrap_or_else(|| tab.title.clone());
    let focus_hint = hint.clone();
    rsx! {
        div { class: if active { "panel-tab active" } else { "panel-tab" },
            button {
                id: "panel-tab-{id}", class: "tab-title", r#type: "button", role: "tab",
                aria_label: tab.title.clone(),
                aria_describedby: if tab.dirty { Some(format!("tab-dirty-{id}")) } else { None },
                aria_selected: active, aria_controls: "tab-body-{id}", tabindex: if active { "0" } else { "-1" },
                onmouseenter: move |_| status.hint.set(Some(hint.clone())),
                onmouseleave: move |_| status.hint.set(None),
                onfocus: move |_| status.hint.set(Some(focus_hint.clone())),
                onblur: move |_| status.hint.set(None),
                onclick: move |_| command("activate", id, ""),
                onkeydown: move |event| {
                    let model = state.read();
                    let index = model.tabs.iter().position(|tab| tab.id == id).unwrap_or(0);
                    let count = model.tabs.len();
                    if count == 0 { return; }
                    let next = match event.key() {
                        Key::ArrowRight => (index + 1) % count,
                        Key::ArrowLeft => (index + count - 1) % count,
                        Key::Home => 0,
                        Key::End => count - 1,
                        _ => return,
                    };
                    event.prevent_default();
                    let target = model.tabs[next].id;
                    command("activate", target, "");
                    let _ = document::eval(&format!("document.getElementById('panel-tab-{target}')?.focus();"));
                },
                Icon { name: if tab.kind == "terminal" { "terminal" } else { "editor" } }
                span { class: "tab-name", "{tab.title}" }
                if tab.dirty { span { id: "tab-dirty-{id}", class: "dirty-dot", aria_label: "Unsaved changes", "•" } }
            }
            UiButton { label: format!("Close {}", tab.title), icon: "close", disabled: tab.locked, onpress: move |_| command("close", id, "") }
        }
    }
}

#[component]
fn TabBody(tab: TabState, active: bool, root: Option<PathBuf>) -> Element {
    let id = tab.id;
    let caption = match tab.operation.as_deref() {
        Some("create") => format!("{} · new", tab.title),
        Some("delete") => format!("{} · pending deletion", tab.title),
        _ => tab.title.clone(),
    };
    let save_label = match tab.operation.as_deref() {
        Some("create") => "Save new file · Ctrl+S",
        Some("delete") => "Delete reviewed file · Ctrl+S",
        _ => "Save file · Ctrl+S",
    };
    rsx! {
        section { id: "tab-body-{id}", class: "tab-body", role: "tabpanel", aria_labelledby: "panel-tab-{id}", hidden: !active,
            if tab.kind == "choice" {
                div { class: "tab-chooser",
                    UiButton { label: "Terminal", text: "Terminal", icon: "terminal", class: "type-button", onpress: move |_| command("terminal", id, "") }
                    UiButton { label: "Editor", text: "Editor", icon: "editor", class: "type-button", onpress: move |_| command("editor", id, "") }
                }
            } else if tab.kind == "terminal" {
                div { id: "terminal-widget-{id}", class: "terminal-host" }
            } else {
                div { class: "editor-toolbar",
                    div { class: "editor-caption", "{caption}" }
                    UiButton { label: "Open file", icon: "editor", disabled: tab.locked || tab.loading, onpress: move |_| command("pick", id, "") }
                    UiButton { label: save_label, icon: "save", disabled: tab.path.is_none() || tab.saving || tab.locked || tab.loading, onpress: move |_| command("save", id, "") }
                    UiButton { label: "Toggle file browser", icon: "folder", onpress: move |_| command("browser", id, "") }
                }
                div { class: "editor-layout",
                    div { class: "editor-host",
                        // This empty leaf belongs to CodeMirror. Dioxus never replaces
                        // it while the tab is alive, including when hidden.
                        div { id: "editor-widget-{id}", class: "editor-widget", hidden: tab.path.is_none() }
                        if tab.path.is_none() {
                            div { class: "open-file-empty",
                                UiButton { label: "Open file", text: "Open file", icon: "folder", class: "type-button", disabled: tab.loading, onpress: move |_| command("pick", id, "") }
                            }
                        }
                        div { class: "editor-comment-rail", hidden: tab.path.is_none(),
                            div { class: "editor-line-counts",
                                for marker in tab.markers {
                                    UiButton {
                                        key: "{marker.ids}", label: format!("View code comments {}", marker.ids),
                                        text: marker.ids.clone(), icon: "comment", class: "editor-line-count comment-badge workbench-icon",
                                        style: format!("top: {}px", marker.top), preserve_selection: true,
                                        onpress: move |_| command("comment-list", id, &marker.ids)
                                    }
                                }
                            }
                            if let Some(top) = tab.comment_top {
                                UiButton {
                                    id: format!("code-comment-{id}"), label: "Comment on code", icon: "comment",
                                    class: "annotation-trigger editor-line-comment workbench-icon", style: format!("top: {top}px"),
                                    preserve_selection: true, onpress: move |_| command("comment", id, "")
                                }
                            }
                        }
                    }
                    aside { class: "file-explorer", hidden: !tab.browser, aria_label: "File browser",
                        FileBrowser { root, selected: tab.path.clone(), visible: active && tab.browser }
                    }
                }
            }
        }
    }
}

#[derive(Clone, Deserialize, PartialEq)]
struct FileEntry {
    path: PathBuf,
    label: String,
    directory: bool,
}

#[component]
fn FileBrowser(root: Option<PathBuf>, selected: Option<String>, visible: bool) -> Element {
    let mut filter = use_signal(String::new);
    rsx! {
        input { class: "file-filter", aria_label: "Filter files", placeholder: "Filter files…", value: "{filter}", oninput: move |event| filter.set(event.value()) }
        div { class: "file-tree", role: "tree", aria_label: "Project files",
            if let Some(path) = root {
                FolderEntries { key: "{path.display()}", path, selected, filter, depth: 0, visible }
            } else { p { class: "explorer-empty", "Open a project folder" } }
        }
    }
}

#[component]
fn FolderEntries(
    path: PathBuf,
    selected: Option<String>,
    filter: Signal<String>,
    depth: usize,
    visible: bool,
) -> Element {
    let mut entries = use_signal(Vec::<FileEntry>::new);
    let mut error = use_signal(|| None::<String>);
    let visible_now = use_signal(|| visible);
    let mut visible_now = visible_now;
    use_effect(use_reactive((&visible,), move |(visible,)| {
        visible_now.set(visible)
    }));
    use_future(move || {
        let path = path.clone();
        async move {
            let mut first = true;
            loop {
                if first || *visible_now.peek() {
                    first = false;
                    let path = path.clone();
                    let result = tokio::task::spawn_blocking(move || {
                        crate::workbench::list_folder(&path).and_then(|value| {
                            serde_json::from_value::<Vec<FileEntry>>(value)
                                .map_err(|e| e.to_string())
                        })
                    })
                    .await;
                    match result {
                        Ok(Ok(next)) => {
                            if *entries.peek() != next {
                                entries.set(next);
                            }
                            error.set(None);
                        }
                        Ok(Err(problem)) => error.set(Some(problem)),
                        Err(problem) => error.set(Some(problem.to_string())),
                    }
                }
                tokio::time::sleep(std::time::Duration::from_secs(4)).await;
            }
        }
    });
    rsx! {
        if let Some(problem) = error() { p { class: "explorer-empty", role: "alert", "{problem}" } }
        for entry in entries() {
            FileBranch { key: "{entry.path.display()}", entry, selected: selected.clone(), filter, depth, visible }
        }
    }
}

#[component]
fn FileBranch(
    entry: FileEntry,
    selected: Option<String>,
    filter: Signal<String>,
    depth: usize,
    visible: bool,
) -> Element {
    let mut expanded = use_signal(|| false);
    let mut opened = use_signal(|| false);
    let mut status = use_context::<Status>();
    let hidden = !entry.directory
        && !entry
            .label
            .to_lowercase()
            .contains(&filter().to_lowercase());
    let path = entry.path.to_string_lossy().into_owned();
    let current = selected.as_deref() == Some(&path);
    let hint = path.clone();
    let focus_hint = path.clone();
    let child_path = entry.path.clone();
    let directory = entry.directory;
    rsx! {
        div { class: "file-branch", hidden,
            button {
                class: format!("file-row{}{}", if current { " current-file" } else { "" }, if expanded() { " expanded" } else { "" }),
                r#type: "button", role: "treeitem", aria_label: entry.label.clone(), aria_level: depth + 1,
                aria_selected: current, aria_expanded: if directory { Some(expanded()) } else { None },
                style: format!("padding-left: {}px", 10 + depth * 12),
                onmouseenter: move |_| status.hint.set(Some(hint.clone())),
                onmouseleave: move |_| status.hint.set(None),
                onfocus: move |_| status.hint.set(Some(focus_hint.clone())),
                onblur: move |_| status.hint.set(None),
                onclick: move |_| {
                    if directory { opened.set(true); expanded.toggle(); }
                    else { let _ = document::eval(&format!("window.dadoWorkbench?.openFile({});", json!(path))); }
                },
                onkeydown: move |event| {
                    if directory && matches!(event.key(), Key::ArrowLeft | Key::ArrowRight) {
                        event.prevent_default(); opened.set(true); expanded.set(event.key() == Key::ArrowRight);
                    }
                },
                Icon { name: if directory { "chevron" } else { "editor" } }
                span { "{entry.label}" }
            }
            if opened() {
                div { class: "file-children", role: "group", hidden: !expanded(),
                    FolderEntries { path: child_path, selected, filter, depth: depth + 1, visible: visible && expanded() }
                }
            }
        }
    }
}
