use crate::{Status, Workbench, terminal::Terminals};
use dioxus::prelude::*;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};

#[derive(Deserialize)]
struct Request {
    action: String,
    id: Option<u64>,
    tab: Option<u64>,
    path: Option<PathBuf>,
    text: Option<String>,
    expected: Option<String>,
    cols: Option<u16>,
    rows: Option<u16>,
    state: Option<crate::workbench_ui::PanelState>,
}

fn reply(id: Option<u64>, result: Result<Value, String>) {
    let value = match result {
        Ok(result) => json!({"id": id, "result": result}),
        Err(error) => json!({"id": id, "error": error}),
    };
    let _ = document::eval(&format!("window.dadoWorkbench?.reply({value});"));
}

fn read_file(path: &Path) -> Result<Value, String> {
    let metadata = std::fs::metadata(path).map_err(|e| e.to_string())?;
    if !metadata.is_file() {
        return Err("Select a file to edit.".into());
    }
    if metadata.len() > 5 * 1024 * 1024 {
        return Err("This first editor supports files up to 5 MB.".into());
    }
    let text = std::fs::read_to_string(path)
        .map_err(|_| "Could not read this file as UTF-8 text.".to_string())?;
    if text.contains('\0') {
        return Err("This is a binary file and cannot be edited as text.".into());
    }
    Ok(json!({"path": path, "text": text}))
}

fn save_file(path: &Path, text: &str, expected: &str) -> Result<Value, String> {
    let on_disk = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    if on_disk != expected {
        return Err("The file changed on disk. Reopen it in another tab to review those changes before saving.".into());
    }
    let parent = path.parent().ok_or("File has no parent folder")?;
    let permissions = std::fs::metadata(path)
        .map_err(|e| e.to_string())?
        .permissions();
    if permissions.readonly() {
        return Err("This file is read-only.".into());
    }
    let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    temporary
        .write_all(text.as_bytes())
        .map_err(|e| e.to_string())?;
    temporary.as_file().sync_all().map_err(|e| e.to_string())?;
    std::fs::set_permissions(temporary.path(), permissions).map_err(|e| e.to_string())?;
    temporary.persist(path).map_err(|e| e.to_string())?;
    Ok(json!({"path": path}))
}

pub(crate) fn list_folder(path: &Path) -> Result<Value, String> {
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(path).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        if kind.is_symlink() {
            continue;
        }
        entries.push(json!({"path": entry.path(), "label": entry.file_name().to_string_lossy(), "directory": kind.is_dir()}));
    }
    entries.sort_by_key(|entry| {
        (
            entry["directory"] != true,
            entry["label"].as_str().unwrap_or_default().to_lowercase(),
        )
    });
    Ok(json!(entries))
}

