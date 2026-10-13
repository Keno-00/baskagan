use crate::Status;
use dioxus::prelude::*;
use serde::Deserialize;
use serde_json::json;
use std::path::PathBuf;

#[derive(Deserialize)]
struct Request {
    action: String,
    hint: Option<String>,
    id: Option<u64>,
    root: Option<PathBuf>,
    state: Option<serde_json::Value>,
    shown: Option<bool>,
    concept: Option<String>,
    kind: Option<String>,
    ticket: Option<String>,
    command: Option<String>,
    #[serde(flatten)]
    ai: serde_json::Map<String, serde_json::Value>,
}

#[component]
pub fn ChatPanel(root: Option<PathBuf>) -> Element {
    let mut general = use_context::<Signal<crate::user_settings::General>>();
    let client = use_hook(|| std::rc::Rc::new(crate::ai::Client::start()));
    let events = client.clone();
    use_future(move || {
        let events = events.clone();
        async move {
            loop {
                while let Ok(event) = events.events.try_recv() {
                    let handler = if event["kind"] == "rpc" {
                        "reply"
                    } else {
                        "aiEvent"
                    };
                    let _ = document::eval(&format!("window.dadoChat?.{handler}({event});"));
                }
                tokio::time::sleep(std::time::Duration::from_millis(25)).await;
            }
        }
    });
    let mut status = use_context::<Status>();
    let mut workbench = use_context::<crate::Workbench>();
    let mut ready = use_signal(|| false);
    let mut current_root = use_signal(|| root.clone());
    let mut project_generation = use_signal(|| 0u64);
    let project_client = client.clone();
    use_effect(use_reactive((&root,), move |(root,)| {
        if *current_root.peek() != root || *project_generation.peek() == 0 {
            let generation = *project_generation.peek() + 1;
            project_generation.set(generation);
            current_root.set(root.clone());
            project_client.send(json!({"action":"ai-project","root":root,"generation":generation}));
        }
        if ready() {
            let _ = document::eval(&format!(
                "window.dadoChat?.setProject({}, {});",
                json!(root),
                *project_generation.peek()
            ));
        }
    }));
    use_future(move || {
        let client = client.clone();
        async move {
            let mut bridge = document::eval(&format!(
                "{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\nwindow.startChat(message => dioxus.send(message), {});",
                include_str!("../ui/annotations.js"),
                include_str!("../ui/review.js"),
                include_str!("../ui/comment_context.js"),
                include_str!("../ui/sidebar.js"),
                include_str!("../ui/commands.js"),
                include_str!("../ui/skills.js"),
                include_str!("../ui/chat.js"),
                include_str!("../ui/concepts.js"),
                json!(include_str!("../assets/dioxus-avatar.txt"))
            ));
            let mut known = std::collections::HashSet::new();
            while let Ok(request) = bridge.recv::<Request>().await {
                match request.action.as_str() {
                    "preferences-load" | "preferences-save" => {
                        let result = crate::user_settings::home().and_then(|home| {
                            if request.action == "preferences-load" {
                                crate::user_settings::load(&home).map(|(settings, exists)| {
                                    general.set(settings.general.clone());
                                    json!({"settings":settings,"exists":exists})
                                })
                            } else {
                                let settings: crate::user_settings::Settings =
                                    serde_json::from_value(request.state.unwrap_or_default())
                                        .map_err(|e| e.to_string())?;
                                crate::user_settings::save(&home, &settings).map(|_| {
                                    general.set(settings.general.clone());
                                    json!(null)
                                })
                            }
                        });
                        let reply = match result {
                            Ok(result) => json!({"id":request.id,"result":result}),
                            Err(error) => json!({"id":request.id,"error":error}),
                        };
                        let _ = document::eval(&format!("window.dadoChat?.reply({reply});"));
                    }
                    "commands" | "command" => {
                        let valid = current_root().is_some() && request.root == current_root();
                        let result = if request.action == "commands" {
                            if valid {
                                Ok(json!(crate::agent::commands::COMMANDS))
                            } else {
                                Err("Open a project first.")
                            }
                        } else {
                            crate::agent::commands::resolve(
                                request.command.as_deref().unwrap_or_default(),
                                valid,
                            )
                            .map(|command| json!(command))
                        };
                        let reply = match result {
                            Ok(result) => json!({"id":request.id,"result":result}),
                            Err(error) => json!({"id":request.id,"error":error}),
                        };
                        let _ = document::eval(&format!("window.dadoChat?.reply({reply});"));
                    }
                    action if action.starts_with("ai-") => {
                        if action == "ai-send" && request.root != current_root() {
                            continue;
                        }
                        let mut value = serde_json::Value::Object(request.ai);
                        value["action"] = json!(action);
                        value["root"] = json!(request.root);
                        value["id"] = json!(request.id);
                        if action == "ai-send" {
                            value["selectedFile"] = json!((workbench.selected_file)());
                        }
                        client.send(value);
                    }
                    "panel" => workbench.shown.set(request.shown.unwrap_or(false)),
                    "ready" => {
                        ready.set(true);
                        let _ = document::eval(&format!(
                            "window.dadoChat?.setProject({}, {});",
                            json!(current_root()),
                            *project_generation.peek()
                        ));
                    }
                    "load" | "save" => {
                        let result = if let Some(path) = request.root {
                            if request.action == "load" && current_root().as_ref() == Some(&path) {
                                known.insert(path.clone());
                            }
                            if !known.contains(&path) {
                                Err("Project is not open.".into())
                            } else if request.action == "load" {
                                crate::project_store::load(&path)
                            } else if let Some(state) = request.state {
                                crate::project_store::save(&path, &state).map(|_| json!(null))
                            } else {
                                Err("Missing project state.".into())
                            }
                        } else {
                            Err("No project is open.".into())
                        };
                        let reply = match result {
                            Ok(state) => json!({"id":request.id,"result":state}),
                            Err(error) => {
                                status.hint.set(Some(error.clone()));
                                json!({"id":request.id,"error":error})
                            }
                        };
                        let _ = document::eval(&format!("window.dadoChat?.reply({reply});"));
                    }
                    "hint" => status.hint.set(request.hint),
                    "pick" => {
                        let concept = request.concept.unwrap_or_default();
                        let root = request.root;
                        let folder = request.kind.as_deref() == Some("folder");
                        spawn(async move {
                            let dialog =
                                rfd::AsyncFileDialog::new().set_title("Add context to concept");
                            let paths: Vec<_> = if folder { dialog.pick_folders().await.unwrap_or_default() }
                        else { dialog.pick_files().await.unwrap_or_default() }.into_iter()
                        .map(|file| json!({"path":file.path(),"kind":if folder {"folder"} else {"file"}})).collect();
                            if current_root() == root {
                                let _ = document::eval(&format!(
                                    "window.dadoSidebar?.addPaths({}, {});",
                                    json!(concept),
                                    json!(paths)
                                ));
                            }
                        });
                    }
                    "attach" => {
                        if request.root != current_root() || current_root().is_none() {
                            continue;
                        }
                        let folder = request.kind.as_deref() == Some("folder");
                        let ticket = request.ticket;
                        let root = request.root;
                        spawn(async move {
                            let dialog = rfd::AsyncFileDialog::new().set_title(if folder {
                                "Attach folders to your message"
                            } else {
                                "Attach files to your message"
                            });
                            let files = if folder {
                                dialog.pick_folders().await.unwrap_or_default()
                            } else {
                                dialog.pick_files().await.unwrap_or_default()
                            };
                            let attachments: Vec<_> = files.iter().filter_map(|file| {
                            let metadata = std::fs::metadata(file.path()).ok()?;
                            (if folder {metadata.is_dir()} else {metadata.is_file()}).then(|| json!({
                                "path": file.path(),
                                "name": file.path().file_name().unwrap_or_default().to_string_lossy(),
                                "kind": if folder {"folder"} else {"file"},
                                "size": if folder {None} else {Some(metadata.len())}
                            }))
                        }).collect();
                            if current_root() == root {
                                let _ = document::eval(&format!(
                                    "window.dadoChat?.attach({}, {});",
                                    json!(attachments),
                                    json!(ticket)
                                ));
                            }
                        });
                    }
                    _ => {}
                }
            }
        }
    });
    rsx! { section { id: "chat-host", aria_label: "Chat", crate::chat_history::ChatHistory { root: root.clone() } crate::composer::Composer {} } crate::annotations::Annotations {} crate::skill_creator::SkillCreator {} }
}
