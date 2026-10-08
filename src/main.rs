#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use dioxus::desktop::{Config, LogicalSize, WindowBuilder};
use dioxus::prelude::*;

mod constellation;
mod terminal;
mod tree;
mod workbench;
use constellation::Constellation;

#[derive(Clone, Copy)]
pub struct Status {
    pub hint: Signal<Option<String>>,
    pub path: Signal<String>,
}

#[derive(Clone, Copy)]
pub struct Workbench {
    pub shown: Signal<bool>,
    pub selected_file: Signal<Option<std::path::PathBuf>>,
}

fn open_folder(mut project: Signal<Option<std::path::PathBuf>>, mut creating: Signal<bool>) {
    spawn(async move {
        if let Some(folder) = rfd::AsyncFileDialog::new()
            .set_title("Open Folder")
            .pick_folder()
            .await
        {
            project.set(Some(folder.path().to_path_buf()));
            creating.set(false);
        }
    });
}

fn main() {
    LaunchBuilder::desktop()
        .with_cfg(
            Config::new().with_menu(None).with_window(
                WindowBuilder::new()
                    .with_title("DADO IDE")
                    .with_inner_size(LogicalSize::new(1200.0, 800.0)),
            ),
        )
        .launch(App);
}

#[component]
fn App() -> Element {
    let mut project = use_signal(|| {
        std::env::args_os()
            .nth(1)
            .map(std::path::PathBuf::from)
            .filter(|path| path.is_dir())
            .map(|path| {
                if path.is_absolute() {
                    path
                } else {
                    std::env::current_dir().unwrap_or_default().join(path)
                }
            })
    });
    let mut creating = use_signal(|| false);
    let mut name = use_signal(String::new);
    let mut error = use_signal(String::new);
    let mut menu = use_signal(|| false);
    let mut status = use_context_provider(|| Status {
        hint: Signal::new(None),
        path: Signal::new(String::new()),
    });
    let mut workbench = use_context_provider(|| Workbench {
        shown: Signal::new(false),
        selected_file: Signal::new(None),
    });
    use_effect(move || {
        status.path.set(
            project
                .read()
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_default(),
        );
        status.hint.set(None);
        workbench.selected_file.set(None);
    });

    rsx! {
        style { {include_str!("style.css")} }
        style { {include_str!("../assets/workbench.css")} }
        header { class: "menu-bar",
            button { class: "menu-trigger", aria_expanded: "{menu}",
                onmouseenter: move |_| status.hint.set(Some("Project actions".into())),
                onmouseleave: move |_| status.hint.set(None),
                onclick: move |_| menu.toggle(), "File"
            }
            if menu() {
                div { class: "menu-dismiss", onclick: move |_| menu.set(false) }
                div { class: "file-menu",
                    button { onmouseenter: move |_| status.hint.set(Some("Create a new project folder".into())),
                        onmouseleave: move |_| status.hint.set(None), onclick: move |_| {
                        name.set(String::new()); error.set(String::new()); creating.set(true); menu.set(false);
                    }, "New Project" }
                    button { onmouseenter: move |_| status.hint.set(Some("Open or switch to an existing folder".into())),
                        onmouseleave: move |_| status.hint.set(None), onclick: move |_| { menu.set(false); open_folder(project, creating); }, "Open Folder…" }
                    button { onmouseenter: move |_| status.hint.set(Some("Close the current project".into())),
                        onmouseleave: move |_| status.hint.set(None), disabled: project.read().is_none(), onclick: move |_| {
                        project.set(None); creating.set(false); menu.set(false);
                    }, "Close Project" }
                }
            }
        }
        div { class: if (workbench.shown)() { "workspace panel-open" } else { "workspace" },
        main {
            if creating() {
                form { class: "create-form",
                    onsubmit: move |event| {
                        event.prevent_default();
                        let project_name = name().trim().to_string();
                        if project_name.is_empty()
                            || project_name == "."
                            || project_name == ".."
                            || project_name.chars().any(|c| c.is_control() || "<>:\"/\\|?*".contains(c))
                            || project_name.ends_with('.')
                        {
                            error.set("Enter a valid folder name.".into());
                            return;
                        }
                        spawn(async move {
                            if let Some(parent) = rfd::AsyncFileDialog::new()
                                .set_title("Choose where to create your project")
                                .pick_folder().await
                            {
                                let path = parent.path().join(project_name);
                                match std::fs::create_dir(&path) {
                                    Ok(()) => {
                                        project.set(Some(path));
                                        creating.set(false);
                                        error.set(String::new());
                                    }
                                    Err(problem) => error.set(format!("Could not create project: {problem}")),
                                }
                            }
                        });
                    },
                    label { r#for: "project-name", "Project name" }
                    input {
                        id: "project-name", autofocus: true, autocomplete: "off",
                        value: "{name}",
                        oninput: move |event| { name.set(event.value()); error.set(String::new()); },
                    }
                    if !error().is_empty() {
                        p { class: "error", role: "alert", "{error}" }
                    }
                    div { class: "actions",
                        button { class: "primary", r#type: "submit", "Choose Location & Create" }
                        button { class: "secondary", r#type: "button",
                            onclick: move |_| { creating.set(false); error.set(String::new()); },
                            "Cancel"
                        }
                    }
                }
            } else if let Some(path) = project.read().as_ref() {
                Constellation { key: "{path.display()}", root: path.clone() }
            } else {
                div { class: "actions",
                    button { class: "primary",
                        onmouseenter: move |_| status.hint.set(Some("Create a new project folder".into())),
                        onmouseleave: move |_| status.hint.set(None),
                        onclick: move |_| { name.set(String::new()); creating.set(true); },
                        "Create Project"
                    }
                    button { class: "secondary",
                        onmouseenter: move |_| status.hint.set(Some("Open an existing project folder".into())),
                        onmouseleave: move |_| status.hint.set(None),
                        onclick: move |_| open_folder(project, creating),
                        "Open Folder"
                    }
                }
            }
        }
        workbench::SidePanel { root: project() }
        }
        footer { class: "status-bar", role: "status",
            "{status.hint.read().as_deref().unwrap_or(&status.path.read())}"
        }
    }
}