#[component]
pub fn SidePanel(root: Option<PathBuf>) -> Element {
    let mut workbench = use_context::<Workbench>();
    let mut status = use_context::<Status>();
    let mut ready = use_signal(|| false);
    let mut ui = use_signal(crate::workbench_ui::PanelState::default);
    let terminals = use_hook(|| Arc::new(Mutex::new(Terminals::default())));
    let (output, input) = use_hook(|| {
        let (sender, receiver) = mpsc::sync_channel::<(u64, Vec<u8>)>(512);
        (sender, Arc::new(Mutex::new(receiver)))
    });
    use_future(move || {
        let input = input.clone();
        async move {
            loop {
                tokio::time::sleep(Duration::from_millis(24)).await;
                let mut chunks = Vec::new();
                if let Ok(receiver) = input.lock() {
                    for _ in 0..64 {
                        match receiver.try_recv() {
                            Ok(chunk) => chunks.push(chunk),
                            Err(_) => break,
                        }
                    }
                }
                if !chunks.is_empty() {
                    let _ = document::eval(&format!(
                        "window.dadoWorkbench?.terminalOutput({});",
                        json!(chunks)
                    ));
                }
            }
        }
    });
    use_future(move || {
        let terminals = terminals.clone();
        let output = output.clone();
        async move {
            let mut bridge = document::eval(&format!(
                "{}\nwindow.startWorkbench(message => dioxus.send(message));",
                include_str!("../assets/workbench.js")
            ));
            while let Ok(request) = bridge.recv::<Request>().await {
                let id = request.id;
                match request.action.as_str() {
                    "ui" => {
                        if let Some(next) = request.state
                            && *ui.peek() != next
                        {
                            ui.set(next);
                        }
                    }
                    "ready" => ready.set(true),
                    "hint" => status.hint.set(request.text),
                    "selected" => workbench.selected_file.set(request.path),
                    "read" | "read_state" | "save" | "list" => {
                        let result = tokio::task::spawn_blocking(move || {
                            let path = request.path.ok_or("No file selected")?;
                            match request.action.as_str() {
                                "read" => read_file(&path),
                                "read_state" => match std::fs::symlink_metadata(&path) {
                                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                                        Ok(json!({"exists":false}))
                                    }
                                    Err(e) => Err(e.to_string()),
                                    Ok(_) => read_file(&path),
                                },
                                "save" => save_file(
                                    &path,
                                    request.text.as_deref().ok_or("No content")?,
                                    request.expected.as_deref().ok_or("No original content")?,
                                ),
                                _ => list_folder(&path),
                            }
                        })
                        .await
                        .unwrap_or_else(|e| Err(e.to_string()));
                        reply(id, result);
                    }
                    "pick" => {
                        let file = rfd::AsyncFileDialog::new()
                            .set_title("Open file")
                            .pick_file()
                            .await;
                        reply(id, Ok(file.map(|f| json!(f.path())).unwrap_or(Value::Null)));
                    }
                    "terminal_open" | "terminal_input" | "terminal_resize" | "terminal_close" => {
                        let terminals = terminals.clone();
                        let output = output.clone();
                        let result = tokio::task::spawn_blocking(move || {
                            let tab = request.tab.ok_or("No terminal selected")?;
                            let mut sessions = terminals.lock().map_err(|e| e.to_string())?;
                            match request.action.as_str() {
                                "terminal_open" => sessions.open(
                                    tab,
                                    &request.path.unwrap_or_else(|| {
                                        std::env::current_dir().unwrap_or_default()
                                    }),
                                    request.cols.unwrap_or(80),
                                    request.rows.unwrap_or(24),
                                    output,
                                )?,
                                "terminal_input" => sessions
                                    .input(tab, request.text.as_deref().unwrap_or_default())?,
                                "terminal_resize" => sessions.resize(
                                    tab,
                                    request.cols.unwrap_or(80),
                                    request.rows.unwrap_or(24),
                                )?,
                                _ => {
                                    sessions.0.remove(&tab);
                                }
                            }
                            Ok(Value::Null)
                        })
                        .await
                        .unwrap_or_else(|e| Err(e.to_string()));
                        reply(id, result);
                    }
                    _ => {}
                }
            }
        }
    });
    use_effect(move || {
        if ready() {
            let selected = (workbench.selected_file)();
            let _ = document::eval(&format!(
                "window.dadoWorkbench?.selectFile({});",
                json!(selected)
            ));
            if selected.is_some() {
                let _ = document::eval(&format!(
                    "window.dadoScene?.selectPath({});",
                    json!(selected)
                ));
            }
        }
    });
    use_effect(use_reactive((&root,), move |(root,)| {
        if ready() {
            let _ = document::eval(&format!("window.dadoWorkbench?.setRoot({});", json!(root)));
        }
    }));
    use_effect(move || {
        if ready() {
            let _ = document::eval(&format!(
                "window.dadoWorkbench?.setVisible({});",
                (workbench.shown)()
            ));
        }
    });
    rsx! { aside { id: "side-panel", class: "side-panel", aria_label: "Editor and terminal panel",
        div { id: "workbench-host", crate::workbench_ui::PanelContents { state: ui, root } }
    } }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn saves_preserve_text_and_reject_external_edits() {
        let folder = tempfile::tempdir().unwrap();
        let file = folder.path().join("sample.rs");
        let original = "\u{feff}fn main() {}\r\n";
        std::fs::write(&file, original).unwrap();
        assert_eq!(read_file(&file).unwrap()["text"], original);
        let updated = "\u{feff}fn main() { println!(\"hello\"); }\r\n";
        save_file(&file, updated, original).unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), updated);
        std::fs::write(&file, "External change").unwrap();
        assert!(save_file(&file, "Unsaved edits", updated).is_err());
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "External change");
    }
}
