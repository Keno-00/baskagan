use crate::{Status, Workbench, tree::Tree};
use dioxus::prelude::*;
use notify::{RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

#[derive(Deserialize)]
struct Interaction {
    action: String,
    path: Option<PathBuf>,
    text: Option<String>,
    directory: Option<bool>,
}

#[derive(Serialize)]
struct Scene<'a> {
    tree: &'a Tree,
    current: &'a PathBuf,
    parent: Option<PathBuf>,
    overview: bool,
}

#[component]
pub fn Constellation(root: PathBuf) -> Element {
    let mut folder = use_signal(|| root.clone());
    let mut overview = use_signal(|| false);
    let mut revision = use_signal(|| 0u64);
    let mut ready = use_signal(|| false);
    let mut status = use_context::<Status>();
    let mut workbench = use_context::<Workbench>();
    let watch_root = root.clone();
    use_future(move || {
        let root = watch_root.clone();
        async move {
            let dirty = Arc::new(AtomicBool::new(false));
            let changed = dirty.clone();
            let mut watcher =
                notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
                    if event
                        .as_ref()
                        .map(|e| !matches!(e.kind, notify::EventKind::Access(_)))
                        .unwrap_or(true)
                    {
                        changed.store(true, Ordering::Relaxed);
                    }
                })
                .ok();
            let watching = watcher
                .as_mut()
                .is_some_and(|w| w.watch(&root, RecursiveMode::Recursive).is_ok());
            let mut ticks = 0;
            loop {
                tokio::time::sleep(Duration::from_millis(250)).await;
                ticks += 1;
                // Periodic reconciliation also catches dropped OS events or a recreated directory.
                if dirty.swap(false, Ordering::Relaxed)
                    || ticks % if watching { 20 } else { 8 } == 0
                {
                    let mut path = folder.peek().clone();
                    while !path.is_dir() && path != root {
                        if !path.pop() {
                            break;
                        }
                    }
                    if path != *folder.peek() {
                        folder.set(path);
                    }
                    revision += 1;
                }
            }
        }
    });
    let tree = use_resource(move || {
        let path = folder();
        let all = overview();
        let _ = revision();
        async move {
            tokio::task::spawn_blocking(move || Tree::read(&path, all))
                .await
                .ok()
        }
    });

    let navigation_root = root.clone();
    use_future(move || {
        let root = navigation_root.clone();
        async move {
            let source = format!(
                "{}\n{}",
                include_str!("scene_math.js"),
                include_str!("scene.js")
            );
            let mut bridge = document::eval(&source);
            while let Ok(message) = bridge.recv::<Interaction>().await {
                match message.action.as_str() {
                    "ready" => ready.set(true),
                    "hint" => status.hint.set(message.text),
                    "select" => {
                        workbench
                            .selected_file
                            .set(if message.directory == Some(false) {
                                message.path
                            } else {
                                None
                            });
                    }
                    "open_file" => {
                        if let Some(path) = message.path.filter(|p| p.is_file()) {
                            workbench.selected_file.set(Some(path.clone()));
                            workbench.shown.set(true);
                            if let Ok(json) = serde_json::to_string(&path) {
                                let _ = document::eval(&format!(
                                    "window.dadoWorkbench?.openFile({json});"
                                ));
                            }
                        }
                    }
                    "enter" => {
                        if let Some(path) = message
                            .path
                            .filter(|p| p.starts_with(&root) && p.is_dir() && !p.is_symlink())
                        {
                            folder.set(path);
                            workbench.selected_file.set(None);
                            overview.set(false);
                            status.hint.set(None);
                        } else {
                            revision += 1;
                            let _ = document::eval("window.dadoScene?.cancelNavigation();");
                        }
                    }
                    _ => {}
                }
            }
        }
    });

    use_effect(move || {
        status.path.set(folder().display().to_string());
    });
    use_effect(move || {
        if !ready() {
            return;
        }
        let graph = tree.read();
        if let Some(Some(graph)) = graph.as_ref() {
            // Use the scan's path while the next navigation may still be loading.
            let current = &graph.nodes[0].path;
            let scene = Scene {
                tree: graph,
                current,
                parent: current
                    .parent()
                    .map(PathBuf::from)
                    .filter(|p| p.starts_with(&root)),
                overview: overview.peek().to_owned(),
            };
            if let Ok(json) = serde_json::to_string(&scene) {
                let _ = document::eval(&format!("window.dadoScene?.setGraph({json});"));
            }
        }
    });

    rsx! {
        section { class: "constellation",
            nav { class: "icon-controls", aria_label: "View controls",
                button { class: if overview() { "icon-button selected" } else { "icon-button" },
                    aria_label: if overview() { "Show current folder" } else { "Show overview" }, aria_pressed: "{overview}",
                    onmouseenter: move |_| status.hint.set(Some(if overview() { "Show only the current folder's contents" } else { "Show all levels below the current folder" }.into())),
                    onmouseleave: move |_| status.hint.set(None),
                    onfocus: move |_| status.hint.set(Some("Toggle folder view and overview".into())),
                    onblur: move |_| status.hint.set(None), onclick: move |_| overview.toggle(),
                    svg { view_box: "0 0 24 24", "aria-hidden": "true",
                        path { d: "M12 5v5M5 14v-4h14v4M12 10v4" }
                        circle { cx: "12", cy: "3", r: "2" }
                        circle { cx: "5", cy: "17", r: "2" }
                        circle { cx: "12", cy: "17", r: "2" }
                        circle { cx: "19", cy: "17", r: "2" }
                    }
                }
                button { class: "icon-button", aria_label: "Reset view",
                    onmouseenter: move |_| status.hint.set(Some("Reset the view position, rotation and scale".into())),
                    onmouseleave: move |_| status.hint.set(None),
                    onfocus: move |_| status.hint.set(Some("Reset the view position, rotation and scale".into())),
                    onblur: move |_| status.hint.set(None),
                    onclick: move |_| { let _ = document::eval("window.dadoScene?.reset();"); },
                    svg { view_box: "0 0 24 24", "aria-hidden": "true",
                        path { d: "M5 9a7 7 0 1 1-1 6M5 4v5h5" }
                    }
                }
                span { class: "control-divider", aria_hidden: "true" }
                button { class: if (workbench.shown)() { "icon-button selected" } else { "icon-button" },
                    aria_label: "Toggle side panel", aria_pressed: "{workbench.shown}",
                    onmouseenter: move |_| status.hint.set(Some("Show or hide editor and terminal tabs".into())),
                    onmouseleave: move |_| status.hint.set(None),
                    onfocus: move |_| status.hint.set(Some("Show or hide editor and terminal tabs".into())),
                    onblur: move |_| status.hint.set(None), onclick: move |_| workbench.shown.toggle(),
                    svg { view_box: "0 0 24 24", "aria-hidden": "true",
                        rect { x: "4", y: "5", width: "16", height: "14", rx: "3" }
                        path { d: "M14 5v14" }
                    }
                }
            }
            svg { id: "folder-scene", class: "graph", view_box: "-600 -300 1200 650", tabindex: "0", "aria-label": "Folder structure",
                defs {
                    linearGradient { id: "boundary-fade", x1: "0", y1: "0", x2: "0", y2: "1",
                        stop { offset: "0%", stop_color: "black" }
                        stop { offset: "12%", stop_color: "white" }
                        stop { offset: "85%", stop_color: "white" }
                        stop { offset: "100%", stop_color: "black" }
                    }
                    linearGradient { id: "side-fade", x1: "0", y1: "0", x2: "1", y2: "0",
                        stop { offset: "0%", stop_color: "black" }
                        stop { offset: "8%", stop_color: "white" }
                        stop { offset: "92%", stop_color: "white" }
                        stop { offset: "100%", stop_color: "black" }
                    }
                    mask { id: "scene-mask", mask_units: "userSpaceOnUse", x: "-600", y: "-300", width: "1200", height: "650",
                        rect { x: "-600", y: "-300", width: "1200", height: "650", fill: "url(#boundary-fade)" }
                    }
                    mask { id: "side-mask", mask_units: "userSpaceOnUse", x: "-600", y: "-300", width: "1200", height: "650",
                        rect { x: "-600", y: "-300", width: "1200", height: "650", fill: "url(#side-fade)" }
                    }
                }
                g { id: "scene-content", mask: "url(#scene-mask)",
                    g { mask: "url(#side-mask)",
                        g { id: "scene-edges" }
                        g { id: "scene-nodes" }
                    }
                }
            }
            if tree.read().is_none() {
                p { class: "canvas-message", "Reading folder…" }
            } else if let Some(Some(graph)) = tree.read().as_ref() {
                if graph.nodes.len() == 1 && graph.notices.is_empty() {
                    p { class: "empty-message", "This folder is empty" }
                }
                if !graph.notices.is_empty() {
                    details { class: "scan-notices",
                        summary {
                            onmouseenter: move |_| status.hint.set(Some("Some contents are unavailable or exceed the current overview limit".into())),
                            onmouseleave: move |_| status.hint.set(None), "Some contents could not be shown"
                        }
                        for notice in &graph.notices { p { "{notice}" } }
                    }
                }
            } else { p { class: "canvas-message", "Could not read this folder" } }
        }
    }
}
