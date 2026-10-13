//! Source-backed project explanations. User notes and layout never enter model output.
use crate::{Status, Workbench, ui_controls::UiButton};
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
pub(crate) fn scene_activity(root: &Path, event: &Value) {
    if dioxus::core::Runtime::try_current().is_none() {
        return;
    }
    let _ = document::eval(&format!(
        "window.dispatchEvent(new CustomEvent('dado-scene-activity',{{detail:{{...{},root:{}}}}}));",
        event,
        json!(root)
    ));
}
use std::{
    collections::{BTreeMap, HashSet},
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Clone, Copy)]
pub struct Understanding {
    pub rebuild: Signal<u64>,
}

fn ignored_inventory_path(path: &str) -> bool {
    path.split(['/', '\\']).any(|name| {
        crate::agent::context::excluded(name)
            || matches!(
                name,
                ".wrangler"
                    | ".next"
                    | ".nuxt"
                    | ".cache"
                    | ".pytest_cache"
                    | "__pycache__"
                    | ".venv"
                    | "venv"
                    | "dist"
                    | "build"
                    | "coverage"
            )
            || name.ends_with(".pyc")
    })
}

fn cached_inventory(model: &Model) -> BTreeMap<String, String> {
    let inventory = if model.inventory.is_empty() {
        &model.revisions
    } else {
        &model.inventory
    };
    inventory
        .iter()
        .filter(|(p, _)| !ignored_inventory_path(p))
        .map(|(p, r)| (p.clone(), r.clone()))
        .collect()
}

// Names are a user's learned vocabulary, not fresh wording for each model run.
fn preserve_identity(old: &Model, next: &mut Model) {
    fn anchors(c: &Concept) -> Vec<(&str, &str, Option<&str>)> {
        let mut sources: Vec<_> = c
            .sources
            .iter()
            .map(|s| (s.path.as_str(), s.kind.as_str(), s.symbol.as_deref()))
            .collect();
        sources.sort_unstable();
        sources
    }
    let mut mapping = BTreeMap::new();
    let mut claimed: HashSet<String> = next
        .concepts
        .iter()
        .filter(|c| old.concepts.iter().any(|o| o.id == c.id))
        .map(|c| c.id.clone())
        .collect();
    for c in &next.concepts {
        if old.concepts.iter().any(|o| o.id == c.id) || c.sources.is_empty() {
            continue;
        }
        let sources = anchors(c);
        let matches: Vec<_> = old
            .concepts
            .iter()
            .filter(|o| anchors(o) == sources && o.parent.is_none() == c.parent.is_none())
            .collect();
        if matches.len() == 1
            && next
                .concepts
                .iter()
                .filter(|n| anchors(n) == sources)
                .count()
                == 1
            && claimed.insert(matches[0].id.clone())
        {
            mapping.insert(c.id.clone(), matches[0].id.clone());
        }
    }
    for c in &mut next.concepts {
        if let Some(id) = mapping.get(&c.id) {
            c.id = id.clone();
        }
        if let Some(parent) = c.parent.as_mut() {
            if let Some(id) = mapping.get(parent) {
                *parent = id.clone();
            }
        }
        if let Some(existing) = old.concepts.iter().find(|o| o.id == c.id) {
            c.name = existing.name.clone();
        }
    }
    for e in &mut next.connections {
        if let Some(id) = mapping.get(&e.from) {
            e.from = id.clone();
        }
        if let Some(id) = mapping.get(&e.to) {
            e.to = id.clone();
        }
    }
}

fn validate_continuity(root: &Path, old: &Model, next: &Model) -> Result<(), String> {
    for c in &old.concepts {
        if next.concepts.iter().any(|n| n.id == c.id) {
            continue;
        }
        let still_present = c.sources.is_empty()
            || c.sources.iter().any(|source| {
                let Ok(path) = source_path(root, source) else {
                    return false;
                };
                if let Some(symbol) = &source.symbol {
                    let name = symbol
                        .rsplit("::")
                        .next()
                        .unwrap_or(symbol)
                        .rsplit('.')
                        .next()
                        .unwrap_or(symbol);
                    std::fs::read_to_string(path).is_ok_and(|text| {
                        text.split(|c: char| !c.is_alphanumeric() && c != '_')
                            .any(|token| token == name)
                    })
                } else {
                    true
                }
            });
        if still_present {
            return Err(format!(
                "Existing concept {} ({}) is missing while its sources remain. Retain its ID and name; update its explanation and references rather than replacing the concept. Only remove concepts whose supporting responsibility was removed.",
                c.id, c.name
            ));
        }
    }
    Ok(())
}

fn preserve_unaffected(old: &Model, next: &mut Model, changed: &[String]) {
    for c in &old.concepts {
        let affected = c.sources.is_empty()
            || c.sources.iter().any(|s| {
                changed.iter().any(|p| {
                    p == &s.path
                        || (s.kind == "folder"
                            && (s.path == "." || p.starts_with(&format!("{}/", s.path))))
                })
            });
        if !affected {
            if let Some(existing) = next.concepts.iter_mut().find(|n| n.id == c.id) {
                *existing = c.clone();
            } else {
                next.concepts.push(c.clone());
            }
        }
    }
    // Keep ancestors of retained concepts, even if an incremental draft omits them.
    for _ in 0..old.concepts.len() {
        let missing: HashSet<_> = next
            .concepts
            .iter()
            .filter_map(|c| c.parent.clone())
            .filter(|p| !next.concepts.iter().any(|n| &n.id == p))
            .collect();
        if missing.is_empty() {
            break;
        }
        let before = next.concepts.len();
        next.concepts.extend(
            old.concepts
                .iter()
                .filter(|c| missing.contains(&c.id))
                .cloned(),
        );
        if before == next.concepts.len() {
            break;
        }
    }
    for e in &old.connections {
        if next.concepts.iter().any(|c| c.id == e.from)
            && next.concepts.iter().any(|c| c.id == e.to)
            && !next
                .connections
                .iter()
                .any(|n| n.from == e.from && n.to == e.to && n.kind == e.kind)
            && old
                .concepts
                .iter()
                .filter(|c| c.id == e.from || c.id == e.to)
                .all(|c| next.concepts.iter().any(|n| n == c))
        {
            next.connections.push(e.clone());
        }
    }
}

