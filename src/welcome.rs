use crate::ui_controls::UiButton;
use dioxus::prelude::*;
use std::path::{Path, PathBuf};

fn history_file() -> Result<PathBuf, String> {
    let home = crate::user_settings::home()?;
    let folder = home.join(".dado/sde");
    let file = folder.join("recents.json");
    if [home.join(".dado"), folder, file.clone()]
        .iter()
        .any(|p| crate::project_store::linked(p))
    {
        return Err("Recent folders storage cannot be a symbolic link.".into());
    }
    Ok(file)
}
fn load_recents() -> Result<Vec<PathBuf>, String> {
    let file = history_file()?;
    read_recents(&file)
}
pub fn startup_project(argument: Option<PathBuf>, reopen: bool) -> Option<PathBuf> {
    let recent = if argument.is_none() && reopen {
        load_recents().ok()?.into_iter().next()
    } else {
        None
    };
    startup_path(argument, reopen, recent)
}
fn startup_path(
    argument: Option<PathBuf>,
    reopen: bool,
    recent: Option<PathBuf>,
) -> Option<PathBuf> {
    let path = if argument.is_some() {
        argument
    } else if reopen {
        recent
    } else {
        None
    }?;
    if !path.is_dir() {
        return None;
    }
    Some(if path.is_absolute() {
        path
    } else {
        std::env::current_dir().ok()?.join(path)
    })
}
fn read_recents(file: &Path) -> Result<Vec<PathBuf>, String> {
    if !file.exists() {
        return Ok(Vec::new());
    }
    serde_json::from_slice(&std::fs::read(file).map_err(|e| e.to_string())?)
        .map_err(|e| format!("Could not read recent folders: {e}"))
}
pub fn remember(path: &Path) -> Result<(), String> {
    remember_at(path, &history_file()?)
}
fn remember_at(path: &Path, file: &Path) -> Result<(), String> {
    let mut recents = read_recents(file)?;
    let path = std::fs::canonicalize(path).map_err(|e| e.to_string())?;
    recents.retain(|p| p != &path);
    recents.insert(0, path);
    recents.truncate(30);
    save_recents(&recents, file)
}
fn forget(path: &Path) -> Result<Vec<PathBuf>, String> {
    forget_at(path, &history_file()?)
}
fn forget_at(path: &Path, file: &Path) -> Result<Vec<PathBuf>, String> {
    let mut recents = read_recents(file)?;
    recents.retain(|recent| recent != path);
    save_recents(&recents, file)?;
    Ok(recents)
}
fn save_recents(recents: &[PathBuf], file: &Path) -> Result<(), String> {
    std::fs::create_dir_all(file.parent().unwrap()).map_err(|e| e.to_string())?;
    let mut temp =
        tempfile::NamedTempFile::new_in(file.parent().unwrap()).map_err(|e| e.to_string())?;
    serde_json::to_writer_pretty(&mut temp, &recents).map_err(|e| e.to_string())?;
    temp.as_file().sync_all().map_err(|e| e.to_string())?;
    temp.persist(file).map_err(|e| e.to_string())?;
    Ok(())
}
fn clone_repository(source: &str, destination: &Path) -> Result<(), String> {
    if source.trim().is_empty() || source.starts_with('-') || source.chars().any(char::is_control) {
        return Err("Enter a valid repository URL or path.".into());
    }
    if destination.exists() {
        return Err("Choose a new folder for the repository.".into());
    }
    let mut command = std::process::Command::new("git");
    command
        .args(["clone", "--"])
        .arg(source)
        .arg(destination)
        .env("GIT_TERMINAL_PROMPT", "0");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let output = command
        .output()
        .map_err(|e| format!("Could not start Git: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "Could not clone repository: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(())
}

#[component]
pub fn Welcome(
    mut project: Signal<Option<PathBuf>>,
    mut creating: Signal<bool>,
    mut name: Signal<String>,
    mut error: Signal<String>,
) -> Element {
    let mut status = use_context::<crate::Status>();
    let mut recents = use_signal(load_recents);
    let mut cloning = use_signal(|| false);
    let mut source = use_signal(String::new);
    let mut busy = use_signal(|| false);
    let was_cloning = use_hook(|| std::rc::Rc::new(std::cell::Cell::new(false)));
    use_effect(move || {
        let active = cloning();
        let previous = was_cloning.replace(active);
        if active || previous {
            let selector = if active {
                "#repository-url"
            } else {
                ".welcome-actions button:last-child"
            };
            let _ = document::eval(&format!(
                "requestAnimationFrame(() => document.querySelector({})?.focus());",
                serde_json::json!(selector)
            ));
        }
    });
    rsx! {
        style { {include_str!("welcome.css")} }
        section { class: "welcome", aria_label: "BASKAGAN Systems Development Environment",
            header { class: "welcome-brand",
                p { "Systems Development Environment " span { class: "welcome-abbreviation", "(SDE)" } }
                h1 { class: "welcome-baybayin", lang: "tl", "ᜊᜐ᜔ᜃᜄᜈ᜔" }
            }
            div { class: "welcome-content",
                if cloning() {
                    form { class: "welcome-clone", aria_label: "Clone Repository",
                        onsubmit: move |event| {
                            event.prevent_default();
                            if busy() { return; }
                            let url = source().trim().to_string();
                            if url.is_empty() { error.set("Enter a repository URL or path.".into()); return; }
                            spawn(async move {
                                let Some(folder) = rfd::AsyncFileDialog::new().set_title("Choose a new folder for the repository").set_file_name("repository").save_file().await else { return; };
                                busy.set(true);
                                error.set(String::new());
                                let destination = folder.path().to_path_buf();
                                let target = destination.clone();
                                let result = tokio::task::spawn_blocking(move || clone_repository(&url, &target)).await;
                                busy.set(false);
                                match result {
                                    Ok(Ok(())) => { project.set(Some(destination)); cloning.set(false); }
                                    Ok(Err(problem)) => error.set(problem),
                                    Err(problem) => error.set(problem.to_string()),
                                }
                            });
                        },
                        label { r#for: "repository-url", "Repository URL" }
                        input { id: "repository-url", autofocus: true, value: "{source}", disabled: busy(), oninput: move |e| source.set(e.value()), onkeydown: move |e| { if e.key() == Key::Escape && !busy() { cloning.set(false); error.set(String::new()); } } }
                        div { class: "welcome-clone-actions",
                            button { class: "ui-text-button", r#type: "submit", disabled: busy(), if busy() { "Cloning…" } else { "Clone Repository" } }
                            UiButton { class: "ui-text-button", label: "Cancel", text: "Cancel", disabled: busy(), onpress: move |_| { cloning.set(false); error.set(String::new()); } }
                        }
                    }
                } else {
                    nav { class: "welcome-actions", aria_label: "Folder actions",
                        h2 { class: "welcome-start", "Start" }
                        UiButton { class: "ui-text-button", label: "Create Folder", text: "Create Folder", hint: "Create a new project folder", onpress: move |_| { name.set(String::new()); error.set(String::new()); creating.set(true); } }
                        UiButton { class: "ui-text-button", label: "Open Folder", text: "Open Folder", hint: "Open an existing project folder", onpress: move |_| crate::open_folder(project, creating) }
                        UiButton { class: "ui-text-button", label: "Clone Repository", text: "Clone Repository", hint: "Clone a Git repository into a new folder", onpress: move |_| { error.set(String::new()); cloning.set(true); } }
                    }
                }
                section { class: "welcome-recents", aria_label: "Recents",
                    h2 { "Recents" }
                    div { class: "welcome-recent-list",
                        match &recents() {
                            Ok(paths) if paths.is_empty() => rsx! { p { class: "welcome-muted", "No recent folders" } },
                            Ok(paths) => rsx! {
                                for path in paths {
                                    {let target = path.clone(); let remove_target = target.clone(); let label = path.file_name().unwrap_or(path.as_os_str()).to_string_lossy().to_string(); let display = path.display().to_string().trim_start_matches("\\\\?\\").to_string(); rsx! {
                                        div { class: "welcome-recent-row",
                                            button { class: "ui-text-button welcome-recent", r#type: "button", aria_label: "Open {display}",
                                                onmouseenter: {let hint = display.clone(); move |_| status.hint.set(Some(hint.clone()))},
                                                onmouseleave: move |_| status.hint.set(None),
                                                onfocus: {let hint = display.clone(); move |_| status.hint.set(Some(hint.clone()))},
                                                onblur: move |_| status.hint.set(None),
                                                onclick: move |_| { if target.is_dir() { project.set(Some(target.clone())); } else { error.set("This recent folder is no longer available.".into()); } },
                                                span { "{label}" } span { class: "welcome-muted", "{display}" }
                                            }
                                            div { class: "welcome-recent-actions",
                                                UiButton {
                                                    class: "workbench-icon welcome-recent-clear",
                                                    label: "Remove {display} from recents",
                                                    hint: "Remove from recents",
                                                    icon: "close",
                                                    onpress: move |_| match forget(&remove_target) {
                                                        Ok(remaining) => { recents.set(Ok(remaining)); error.set(String::new()); }
                                                        Err(problem) => error.set(problem),
                                                    }
                                                }
                                            }
                                        }
                                    }}
                                }
                            },
                            Err(problem) => rsx! { p { class: "error", role: "alert", "{problem}" } },
                        }
                    }
                }
                if !error().is_empty() { p { class: "error", role: "alert", "{error}" } }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn startup_respects_preference_and_explicit_project() {
        let recent = tempfile::tempdir().unwrap();
        let explicit = tempfile::tempdir().unwrap();
        let r = Some(recent.path().to_path_buf());
        assert_eq!(startup_path(None, true, r.clone()), r);
        assert_eq!(startup_path(None, false, r.clone()), None);
        assert_eq!(
            startup_path(Some(explicit.path().to_path_buf()), false, r.clone()),
            Some(explicit.path().to_path_buf())
        );
        assert_eq!(
            startup_path(Some(recent.path().join("missing")), true, r),
            None
        );
    }
    #[test]
    fn recents_persist_reorder_and_preserve_malformed_history() {
        let temp = tempfile::tempdir().unwrap();
        let a = temp.path().join("a");
        let b = temp.path().join("b");
        std::fs::create_dir(&a).unwrap();
        std::fs::create_dir(&b).unwrap();
        let file = temp.path().join("recents.json");
        remember_at(&a, &file).unwrap();
        remember_at(&b, &file).unwrap();
        remember_at(&a, &file).unwrap();
        assert_eq!(
            read_recents(&file).unwrap(),
            vec![a.canonicalize().unwrap(), b.canonicalize().unwrap()]
        );
        std::fs::write(&file, "broken").unwrap();
        assert!(remember_at(&a, &file).is_err());
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "broken");
    }
    #[test]
    fn clone_local_repository_and_protect_existing_destination() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        assert!(
            std::process::Command::new("git")
                .arg("init")
                .arg(&source)
                .output()
                .unwrap()
                .status
                .success()
        );
        let destination = temp.path().join("clone");
        clone_repository(source.to_str().unwrap(), &destination).unwrap();
        assert!(destination.join(".git").is_dir());
        assert!(clone_repository(source.to_str().unwrap(), &destination).is_err());
        assert!(clone_repository("--upload-pack=bad", &temp.path().join("bad")).is_err());
    }
}
