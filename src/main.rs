#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use dioxus::desktop::{Config, LogicalSize, WindowBuilder};
use dioxus::prelude::*;

mod agent;
mod ai;
mod ai_process;
mod annotations;
mod chat;
mod chat_history;
mod composer;
mod constellation;
mod markdown;
mod model_selector;
mod node_insight;
mod permission_selector;
mod project_model;
mod project_store;
mod sidebar_ui;
mod skill_creator;
mod systems_ui;
mod terminal;
mod titlebar;
mod tree;
mod ui_controls;
mod user_settings;
mod welcome;
mod workbench;
mod workbench_ui;
use constellation::Constellation;

#[derive(Clone, Copy)]
pub struct Status {
    pub hint: Signal<Option<String>>,
    pub path: Signal<String>,
    pub activity: Signal<Option<String>>,
    pub phase: Signal<&'static str>,
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
                    .with_title("ᜊᜐ᜔ᜃᜄᜈ᜔")
                    .with_decorations(false)
                    .with_inner_size(LogicalSize::new(1200.0, 800.0)),
            ),
        )
        .launch(App);
}

#[component]
fn App() -> Element {
    let general = use_context_provider(|| {
        Signal::new(
            user_settings::home()
                .and_then(|home| user_settings::load(&home))
                .map(|(s, _)| s.general)
                .unwrap_or_default(),
        )
    });
    use_context_provider(|| project_model::Understanding {
        rebuild: Signal::new(0),
    });
    let mut project = use_signal(|| {
        welcome::startup_project(
            std::env::args_os().nth(1).map(std::path::PathBuf::from),
            general.peek().reopen_last_project,
        )
    });
    let mut creating = use_signal(|| false);
    let mut name = use_signal(String::new);
    let mut error = use_signal(String::new);
    let left_sidebar = use_signal(|| false);
    let sidebar_state = use_context_provider(|| Signal::new(sidebar_ui::State::default()));
    let mut status = use_context_provider(|| Status {
        hint: Signal::new(None),
        activity: Signal::new(None),
        phase: Signal::new("idle"),
        path: Signal::new(String::new()),
    });
    let mut workbench = use_context_provider(|| Workbench {
        shown: Signal::new(false),
        selected_file: Signal::new(None),
    });
    use_effect(move || {
        if let Some(path) = project.read().as_ref() {
            if let Err(problem) = welcome::remember(path) {
                error.set(problem);
            }
        }
        status.path.set(
            project
                .read()
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_default(),
        );
        status.hint.set(None);
        status.activity.set(None);
        status.phase.set("idle");
        workbench.selected_file.set(None);
    });

    use_effect(move || {
        let phase = (status.phase)();
        let text = (status.activity)();
        if ["done", "paused", "error"].contains(&phase) && text.is_some() {
            spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                if *status.phase.peek() == phase && *status.activity.peek() == text {
                    status.activity.set(None);
                }
            });
        }
    });

    rsx! {
        style { {include_str!("style.css")} }
        style { {include_str!("../assets/workbench.css")} }
        style { {include_str!("ui_controls.css")} }
        titlebar::TitleBar { project, creating, name, error, sidebar:left_sidebar }
        div { class:if left_sidebar() {"app-body sidebar-open"}else{"app-body"}, "data-project": if project.read().is_some() { "open" } else { "closed" },
        aside { id: "left-sidebar", hidden: !left_sidebar(), inert:if sidebar_state.read().settings_open {Some("")}else{None}, aria_label: "Sidebar", sidebar_ui::SidebarUi {} }
        div { inert:if sidebar_state.read().settings_open {Some("")}else{None}, class: if (workbench.shown)() { "workspace panel-open" } else { "workspace" },
        div { id: "systems-chat", class: "systems-chat", "data-systems-hidden": project().is_some() && !creating() && !general().show_systems_view,
        main { class: "systems-stage", "data-activity": (status.phase)(), "data-mode": if project.read().is_some() { "project" } else { "welcome" },
            canvas { id: "systems-atmosphere", aria_hidden: "true",
                onmounted: move |_| { let _ = document::eval(include_str!("atmosphere.js")); }
            }
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
                welcome::Welcome { project, creating, name, error }
            }
        }
        chat::ChatPanel { root: project() }
        }
        workbench::SidePanel { root: project() }
        }
        sidebar_ui::SettingsUi {}
        }
        footer { class: "status-bar", role: "status",
            span { class:"status-context", "{status.hint.read().as_deref().unwrap_or(&status.path.read())}" }
            span { class:"status-activity", "data-state":(status.phase)(), aria_live:"polite", aria_atomic:"true",
                if ["checking", "working"].contains(&(status.phase)()) {
                    span { class:"status-progress", role:"progressbar", aria_label:"Project understanding in progress" }
                }
                if let Some(activity) = (status.activity)() { span { "{activity}" } }
            }
        }
    }
}