#[derive(Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Model {
    pub version: u32,
    pub concepts: Vec<Concept>,
    pub connections: Vec<Connection>,
    #[serde(default)]
    pub revisions: BTreeMap<String, String>,
    #[serde(default)]
    pub inventory: BTreeMap<String, String>,
    #[serde(default)]
    pub analysis_version: u32,
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct Concept {
    pub id: String,
    pub name: String,
    pub description: String,
    pub parent: Option<String>,
    pub sources: Vec<Source>,
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct Source {
    pub path: String,
    pub kind: String,
    pub symbol: Option<String>,
    pub start: Option<usize>,
    pub end: Option<usize>,
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct Connection {
    pub from: String,
    pub to: String,
    pub kind: String,
    pub description: String,
    pub uncertain: bool,
}

pub(crate) fn source_path(root: &Path, source: &Source) -> Result<PathBuf, String> {
    let p = Path::new(&source.path);
    if p.is_absolute()
        || p.components().any(|c| {
            !matches!(
                c,
                std::path::Component::Normal(_) | std::path::Component::CurDir
            )
        })
    {
        return Err(format!("Invalid relative path: {}", source.path));
    }
    let path = root.join(p);
    let canonical = path
        .canonicalize()
        .map_err(|_| format!("File not found: {}", source.path))?;
    if !canonical.starts_with(root.canonicalize().map_err(|e| e.to_string())?)
        || p.components()
            .any(|c| crate::agent::context::excluded(&c.as_os_str().to_string_lossy()))
    {
        return Err("Source outside project or inside generated storage".into());
    }
    Ok(path)
}
pub fn validate(root: &Path, model: &mut Model) -> Result<(), String> {
    if model.version != 1
        || model.concepts.is_empty()
        || model.concepts.len() > 2000
        || model.connections.len() > 8000
    {
        return Err("Expected version 1, 1–2000 concepts and at most 8000 connections".into());
    }
    let ids: HashSet<_> = model.concepts.iter().map(|c| c.id.clone()).collect();
    if ids.len() != model.concepts.len()
        || model.concepts.iter().filter(|c| c.parent.is_none()).count() != 1
    {
        return Err("Concept IDs must be unique with exactly one project root".into());
    }
    model.revisions.clear();
    let mut source_errors = Vec::new();
    for c in &model.concepts {
        if c.id.is_empty()
            || c.id.len() > 100
            || !c
                .id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
            || c.name.is_empty()
            || c.name.len() > 120
            || c.description.len() > 8000
            || c.sources.len() > 24
        {
            return Err(format!("Invalid concept fields: {}", c.id));
        }
        let mut seen = HashSet::new();
        let mut parent = Some(c.id.as_str());
        while let Some(id) = parent {
            if !seen.insert(id) {
                return Err(format!("Hierarchy cycle: {}", c.id));
            }
            let node = model
                .concepts
                .iter()
                .find(|n| n.id == id)
                .ok_or(format!("Missing parent: {id}"))?;
            parent = node.parent.as_deref();
        }
        for s in &c.sources {
            let checked = (|| -> Result<(), String> {
                let p = source_path(root, s)?;
                if !["file", "folder"].contains(&s.kind.as_str())
                    || (s.kind == "folder") != p.is_dir()
                {
                    return Err(format!("Wrong source type: {}", s.path));
                }
                if s.start.is_some() != s.end.is_some() || (s.kind == "folder" && s.start.is_some())
                {
                    return Err(format!("Invalid range: {}", s.path));
                }
                if p.is_file() {
                    let bytes = std::fs::read(&p).map_err(|e| e.to_string())?;
                    if let (Some(a), Some(b)) = (s.start, s.end) {
                        let text = std::str::from_utf8(&bytes)
                            .map_err(|_| format!("Non-text range: {}", s.path))?;
                        if a == 0 || b < a || b > text.lines().count() {
                            return Err(format!(
                                "Out of range: {} L{a}-{b} ({} lines)",
                                s.path,
                                text.lines().count()
                            ));
                        }
                        if let Some(symbol) = &s.symbol {
                            let name = symbol
                                .rsplit("::")
                                .next()
                                .unwrap_or(symbol)
                                .rsplit('.')
                                .next()
                                .unwrap_or(symbol);
                            if name.is_empty()
                                || !name.chars().all(|c| c.is_alphanumeric() || c == '_')
                            {
                                return Err(format!(
                                    "Expected one symbol name for {}; use separate references or null for broad source sections",
                                    s.path
                                ));
                            }
                            if !text.lines().skip(a - 1).take(b - a + 1).any(|l| {
                                l.split(|c: char| !c.is_alphanumeric() && c != '_')
                                    .any(|token| token == name)
                            }) {
                                return Err(format!(
                                    "Symbol {symbol} missing from {} L{a}-{b}",
                                    s.path
                                ));
                            }
                        }
                    }
                    model
                        .revisions
                        .insert(s.path.clone(), crate::agent::snapshot::revision(&bytes));
                }
                Ok(())
            })();
            if let Err(error) = checked {
                source_errors.push(format!("{}: {error}", c.id));
            }
        }
    }
    if !source_errors.is_empty() {
        return Err(source_errors
            .into_iter()
            .take(24)
            .collect::<Vec<_>>()
            .join("\n"));
    }
    for e in &model.connections {
        if !ids.contains(&e.from)
            || !ids.contains(&e.to)
            || !["calls", "data", "triggers", "depends", "relates"].contains(&e.kind.as_str())
            || e.description.len() > 2000
        {
            return Err(format!(
                "Invalid connection: {} → {} ({})",
                e.from, e.to, e.kind
            ));
        }
    }
    Ok(())
}
pub(crate) fn inventory(root: &Path) -> Result<BTreeMap<String, String>, String> {
    fn visit(
        root: &Path,
        dir: &Path,
        out: &mut BTreeMap<String, String>,
        depth: usize,
    ) -> Result<(), String> {
        if depth > 40 || out.len() >= 6000 {
            return Err("Project inventory exceeds the supported scan limit".into());
        }
        let entries = std::fs::read_dir(dir)
            .map_err(|e| format!("Could not check {}: {e}", dir.display()))?;
        for e in entries {
            let e = e.map_err(|e| e.to_string())?;
            let p = e.path();
            let name = e.file_name().to_string_lossy().into_owned();
            if crate::project_store::linked(&p) || ignored_inventory_path(&name) {
                continue;
            }
            if p.is_dir() {
                visit(root, &p, out, depth + 1)?;
            } else if e.metadata().map_err(|e| e.to_string())?.len() <= 1024 * 1024 {
                {
                    let bytes = std::fs::read(&p)
                        .map_err(|e| format!("Could not check {}: {e}", p.display()))?;
                    out.insert(
                        p.strip_prefix(root)
                            .unwrap()
                            .to_string_lossy()
                            .replace('\\', "/"),
                        crate::agent::snapshot::revision(&bytes),
                    );
                }
            }
        }
        Ok(())
    }
    let mut out = BTreeMap::new();
    visit(root, root, &mut out, 0)?;
    Ok(out)
}
fn model_file(root: &Path) -> Result<PathBuf, String> {
    let p = crate::project_store::storage(root)?.join("systems.json");
    if crate::project_store::linked(&p) {
        return Err("Systems storage cannot follow links".into());
    }
    Ok(p)
}
fn save(root: &Path, model: &Model) -> Result<(), String> {
    let path = model_file(root)?;
    let mut temp =
        tempfile::NamedTempFile::new_in(path.parent().unwrap()).map_err(|e| e.to_string())?;
    serde_json::to_writer_pretty(&mut temp, model).map_err(|e| e.to_string())?;
    temp.as_file().sync_all().map_err(|e| e.to_string())?;
    temp.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}

pub(crate) fn output_schema() -> Value {
    fn object(properties: Value) -> Value {
        let required = properties
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        json!({"type":"object","additionalProperties":false,"required":required,"properties":properties})
    }
    let text = json!({"type":"string"});
    let nullable = json!({"type":["string","null"]});
    let whole = object(
        json!({"path":text,"kind":{"type":"string","enum":["file","folder"]},"symbol":{"type":"null"},"start":{"type":"null"},"end":{"type":"null"}}),
    );
    let range = object(
        json!({"path":text,"kind":{"type":"string","enum":["file"]},"symbol":nullable,"start":{"type":"integer"},"end":{"type":"integer"}}),
    );
    let concept = object(
        json!({"id":text,"name":text,"description":text,"parent":nullable,"sources":{"type":"array","items":{"anyOf":[whole,range]}}}),
    );
    let connection = object(
        json!({"from":text,"to":text,"kind":{"type":"string","enum":["calls","data","triggers","depends","relates"]},"description":text,"uncertain":{"type":"boolean"}}),
    );
    object(
        json!({"version":{"type":"integer","enum":[1]},"concepts":{"type":"array","items":concept},"connections":{"type":"array","items":connection}}),
    )
}

#[derive(Clone, PartialEq)]
struct Draft {
    model: Model,
    arrived: Vec<String>,
}

// Read only complete objects from streamed top-level arrays. Braces inside
// descriptions and escaped quotes must never be mistaken for JSON boundaries.
fn streaming_model(text: &str, old: &Model) -> Option<Draft> {
    if text.len() > 2_000_000 {
        return None;
    }
    let mut depth = 0usize;
    let mut quoted = false;
    let mut escaped = false;
    let mut string_start = 0;
    let mut key = String::new();
    let mut array = String::new();
    let mut start = None;
    let mut concepts = Vec::<Concept>::new();
    let mut connections = Vec::<Connection>::new();
    for (i, b) in text.bytes().enumerate() {
        if quoted {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                quoted = false;
                if depth == 1 {
                    key = text[string_start..i].to_string();
                }
            }
            continue;
        }
        match b {
            b'"' => {
                quoted = true;
                string_start = i + 1;
            }
            b'[' | b'{' => {
                if depth == 1 && b == b'[' {
                    array = key.clone();
                }
                if depth == 2 && b == b'{' {
                    start = Some(i);
                }
                depth += 1;
            }
            b']' | b'}' => {
                if depth == 3 && b == b'}' {
                    if let Some(begin) = start.take() {
                        let raw = &text[begin..=i];
                        if array == "concepts" {
                            if let Ok(c) = serde_json::from_str::<Concept>(raw) {
                                concepts.push(c);
                            }
                        } else if array == "connections" {
                            if let Ok(c) = serde_json::from_str::<Connection>(raw) {
                                connections.push(c);
                            }
                        }
                    }
                }
                depth = depth.saturating_sub(1);
            }
            _ => {}
        }
    }
    if concepts.is_empty() && connections.is_empty() {
        return None;
    }
    let mut incoming = Model {
        concepts,
        connections,
        ..Default::default()
    };
    preserve_identity(old, &mut incoming);
    let concepts = incoming.concepts;
    let connections = incoming.connections;
    let arrived = concepts.iter().map(|c| c.id.clone()).collect();
    let mut result = old.clone();
    result.inventory.clear();
    result.revisions.clear();
    for c in concepts.into_iter().take(2000) {
        if c.id.is_empty() || c.id.len() > 120 || c.name.len() > 240 {
            continue;
        }
        if let Some(existing) = result.concepts.iter_mut().find(|n| n.id == c.id) {
            *existing = c;
        } else {
            result.concepts.push(c);
        }
    }
    // Keep a single rooted, acyclic hierarchy even if children arrive first.
    let root = old
        .concepts
        .iter()
        .find(|c| c.parent.is_none())
        .or_else(|| result.concepts.iter().find(|c| c.parent.is_none()))?
        .id
        .clone();
    let mut included = HashSet::from([root.clone()]);
    for _ in 0..result.concepts.len() {
        let before = included.len();
        for c in &result.concepts {
            if c.parent.as_ref().is_some_and(|p| included.contains(p)) {
                included.insert(c.id.clone());
            }
        }
        if included.len() == before {
            break;
        }
    }
    result
        .concepts
        .retain(|c| included.contains(&c.id) && (c.id == root || c.parent.is_some()));
    for c in connections.into_iter().take(8000) {
        if !result
            .connections
            .iter()
            .any(|e| e.from == c.from && e.to == c.to && e.kind == c.kind)
        {
            result.connections.push(c);
        }
    }
    result
        .connections
        .retain(|e| included.contains(&e.from) && included.contains(&e.to));
    Some(Draft {
        model: result,
        arrived,
    })
}

async fn generate(
    root: &Path,
    old: &Model,
    changed: &[String],
    settings: &crate::user_settings::Settings,
    mut activity: Signal<Option<String>>,
    mut draft: Signal<Option<Draft>>,
    general: Signal<crate::user_settings::General>,
    manual: bool,
) -> Result<Model, String> {
    let client = crate::ai::Client::start();
    generate_with_progress(
        root,
        old,
        changed,
        settings,
        client,
        |text| activity.set(Some(text)),
        |m| draft.set(m),
        || {
            manual
                || (general.peek().show_systems_view
                    && general.peek().automatic_project_understanding)
        },
    )
    .await
}
#[cfg(test)]
async fn generate_with_client(
    root: &Path,
    old: &Model,
    changed: &[String],
    settings: &crate::user_settings::Settings,
    client: crate::ai::Client,
) -> Result<Model, String> {
    generate_with_progress(
        root,
        old,
        changed,
        settings,
        client,
        |_| {},
        |_| {},
        || true,
    )
    .await
}
async fn generate_with_progress(
    root: &Path,
    old: &Model,
    changed: &[String],
    settings: &crate::user_settings::Settings,
    client: crate::ai::Client,
    mut progress: impl FnMut(String),
    mut draft: impl FnMut(Option<Draft>),
    mut enabled: impl FnMut() -> bool,
) -> Result<Model, String> {
    let started = std::time::Instant::now();
    let mut elapsed = u64::MAX;
    let mut phase = "reading project sources";
    let inventory_root = root.to_path_buf();
    let files = tokio::task::spawn_blocking(move || inventory(&inventory_root))
        .await
        .map_err(|e| e.to_string())??;
    let scope = if old.concepts.is_empty() {
        "Explain the entire application. First read the README, manifest, entry point and major modules. Use the project name for the root. Build breadth first across all user-facing responsibilities, supporting subsystems, external inputs/outputs and lifecycle stages. Expand siblings before drilling into one branch. Inspect cross-module callers and data movement for evidence-backed correlations. Audit major modules for missing responsibilities before returning. Include representative concrete functions with verified ranges."
    } else {
        "The saved model is authoritative. This is an incremental update, not a fresh project analysis. Read changed paths and relevant callers/dependencies. Return the complete existing graph with only affected descriptions, source references and relationships updated. Add genuinely new responsibilities supported by the changes, keeping the existing hierarchy and terminology. Do not audit and redesign the whole project. Preserve IDs and names verbatim, including during validation corrections."
    };
    let mut explanation = old.clone();
    explanation.inventory.clear();
    explanation.revisions.clear();
    let listing = files
        .keys()
        .take(3000)
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    let mut orientation = String::new();
    for path in [
        "README.md",
        "Cargo.toml",
        "package.json",
        "pyproject.toml",
        "src/main.rs",
    ] {
        if files.contains_key(path) {
            if let Ok(text) = std::fs::read_to_string(root.join(path)) {
                orientation.push_str(&format!(
                    "\n--- {path} ---\n{}",
                    text.chars().take(12000).collect::<String>()
                ));
            }
        }
    }
    client.send(json!({"action":"ai-project","root":root,"generation":1}));
    let mut session = Value::Null;
    let mut correction = String::new();
    let mut last_error = String::new();
    for attempt in 0..3 {
        draft(None);
        let mut emitted = 0;
        let mut last_preview = started;
        let mut text = String::new();
        let mut request = String::new();
        let deadline = std::time::Instant::now() + Duration::from_secs(240);
        loop {
            if !enabled() {
                return Err("Project understanding paused".into());
            }
            let seconds = started.elapsed().as_secs();
            if seconds != elapsed {
                elapsed = seconds;
                progress(format!(
                    "Updating understanding · {seconds}s · {phase}{}",
                    if attempt > 0 {
                        format!(" · correction {attempt}/2")
                    } else {
                        String::new()
                    }
                ));
            }
            while let Ok(e) = client.events.try_recv() {
                if e["kind"] == "scene_activity" {
                    scene_activity(root, &e);
                }
                if e["kind"] == "project" {
                    session = e.clone();
                }
                if e["requestId"] == request && !request.is_empty() {
                    if e["kind"] == "delta" {
                        phase = "building the concept model";
                        text.push_str(e["delta"].as_str().unwrap_or(""));
                    }
                    if e["kind"] == "message" {
                        text = e["text"].as_str().unwrap_or(&text).to_string();
                    }
                    if e["kind"] == "completed" {
                        if e["status"] != "completed" {
                            return Err(e["error"]
                                .as_str()
                                .unwrap_or("Background analysis failed")
                                .into());
                        }
                        let raw = text
                            .trim()
                            .trim_start_matches("```json")
                            .trim_start_matches("```")
                            .trim_end_matches("```")
                            .trim();
                        let parsed = serde_json::from_str::<Model>(raw)
                            .map_err(|e| e.to_string())
                            .and_then(|mut m| {
                                preserve_identity(old, &mut m);
                                preserve_unaffected(old, &mut m, changed);
                                validate_continuity(root, old, &m)?;
                                explanation = m.clone();
                                validate(root, &mut m)?;
                                Ok(m)
                            });
                        match parsed {
                            Ok(mut m) => {
                                m.inventory = files.clone();
                                m.analysis_version = 3;
                                return Ok(m);
                            }
                            Err(e) => {
                                last_error = e.clone();
                                correction = e;
                                let errors = correction.clone();
                                let mut shown = HashSet::new();
                                for source in explanation.concepts.iter().flat_map(|c| &c.sources) {
                                    if !errors.contains(&source.path)
                                        || !shown.insert(source.path.clone())
                                    {
                                        continue;
                                    }
                                    if let Ok(path) = source_path(root, source) {
                                        if let Ok(contents) = std::fs::read_to_string(path) {
                                            let total = contents.lines().count();
                                            let start = source
                                                .start
                                                .unwrap_or(1)
                                                .saturating_sub(30)
                                                .max(1)
                                                .min(total.max(1));
                                            let excerpt = contents
                                                .lines()
                                                .enumerate()
                                                .skip(start - 1)
                                                .take(100)
                                                .map(|(i, line)| format!("{}: {line}", i + 1))
                                                .collect::<Vec<_>>()
                                                .join("\n");
                                            correction.push_str(&format!("\nVerified {} ({total} lines; both bounds null for a whole file):\n{}\n",source.path,excerpt.chars().take(10000).collect::<String>()));
                                        }
                                    }
                                    if correction.len() > 80000 {
                                        break;
                                    }
                                }
                                phase = "correcting source references";
                                break;
                            }
                        }
                    }
                }
                if e["kind"] == "state" && e["error"].is_string() {
                    return Err(e["error"].as_str().unwrap().into());
                }
            }
            if text.len() != emitted && last_preview.elapsed() >= Duration::from_millis(200) {
                if let Some(m) = streaming_model(&text, old) {
                    draft(Some(m));
                }
                emitted = text.len();
                last_preview = std::time::Instant::now();
            }
            if !correction.is_empty() && !text.is_empty() {
                break;
            }
            if request.is_empty() && !session.is_null() {
                request = uuid::Uuid::new_v4().to_string();
                let prompt = format!(
                    "Build a source-backed mental model and system workflows for this project. Read project sources using read-only tools. Return ONLY JSON, no Markdown. Schema: {{\"version\":1,\"concepts\":[{{\"id\":\"stable-id\",\"name\":\"Short concept\",\"description\":\"Purpose in plain language\",\"parent\":null,\"sources\":[{{\"path\":\"relative/path\",\"kind\":\"file or folder\",\"symbol\":null,\"start\":null,\"end\":null}}]}}],\"connections\":[{{\"from\":\"id\",\"to\":\"id\",\"kind\":\"calls or data or triggers or depends or relates\",\"description\":\"Evidence-backed relationship\",\"uncertain\":false}}]}}. {} Give new concepts short names (ideally 2–4 words); put explanation in descriptions. Exactly one root: project and purpose, source folder path '.'; descend through responsibilities and implementation concepts into functions with exact 1-based inclusive ranges. For whole-file sources both start and end MUST be null; for a range both MUST be integers, never supply only one bound. Read the actual lines before specifying ranges. A symbol is ONE declaration name (qualified names such as Tree::read are accepted); use null for a broad section or whole file. Never combine multiple names in a symbol string. If you cannot verify a function range, use a verified whole-file reference rather than inventing a range. Concepts are NOT the folder tree. Sources become more precise with depth. Include meaningful workflow edges; never invent a workflow when none exists. Use uncertainty for inferred edges. TERMINOLOGY STABILITY IS REQUIRED: preserve existing concept IDs AND names verbatim. Do not rename, rephrase, reorganize or replace established concepts for stylistic reasons. This is an incremental update when a previous model exists: inspect changed paths and their callers, revise only affected descriptions, source ranges and relationships; preserve unaffected concepts and connections verbatim. Add concepts only for new responsibilities supported by source changes; remove concepts only when their responsibility was removed. A changed model setting is not permission to redesign the mental model. Do not edit files. Do not treat user ideas as implemented behavior. Use as many concepts as source-backed coverage requires, up to the validation safety limit of 2000; do not stop at an arbitrary initial concept count. Keep each description under 150 words. Emit the root and broad responsibility concepts first, then implementation concepts, so the graph can form progressively. Previous model: {}\nChanged paths: {}\nValidation feedback (correct only the invalid draft references below; do not rebuild valid concepts): {}\nAttempt {}\nProject file inventory:\n{}\nOrientation sources:\n{}",
                    scope,
                    serde_json::to_string(&explanation).unwrap(),
                    json!(changed),
                    correction,
                    attempt + 1,
                    listing,
                    orientation
                );
                correction.clear();
                client.send(json!({"action":"ai-send","root":root,"projectId":session["projectId"],"generation":session["generation"],"requestId":request,"chat":"systems-background","mode":"inspect","model":settings.small_model,"effort":settings.small_reasoning_effort,"text":prompt,"attachments":[],"history":[],"context":{},"outputSchema":output_schema()}));
            }
            if std::time::Instant::now() > deadline {
                return Err("Background analysis timed out".into());
            }
            tokio::time::sleep(Duration::from_millis(40)).await;
        }
    }
    Err(format!("Background model needs correction: {last_error}"))
}

#[component]
pub fn ProjectViews(
    root: PathBuf,
    view: Signal<String>,
    overview: Signal<bool>,
    ready: Signal<bool>,
) -> Element {
    let general = use_context::<Signal<crate::user_settings::General>>();
    let mut model = use_signal(Model::default);
    let mut draft = use_signal(|| None::<Draft>);
    let mut problem = use_signal(String::new);
    let mut busy = use_signal(|| false);
    let understanding = use_context::<Understanding>();
    let retry = understanding.rebuild;
    let mut inspecting = use_signal(|| false);
    let mut focus = use_signal(String::new);
    let mut selected = use_signal(|| None::<String>);
    let mut preview = use_signal(|| None::<String>);
    use_future(move || async move {
        let mut bridge =
            document::eval("window.addEventListener('dado-concept',e=>dioxus.send(e.detail));");
        while let Ok(event) = bridge.recv::<Value>().await {
            let id = event["id"].as_str().map(str::to_string);
            match event["action"].as_str() {
                Some("enter") => {
                    if let Some(id) = id {
                        focus.set(id);
                        selected.set(None);
                        preview.set(None);
                    }
                }
                Some("select") => {
                    selected.set(id);
                }
                Some("preview") => {
                    // Keep the inspector available while moving from a node to its sources.
                    if id.is_some() {
                        preview.set(id);
                    }
                }
                _ => {}
            }
        }
    });
    let mut workbench = use_context::<Workbench>();
    let mut status = use_context::<Status>();
    let worker_root = root.clone();
    use_future(move || {
        let root = worker_root.clone();
        async move {
            match model_file(&root).and_then(|p| {
                if p.exists() {
                    serde_json::from_slice::<Model>(&std::fs::read(p).map_err(|e| e.to_string())?)
                        .map(Some)
                        .map_err(|e| format!("Could not read systems model: {e}"))
                } else {
                    Ok(None)
                }
            }) {
                Ok(Some(m)) => {
                    model.set(m);
                }
                Ok(None) => {}
                Err(e) => {
                    problem.set(e);
                    return;
                }
            }
            let mut previous = cached_inventory(&model.peek());
            let mut first_scan = true;
            let mut was_paused = false;
            let mut scan_error = String::new();
            let mut previous_settings = crate::user_settings::home()
                .and_then(|p| crate::user_settings::load(&p))
                .ok()
                .map(|s| (s.0.small_model, s.0.small_reasoning_effort));
            let mut retried = *retry.peek();
            let mut failed = false;
            loop {
                if first_scan {
                    status.phase.set("checking");
                    status.activity.set(Some("Checking for changes…".into()));
                }
                let scan_root = root.clone();
                let scan = match tokio::task::spawn_blocking(move || inventory(&scan_root))
                    .await
                    .map_err(|e| e.to_string())
                    .and_then(|scan| scan)
                {
                    Ok(scan) => scan,
                    Err(error) => {
                        if scan_error != error {
                            status.phase.set("error");
                            status.activity.set(Some(error.clone()));
                            scan_error = error;
                        }
                        tokio::time::sleep(Duration::from_secs(3)).await;
                        continue;
                    }
                };
                scan_error.clear();
                let settings = crate::user_settings::home()
                    .and_then(|p| crate::user_settings::load(&p))
                    .map(|s| s.0);
                if first_scan {
                    if !model.peek().concepts.is_empty()
                        && model.peek().inventory.is_empty()
                        && previous.iter().all(|(p, r)| scan.get(p) == Some(r))
                    {
                        previous = scan.clone();
                        let mut cached = model.peek().clone();
                        cached.inventory = scan.clone();
                        if let Err(e) = save(&root, &cached) {
                            problem.set(e);
                        } else {
                            model.set(cached);
                        }
                    }
                    status.phase.set("idle");
                    status.activity.set(None);
                    first_scan = false;
                }
                if let Ok(settings) = settings {
                    let choice = (
                        settings.small_model.clone(),
                        settings.small_reasoning_effort.clone(),
                    );
                    let manual = *retry.peek() != retried;
                    if (manual
                        || (general.peek().show_systems_view
                            && general.peek().automatic_project_understanding))
                        && (scan != previous
                            || model.peek().concepts.is_empty()
                                && previous_settings.as_ref() != Some(&choice)
                            || manual)
                    {
                        retried = *retry.peek();
                        let changed = scan
                            .keys()
                            .chain(previous.keys())
                            .filter(|p| scan.get(*p) != previous.get(*p))
                            .cloned()
                            .collect::<HashSet<_>>()
                            .into_iter()
                            .collect::<Vec<_>>();
                        previous = scan;
                        previous_settings = Some(choice);
                        failed = false;
                        if settings.small_model.is_some() {
                            busy.set(true);
                            status.phase.set("working");
                            let old = if manual {
                                Model::default()
                            } else {
                                model.peek().clone()
                            };
                            let mut completed = false;
                            let mut paused = false;
                            let updating = model
                                .peek()
                                .concepts
                                .iter()
                                .filter(|c| {
                                    manual
                                        || c.sources.iter().any(|s| {
                                            changed.iter().any(|p| {
                                                p == &s.path
                                                    || s.kind == "folder"
                                                        && (s.path == "."
                                                            || p.starts_with(&format!(
                                                                "{}/",
                                                                s.path
                                                            )))
                                            })
                                        })
                                })
                                .map(|c| c.id.clone())
                                .collect::<Vec<_>>();
                            scene_activity(
                                &root,
                                &json!({"action":"updating","nodes":updating,"status":"running"}),
                            );
                            match generate(
                                &root,
                                &old,
                                &changed,
                                &settings,
                                status.activity,
                                draft,
                                general,
                                manual,
                            )
                            .await
                            {
                                Ok(m) => {
                                    let check_root = root.clone();
                                    let now =
                                        tokio::task::spawn_blocking(move || inventory(&check_root))
                                            .await
                                            .map_err(|e| e.to_string())
                                            .and_then(|scan| scan);
                                    if let Err(e) = &now {
                                        problem.set(e.clone());
                                    }
                                    if now.is_ok_and(|now| now == previous) {
                                        match save(&root, &m) {
                                            Ok(()) => {
                                                model.set(m);
                                                completed = true;
                                                problem.set(String::new());
                                            }
                                            Err(e) => problem.set(e),
                                        }
                                    } else {
                                        previous.clear();
                                    }
                                }
                                Err(e) if e == "Project understanding paused" => {
                                    paused = true;
                                    previous.clear();
                                }
                                Err(e) => {
                                    problem.set(e);
                                    failed = true;
                                }
                            }
                            draft.set(None);
                            scene_activity(
                                &root,
                                &json!({"action":if completed {"updated"} else {"stopped"},"nodes":updating,"status":"completed"}),
                            );
                            busy.set(false);
                            status.phase.set(if !problem.peek().is_empty() {
                                "error"
                            } else if paused {
                                "paused"
                            } else if completed {
                                "done"
                            } else {
                                "checking"
                            });
                            status.activity.set(Some(if !problem.peek().is_empty() {
                                format!("Update failed · {}", problem.peek())
                            } else if paused {
                                "Understanding paused".into()
                            } else if completed {
                                "Up to date".into()
                            } else {
                                "Source changed; checking again…".into()
                            }));
                        } else {
                            problem.set("Choose a small model in Model Settings to build project explanations.".into());
                            status.phase.set("error");
                            status.activity.set(Some(problem.peek().clone()));
                        }
                    }
                } else if !failed {
                    problem.set("Could not load small-model settings.".into());
                    failed = true;
                }
                tokio::time::sleep(Duration::from_secs(3)).await;
                let paused_now = !general.peek().show_systems_view
                    || !general.peek().automatic_project_understanding;
                if paused_now {
                    if !was_paused
                        && problem.peek().is_empty()
                        && !["working", "checking"].contains(&*status.phase.peek())
                    {
                        status.phase.set("paused");
                        status.activity.set(Some("Understanding paused".into()));
                    }
                } else if ["done", "paused"].contains(&*status.phase.peek()) {
                    status.phase.set("idle");
                    status.activity.set(None);
                }
                was_paused = paused_now;
            }
        }
    });
    let publish_root = root.clone();
    use_effect(move || {
        let m = model();
        let root = publish_root.clone();
        let _ = document::eval(&format!(
            "window.dadoProjectModel={{root:{},model:{}}};window.dadoSidebar?.syncModel(window.dadoProjectModel);window.dadoConceptNotes?.bind({});",
            json!(root),
            json!(m),
            json!(root)
        ));
    });
    let m = model();
    let current = m
        .concepts
        .iter()
        .find(|c| c.id == focus())
        .or_else(|| m.concepts.iter().find(|c| c.parent.is_none()))
        .cloned();
    let nodes: Vec<Concept> = if let Some(c) = &current {
        std::iter::once(c.clone())
            .chain(
                m.concepts
                    .iter()
                    .filter(|n| n.parent.as_deref() == Some(&c.id))
                    .cloned(),
            )
            .collect()
    } else {
        vec![]
    };
    let detail = selected()
        .or_else(|| preview())
        .and_then(|id| m.concepts.iter().find(|c| c.id == id).cloned());
    use_effect(move || {
        if !ready() {
            return;
        }
        let _ = document::eval(&format!(
            "window.dadoScene?.setConceptGraph({},{},{},{},{},{});window.dadoScene?.setMode({});",
            json!(draft().map(|d| d.model).unwrap_or_else(|| model())),
            json!(focus()),
            json!(view()),
            json!(overview()),
            json!(if draft().is_some() {
                Some(
                    model()
                        .concepts
                        .iter()
                        .map(|c| c.id.clone())
                        .collect::<Vec<_>>(),
                )
            } else {
                None
            }),
            json!(draft().map(|d| d.arrived)),
            json!(view())
        ));
    });
    rsx! {
        style {{include_str!("project_model.css")}}
        crate::node_insight::NodeInsight {root:root.clone(),model,project_busy:busy,view,oninspect:move |active|inspecting.set(active)}
        section {class:"project-model",hidden:view()=="folders" || inspecting(),aria_label:"Concept details",
            if m.concepts.is_empty() && draft().is_none(){p{class:"canvas-message",if busy(){"Understanding project…"}else{"{problem}"}}}
            if let Some(c)=detail{
                section {class:"concept-detail",aria_label:"Concept description","data-concept-id":c.id.clone(),
                    div {class:"concept-detail-actions",
                        if nodes.iter().any(|n|n.parent.as_ref()==Some(&c.id)) || m.concepts.iter().any(|n|n.parent.as_ref()==Some(&c.id)){UiButton{label:"Explore concept",icon:"chevron",class:"ui-icon-button",onpress:{let id=c.id.clone();move |_|{focus.set(id.clone());selected.set(None);preview.set(None);}}}}
                        for (kind,icon,label) in [("comment","comment","Annotate concept"),("idea","idea","Add idea to concept")]{UiButton{label,icon,class:"ui-icon-button",onpress:{let c=c.clone();let root=root.clone();move |_|{let _=document::eval(&format!("window.dadoConceptNotes?.begin({},{},{},{});",json!(kind),json!(c.id),json!(c.name),json!(root)));}}}}
                        UiButton{label:"Close description",icon:"close",class:"ui-icon-button",onpress:move |_|{selected.set(None);preview.set(None);}}
                    }
                    div {class:"concept-detail-content",
                    p{class:"concept-description","{c.description}"}
                    div {class:"concept-sources",for source in &c.sources{button{r#type:"button",onclick:{let source=source.clone();let root=root.clone();move |_|{if let Ok(path)=source_path(&root,&source){if path.is_file(){workbench.shown.set(true);workbench.selected_file.set(Some(path.clone()));let _=document::eval(&format!("window.dadoWorkbench?.openFile({},{});",json!(path),json!({"start":source.start,"end":source.end})));}else{view.set("folders".into());let _=document::eval(&format!("window.dadoScene?.openFolder({});",json!(path)));}}}},"{source.path}",if let (Some(a),Some(b))=(source.start,source.end){" L{a}–{b}"}}}}
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn background_generation_stops_when_disabled() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("main.rs"), "fn main() {}\n").unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap();
        let started = std::time::Instant::now();
        let preview_seen = std::cell::Cell::new(false);
        let result = runtime.block_on(generate_with_progress(
            root.path(),
            &Model::default(),
            &[],
            &crate::user_settings::Settings {
                small_model: Some("fixture".into()),
                small_reasoning_effort: Some("low".into()),
                ..Default::default()
            },
            crate::ai::Client::fixture(),
            |_| {},
            |m| preview_seen.set(preview_seen.get() || m.is_some()),
            || !preview_seen.get() && started.elapsed() < Duration::from_secs(10),
        ));
        assert!(
            preview_seen.get(),
            "Cancellation occurs during real streamed work"
        );
        assert_eq!(
            result.err().as_deref(),
            Some("Project understanding paused")
        );
        assert!(
            !model_file(root.path()).unwrap().exists(),
            "Cancelled drafts are never saved"
        );
    }
    #[test]
    fn unchanged_cache_ignores_prompt_version_and_generated_files() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("main.rs"), "fn main() {}\n").unwrap();
        let initial = inventory(root.path()).unwrap();
        for folder in [
            ".wrangler",
            ".cache",
            "__pycache__",
            "dist",
            "node_modules",
            ".dado",
        ] {
            std::fs::create_dir_all(root.path().join(folder)).unwrap();
            std::fs::write(root.path().join(folder).join("generated.txt"), "changes").unwrap();
        }
        let mut model = Model {
            analysis_version: 1,
            inventory: initial.clone(),
            ..Default::default()
        };
        model
            .inventory
            .insert(".wrangler/cache.json".into(), "old".into());
        assert_eq!(cached_inventory(&model), inventory(root.path()).unwrap());
        std::fs::write(root.path().join("main.rs"), "fn main() { }\n").unwrap();
        assert_ne!(cached_inventory(&model), inventory(root.path()).unwrap());
        assert!(
            inventory(&root.path().join("missing")).is_err(),
            "An unreadable project is not an empty inventory"
        );
    }

    #[test]
    fn incremental_updates_preserve_terminology_ids_and_unaffected_concepts() {
        let source = |path: &str| Source {
            path: path.into(),
            kind: "file".into(),
            symbol: None,
            start: None,
            end: None,
        };
        let root = Concept {
            id: "project".into(),
            name: "Ironmanhands".into(),
            description: "Project".into(),
            parent: None,
            sources: vec![Source {
                path: ".".into(),
                kind: "folder".into(),
                symbol: None,
                start: None,
                end: None,
            }],
        };
        let camera = Concept {
            id: "camera".into(),
            name: "Camera capture".into(),
            description: "Existing explanation".into(),
            parent: Some("project".into()),
            sources: vec![source("camera.py")],
        };
        let output = Concept {
            id: "output".into(),
            name: "Control PC".into(),
            description: "Keep this explanation".into(),
            parent: Some("project".into()),
            sources: vec![source("output.py")],
        };
        let old = Model {
            concepts: vec![root.clone(), camera.clone(), output.clone()],
            ..Default::default()
        };
        let mut renamed = camera.clone();
        renamed.id = "new-camera".into();
        renamed.name = "Frame ingestion".into();
        renamed.description = "Updated camera explanation".into();
        let mut next = Model {
            concepts: vec![root, renamed],
            connections: vec![Connection {
                from: "new-camera".into(),
                to: "project".into(),
                kind: "data".into(),
                description: "Input".into(),
                uncertain: false,
            }],
            ..Default::default()
        };
        preserve_identity(&old, &mut next);
        preserve_unaffected(&old, &mut next, &["camera.py".into()]);
        let updated = next.concepts.iter().find(|c| c.id == "camera").unwrap();
        assert_eq!(updated.name, "Camera capture");
        assert_eq!(updated.description, "Updated camera explanation");
        assert_eq!(next.connections[0].from, "camera");
        assert!(next.concepts.contains(&output));
        let stream = streaming_model(&serde_json::to_string(&next).unwrap(), &old).unwrap();
        assert_eq!(
            stream
                .model
                .concepts
                .iter()
                .find(|c| c.id == "camera")
                .unwrap()
                .name,
            "Camera capture"
        );
    }

    #[test]
    fn background_protocol_corrects_invalid_sources_and_refreshes_revisions() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("main.rs"), "fn main() {}\n").unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap();
        let settings = crate::user_settings::Settings {
            small_model: Some("fixture".into()),
            small_reasoning_effort: Some("low".into()),
            ..Default::default()
        };
        let mut previews = Vec::new();
        let first = runtime
            .block_on(generate_with_progress(
                root.path(),
                &Model::default(),
                &["main.rs".into()],
                &settings,
                crate::ai::Client::fixture(),
                |_| {},
                |m| previews.push(m),
                || true,
            ))
            .unwrap();
        assert!(
            previews
                .iter()
                .filter_map(Option::as_ref)
                .any(|m| m.model.concepts[0].sources[0].path == "missing.rs"),
            "Unvalidated streaming output is available for scene previews"
        );
        assert!(
            previews.iter().filter(|m| m.is_none()).count() >= 2,
            "Correction clears the rejected preview"
        );
        assert_eq!(first.concepts[0].sources[0].path, "main.rs");
        save(root.path(), &first).unwrap();
        std::fs::write(
            root.path().join("main.rs"),
            "fn main() { println!(\"changed\"); }\n",
        )
        .unwrap();
        let second = runtime
            .block_on(generate_with_client(
                root.path(),
                &first,
                &["main.rs".into()],
                &settings,
                crate::ai::Client::fixture(),
            ))
            .unwrap();
        assert_eq!(first.concepts[0].id, second.concepts[0].id);
        assert_ne!(first.revisions, second.revisions);
        save(root.path(), &second).unwrap();
        let recovered: Model =
            serde_json::from_slice(&std::fs::read(model_file(root.path()).unwrap()).unwrap())
                .unwrap();
        assert_eq!(recovered.revisions, second.revisions);
    }
    #[test]
    fn streamed_objects_respect_strings_order_and_partial_children() {
        let root = json!({"id":"root","name":"Project","description":"braces { } and \"quote\" and café","parent":null,"sources":[]});
        let child = json!({"id":"child","name":"Camera","description":"Capture","parent":"root","sources":[]});
        let edge = json!({"from":"root","to":"child","kind":"data","description":"Flow","uncertain":false});
        let text = format!("{{\"connections\":[{edge}],\"concepts\":[{child},{root},{{\"id\":");
        let draft = streaming_model(&text, &Model::default()).unwrap();
        assert_eq!(draft.arrived.len(), 2);
        let preview = draft.model;
        assert_eq!(preview.concepts.len(), 2);
        assert_eq!(preview.connections.len(), 1);
        assert!(
            preview
                .concepts
                .iter()
                .any(|c| c.description.contains("café"))
        );
        assert!(streaming_model("{\"concepts\":[{\"id\":", &Model::default()).is_none());
        let previous = preview.clone();
        let update = format!("{{\"concepts\":[{root}],\"connections\":[");
        assert_eq!(
            streaming_model(&update, &previous)
                .unwrap()
                .model
                .concepts
                .len(),
            2,
            "Refresh preserves existing branches until validation"
        );
    }
    #[test]
    fn checks_sources_ranges_and_cycles() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join("camera.py"),
            "def capture():\n    return 1\n",
        )
        .unwrap();
        let mut m = Model {
            version: 1,
            concepts: vec![Concept {
                id: "project".into(),
                name: "Project".into(),
                description: "Camera".into(),
                parent: None,
                sources: vec![Source {
                    path: "camera.py".into(),
                    kind: "file".into(),
                    symbol: Some("capture".into()),
                    start: Some(1),
                    end: Some(2),
                }],
            }],
            ..Default::default()
        };
        validate(root.path(), &mut m).unwrap();
        assert!(m.revisions.contains_key("camera.py"));
        m.concepts[0].sources[0].symbol = Some("Camera::capture".into());
        validate(root.path(), &mut m).unwrap();
        m.concepts[0].sources[0].symbol = Some("capture, process".into());
        assert!(
            validate(root.path(), &mut m)
                .unwrap_err()
                .contains("Expected one symbol")
        );
        m.concepts[0].sources[0].symbol = Some("capture".into());
        m.concepts[0].sources[0].end = Some(9);
        assert!(
            validate(root.path(), &mut m)
                .unwrap_err()
                .contains("Out of range")
        );
        m.concepts[0].sources[0].path = "../escape".into();
        assert!(validate(root.path(), &mut m).is_err());
        m.concepts[0].parent = Some("project".into());
        assert!(validate(root.path(), &mut m).is_err());
    }
}
