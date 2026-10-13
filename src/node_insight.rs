//! On-demand, source-validated explanations; the project vocabulary stays authoritative.
use crate::{
    Status, Workbench,
    project_model::{self, Concept, Model, Source},
    ui_controls::UiButton,
};
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

#[derive(Clone, Serialize, Deserialize)]
struct Neighbor {
    id: String,
    name: String,
    relationship: String,
    description: String,
    sources: Vec<Source>,
}
#[derive(Clone, Serialize, Deserialize)]
struct Target {
    id: String,
    name: String,
    description: String,
    sources: Vec<Source>,
    neighbors: Vec<Neighbor>,
    concept: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
struct Relationship {
    node: String,
    description: String,
    sources: Vec<Source>,
}
#[derive(Clone, Serialize, Deserialize)]
struct Explanation {
    description: String,
    relationships: Vec<Relationship>,
    sources: Vec<Source>,
}
#[derive(Clone, Serialize, Deserialize)]
struct Entry {
    context: String,
    revisions: BTreeMap<String, String>,
    explanation: Explanation,
}
#[derive(Default, Serialize, Deserialize)]
struct Cache {
    version: u32,
    entries: BTreeMap<String, Entry>,
}

fn relative_source(root: &Path, path: &Path) -> Result<Source, String> {
    let canonical = path.canonicalize().map_err(|e| e.to_string())?;
    let base = root.canonicalize().map_err(|e| e.to_string())?;
    let relative = canonical
        .strip_prefix(base)
        .map_err(|_| "Node outside project")?
        .to_string_lossy()
        .replace('\\', "/");
    let source = Source {
        path: if relative.is_empty() {
            ".".into()
        } else {
            relative
        },
        kind: if canonical.is_dir() { "folder" } else { "file" }.into(),
        symbol: None,
        start: None,
        end: None,
    };
    project_model::source_path(root, &source)?;
    Ok(source)
}
fn target(root: &Path, model: &Model, event: &Value) -> Result<Target, String> {
    let path = event["path"].as_str().ok_or("Missing held node")?;
    if let Some(id) = path.strip_prefix("concept:") {
        let c = model
            .concepts
            .iter()
            .find(|c| c.id == id)
            .ok_or("Concept is not yet validated")?;
        let mut neighbors = BTreeMap::<String, Neighbor>::new();
        for n in &model.concepts {
            let relationship = if c.parent.as_ref() == Some(&n.id) {
                Some("part of")
            } else if n.parent.as_ref() == Some(&c.id) {
                Some("contains")
            } else {
                None
            };
            if let Some(relationship) = relationship {
                neighbors.insert(
                    n.id.clone(),
                    Neighbor {
                        id: n.id.clone(),
                        name: n.name.clone(),
                        relationship: relationship.into(),
                        description: n.description.clone(),
                        sources: n.sources.clone(),
                    },
                );
            }
        }
        for e in &model.connections {
            let (id, direction) = if e.from == c.id {
                (&e.to, "outgoing")
            } else if e.to == c.id {
                (&e.from, "incoming")
            } else {
                continue;
            };
            if let Some(n) = model.concepts.iter().find(|c| &c.id == id) {
                let relation = format!(
                    "{direction} {}: {}{}",
                    e.kind,
                    e.description,
                    if e.uncertain { " (uncertain)" } else { "" }
                );
                neighbors
                    .entry(id.clone())
                    .and_modify(|n| {
                        n.relationship.push_str("; ");
                        n.relationship.push_str(&relation);
                    })
                    .or_insert(Neighbor {
                        id: n.id.clone(),
                        name: n.name.clone(),
                        relationship: relation,
                        description: n.description.clone(),
                        sources: n.sources.clone(),
                    });
            }
        }
        if neighbors.len() > 128 {
            return Err("This node has too many relationships to explain at once. Explore a narrower concept.".into());
        }
        Ok(Target {
            id: format!("concept:{id}"),
            name: c.name.clone(),
            description: c.description.clone(),
            sources: c.sources.clone(),
            neighbors: neighbors.into_values().collect(),
            concept: Some(c.id.clone()),
        })
    } else {
        let source = relative_source(root, Path::new(path))?;
        let mut neighbors = Vec::new();
        for n in event["neighbors"]
            .as_array()
            .into_iter()
            .flatten()
            .take(128)
        {
            let Some(path) = n["path"].as_str() else {
                continue;
            };
            let Ok(s) = relative_source(root, Path::new(path)) else {
                continue;
            };
            // Only real containment relationships come from the folder graph.
            let a = root
                .join(&source.path)
                .canonicalize()
                .map_err(|e| e.to_string())?;
            let b = root
                .join(&s.path)
                .canonicalize()
                .map_err(|e| e.to_string())?;
            let relationship = if a.parent() == Some(b.as_path()) {
                "part of"
            } else if b.parent() == Some(a.as_path()) {
                "contains"
            } else {
                continue;
            };
            neighbors.push(Neighbor {
                id: s.path.clone(),
                name: b.file_name().unwrap_or_default().to_string_lossy().into(),
                relationship: relationship.into(),
                description: String::new(),
                sources: vec![s],
            });
        }
        Ok(Target {id:format!("path:{}",source.path),name:root.join(&source.path).file_name().unwrap_or_default().to_string_lossy().into(),description:"A source item in this project. Explain its actual responsibility, not merely its position in a folder.".into(),sources:vec![source],neighbors,concept:None})
    }
}
fn cache_file(root: &Path) -> Result<PathBuf, String> {
    let p = crate::project_store::storage(root)?.join("node-insights.json");
    if crate::project_store::linked(&p) {
        return Err("Insight storage cannot follow links".into());
    }
    Ok(p)
}
fn load(root: &Path) -> Result<Cache, String> {
    let p = cache_file(root)?;
    if !p.exists() {
        return Ok(Cache {
            version: 1,
            ..Default::default()
        });
    }
    let cache: Cache = serde_json::from_slice(&std::fs::read(p).map_err(|e| e.to_string())?)
        .map_err(|e| format!("Could not read saved node explanations: {e}"))?;
    if cache.version != 1 {
        return Err("Unsupported node explanation cache version".into());
    }
    Ok(cache)
}
fn save(root: &Path, key: &str, entry: Entry) -> Result<(), String> {
    let mut cache = load(root)?;
    cache.entries.insert(key.into(), entry);
    let path = cache_file(root)?;
    let mut temp =
        tempfile::NamedTempFile::new_in(path.parent().unwrap()).map_err(|e| e.to_string())?;
    serde_json::to_writer_pretty(&mut temp, &cache).map_err(|e| e.to_string())?;
    temp.as_file().sync_all().map_err(|e| e.to_string())?;
    temp.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}
fn context_revision(target: &Target) -> String {
    crate::agent::snapshot::revision(&serde_json::to_vec(target).unwrap())
}
fn revisions(
    root: &Path,
    paths: impl Iterator<Item = String>,
) -> Result<BTreeMap<String, String>, String> {
    let mut result = BTreeMap::new();
    let mut files = None;
    for path in paths {
        if result.contains_key(&path) {
            continue;
        }
        let full = project_model::source_path(
            root,
            &Source {
                path: path.clone(),
                kind: "file".into(),
                symbol: None,
                start: None,
                end: None,
            },
        )?;
        let bytes = if full.is_dir() {
            if files.is_none() {
                files = Some(project_model::inventory(root)?);
            }
            // Folder membership matters; precise file citations track content changes.
            files
                .as_ref()
                .unwrap()
                .keys()
                .filter(|p| path == "." || p.starts_with(&format!("{path}/")))
                .cloned()
                .collect::<Vec<_>>()
                .join("\n")
                .into_bytes()
        } else {
            std::fs::read(full).map_err(|e| e.to_string())?
        };
        result.insert(path, crate::agent::snapshot::revision(&bytes));
    }
    Ok(result)
}
fn fresh(root: &Path, target: &Target, entry: &Entry) -> bool {
    entry.context == context_revision(target)
        && revisions(root, entry.revisions.keys().cloned()).is_ok_and(|r| r == entry.revisions)
}
fn validate(root: &Path, target: &Target, explanation: Explanation) -> Result<Entry, String> {
    if explanation.description.is_empty()
        || explanation.description.len() > 8000
        || explanation.relationships.len() > 128
        || explanation.sources.is_empty()
    {
        return Err("Expected a source-backed explanation under 8000 characters".into());
    }
    for neighbor in &target.neighbors {
        if !explanation
            .relationships
            .iter()
            .any(|r| r.node == neighbor.id)
        {
            return Err(format!(
                "Missing relationship explanation for {} ({}). Use its existing ID and name and explain the source-backed relationship, including containment where applicable.",
                neighbor.id, neighbor.name
            ));
        }
    }
    for r in &explanation.relationships {
        if !target.neighbors.iter().any(|n| n.id == r.node)
            || r.description.is_empty()
            || r.description.len() > 3000
            || r.sources.is_empty()
        {
            return Err(format!(
                "Invalid relationship {}: use an existing neighbor ID and verified sources",
                r.node
            ));
        }
    }
    let sources: Vec<_> = target
        .sources
        .iter()
        .chain(target.neighbors.iter().flat_map(|n| &n.sources))
        .chain(&explanation.sources)
        .chain(explanation.relationships.iter().flat_map(|r| &r.sources))
        .cloned()
        .collect();
    if sources.len() > 2000 {
        return Err("Too many explanation sources".into());
    }
    // Reuse file/type/symbol/range validation rather than trusting the small model.
    let mut source_errors = Vec::new();
    for source in explanation
        .sources
        .iter()
        .chain(explanation.relationships.iter().flat_map(|r| &r.sources))
    {
        let mut m = Model {
            version: 1,
            concepts: vec![Concept {
                id: "insight".into(),
                name: target.name.clone(),
                description: String::new(),
                parent: None,
                sources: vec![source.clone()],
            }],
            ..Default::default()
        };
        if let Err(error) = project_model::validate(root, &mut m) {
            if !source_errors.contains(&error) {
                source_errors.push(error);
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
    Ok(Entry {
        context: context_revision(target),
        revisions: revisions(root, sources.into_iter().map(|s| s.path))?,
        explanation,
    })
}
fn schema() -> Value {
    fn object(p: Value) -> Value {
        json!({"type":"object","additionalProperties":false,"required":p.as_object().unwrap().keys().collect::<Vec<_>>(),"properties":p})
    }
    let sources = json!({"type":"array","items":project_model::output_schema()["properties"]["concepts"]["items"]["properties"]["sources"]["items"]});
    object(
        json!({"description":{"type":"string"},"sources":sources,"relationships":{"type":"array","items":object(json!({"node":{"type":"string"},"description":{"type":"string"},"sources":sources}))}}),
    )
}
async fn generate(
    root: &Path,
    target: &Target,
    old: Option<&Entry>,
    settings: &crate::user_settings::Settings,
    client: crate::ai::Client,
    mut progress: impl FnMut(String),
) -> Result<Entry, String> {
    let model = settings
        .small_model
        .clone()
        .ok_or("Choose a small model in Model Settings to explain this node.")?;
    let baseline = project_model::inventory(root)?;
    client.send(json!({"action":"ai-project","root":root,"generation":1}));
    let started = Instant::now();
    let mut session = Value::Null;
    let mut feedback = String::new();
    let mut previous = old.map(|e| json!(e.explanation)).unwrap_or(Value::Null);
    for _ in 0..3 {
        let mut request = String::new();
        let mut text = String::new();
        let mut seconds = u64::MAX;
        loop {
            if started.elapsed() > Duration::from_secs(180) {
                return Err("Node explanation timed out".into());
            }
            if seconds != started.elapsed().as_secs() {
                seconds = started.elapsed().as_secs();
                progress(format!("Explaining {} · {seconds}s", target.name));
            }
            while let Ok(event) = client.events.try_recv() {
                if event["kind"] == "scene_activity" {
                    project_model::scene_activity(root, &event);
                }
                if event["kind"] == "project" {
                    session = event.clone();
                }
                if event["kind"] == "state" && event["error"].is_string() {
                    return Err(event["error"].as_str().unwrap().into());
                }
                if !request.is_empty() && event["requestId"] == request {
                    if event["kind"] == "delta" {
                        text.push_str(event["delta"].as_str().unwrap_or(""));
                    }
                    if event["kind"] == "message" {
                        text = event["text"].as_str().unwrap_or(&text).into();
                    }
                    if event["kind"] == "completed" {
                        if event["status"] != "completed" {
                            return Err(event["error"]
                                .as_str()
                                .unwrap_or("Node explanation failed")
                                .into());
                        }
                        let parsed = serde_json::from_str::<Explanation>(text.trim())
                            .map_err(|e| e.to_string())
                            .and_then(|e| {
                                previous = json!(e);
                                validate(root, target, e)
                            });
                        match parsed {
                            Ok(entry) => {
                                for (path, revision) in &entry.revisions {
                                    let source = root.join(path);
                                    let before = if source.is_dir() {
                                        Some(crate::agent::snapshot::revision(
                                            &baseline
                                                .keys()
                                                .filter(|p| {
                                                    path == "."
                                                        || p.starts_with(&format!("{path}/"))
                                                })
                                                .cloned()
                                                .collect::<Vec<_>>()
                                                .join("\n")
                                                .into_bytes(),
                                        ))
                                    } else {
                                        baseline.get(path).cloned()
                                    };
                                    if before.as_ref() != Some(revision) {
                                        return Err("Supporting sources changed while explaining this node. Hold it again to update the explanation.".into());
                                    }
                                }
                                return Ok(entry);
                            }
                            Err(e) => {
                                feedback = e;
                                break;
                            }
                        }
                    }
                }
            }
            if !feedback.is_empty() && !text.is_empty() {
                break;
            }
            if request.is_empty() && !session.is_null() {
                request = uuid::Uuid::new_v4().to_string();
                let prompt = format!(
                    "Explain one held node and its relationships in this project, using read-only source tools. Return ONLY JSON matching the schema. Describe its actual responsibility in more depth than the existing summary: how it is used, inputs/outputs, and how each relevant neighbor participates. Cite verified relative files or exact inclusive line ranges. Use null for both bounds for whole files. For a range both bounds must be 1-based inclusive integers. Read the actual source lines before citing them. A symbol is ONE declaration name (qualified names such as Tree::read are accepted); use null for a broad section or whole file. Never combine multiple names or include signatures in a symbol string. The named symbol must occur inside the cited range. If you cannot verify a function range, use a verified whole-file reference with symbol null rather than inventing a range. When validation feedback is present, re-read the cited sources and correct ALL reported references together; preserve valid text, names and citations. Neighbor IDs and all existing names/terminology are authoritative: NEVER rename them or invent alternative labels. Do not invent runtime dependencies from folder containment; distinguish containment, confirmed usage and inference in prose. Explain only evidence-supported relationships; use existing neighbor IDs in relationships[].node. If refreshing, preserve the previous wording and update only outdated passages and references. Do not edit files or change the mental model.\nTarget and current relationships: {}\nPrevious explanation: {}\nValidation feedback (correct the invalid references while preserving valid text): {}",
                    json!(target),
                    previous,
                    feedback
                );
                feedback.clear();
                client.send(json!({"action":"ai-send","root":root,"projectId":session["projectId"],"generation":session["generation"],"requestId":request,"chat":"node-insight","mode":"inspect","model":model,"effort":settings.small_reasoning_effort,"text":prompt,"attachments":[],"history":[],"context":{},"outputSchema":schema()}));
            }
            if text.len() > 256000 {
                return Err("Node explanation exceeded the output limit".into());
            }
            tokio::time::sleep(Duration::from_millis(40)).await;
        }
    }
    Err("I couldn’t verify the explanation’s source references. Hold the node again to retry; any previous explanation is still available.".into())
}

#[component]
pub fn NodeInsight(
    root: PathBuf,
    model: Signal<Model>,
    project_busy: Signal<bool>,
    view: Signal<String>,
    oninspect: EventHandler<bool>,
) -> Element {
    let mut request = use_signal(|| None::<Value>);
    let mut content = use_signal(|| None::<Entry>);
    let mut active = use_signal(|| None::<Target>);
    let mut busy = use_signal(|| false);
    let mut problem = use_signal(String::new);
    let mut validated = use_signal(|| false);
    let mut status = use_context::<Status>();
    let bridge_token = use_signal(|| uuid::Uuid::new_v4().to_string());
    let cleanup_token = bridge_token.peek().clone();
    use_drop(move || {
        let _ = document::eval(&format!(
            "if(window.dadoInsightBridgeToken==={}){{window.dadoInsightBridge?.abort();window.dadoNodeInsight=null;}}",
            json!(cleanup_token)
        ));
    });
    use_future(move || async move {
        let _ = document::eval(&format!(
            "window.dadoInsightBridgeToken={};",
            json!(bridge_token.peek().as_str())
        ));
        let mut bridge = document::eval(
            r#"window.dadoInsightBridge?.abort();window.dadoInsightBridge=new AbortController();
            let pending=null;
            const normalize=p=>p.replaceAll('\\','/').replaceAll('/./','/').replace(/\/$/,'');
            const reveal=()=>{
                if(!pending)return;
                const row=[...document.querySelectorAll('.node-insight:not([hidden]) [data-relationship-path]')].find(row=>normalize(row.dataset.relationshipPath)===normalize(pending));
                if(!row)return;
                pending=null;
                const text=row.querySelector('.concept-description');
                const reduced=(document.documentElement.dataset.respectReducedMotion!=='false' && matchMedia('(prefers-reduced-motion:reduce)').matches) || document.documentElement.dataset.reducedMotion==='true';
                row.scrollIntoView({block:'center',behavior:reduced?'instant':'smooth'});
                text.classList.remove('relationship-text-flash');void text.offsetWidth;text.classList.add('relationship-text-flash');
                setTimeout(()=>text.classList.remove('relationship-text-flash'),500);
            };
            const observer=new MutationObserver(reveal);observer.observe(document.body,{childList:true,subtree:true});
            window.dadoInsightBridge.signal.addEventListener('abort',()=>observer.disconnect());
            window.addEventListener('dado-node-hold',e=>{
                if(e.detail.relationship){pending=e.detail.relationship;reveal();}
                else{pending=null;dioxus.send(e.detail);}
            },{signal:window.dadoInsightBridge.signal});"#,
        );
        while let Ok(event) = bridge.recv::<Value>().await {
            if event["close"] == true {
                request.set(None);
                oninspect.call(false);
            } else if !(busy.peek().to_owned()
                && request
                    .peek()
                    .as_ref()
                    .is_some_and(|r| r["path"] == event["path"]))
            {
                request.set(Some(event));
                oninspect.call(true);
            }
        }
    });
    let worker_root = root.clone();
    use_resource(move || {
        let event = request();
        let snapshot = model();
        let root = worker_root.clone();
        async move {
            let Some(event) = event else {
                content.set(None);
                active.set(None);
                problem.set(String::new());
                if busy.peek().to_owned() && !*project_busy.peek() {
                    status.phase.set("idle");
                    status.activity.set(None);
                }
                busy.set(false);
                return;
            };
            busy.set(false);
            validated.set(false);
            problem.set(String::new());
            let target = match target(&root, &snapshot, &event) {
                Ok(t) => t,
                Err(e) => {
                    problem.set(e);
                    return;
                }
            };
            if active.peek().as_ref().is_none_or(|t| t.id != target.id) {
                content.set(None);
            }
            active.set(Some(target.clone()));
            let cache = match load(&root) {
                Ok(c) => c,
                Err(e) => {
                    problem.set(e);
                    return;
                }
            };
            let cached = cache.entries.get(&target.id).cloned();
            content.set(cached.clone());
            if cached.as_ref().is_some_and(|e| fresh(&root, &target, e)) {
                validated.set(true);
                return;
            }
            busy.set(true);
            while *project_busy.peek() {
                tokio::time::sleep(Duration::from_millis(80)).await;
            }
            status.phase.set("working");
            status
                .activity
                .set(Some(format!("Explaining {}…", target.name)));
            let generated =
                match crate::user_settings::home().and_then(|h| crate::user_settings::load(&h)) {
                    Ok((settings, _)) => {
                        generate(
                            &root,
                            &target,
                            cached.as_ref(),
                            &settings,
                            crate::ai::Client::start(),
                            |text| status.activity.set(Some(text)),
                        )
                        .await
                    }
                    Err(error) => Err(error),
                };
            match generated.and_then(|e| {
                save(&root, &target.id, e.clone())?;
                Ok(e)
            }) {
                Ok(entry) => {
                    content.set(Some(entry));
                    validated.set(true);
                    status.phase.set("done");
                    status.activity.set(Some("Explanation ready".into()));
                }
                Err(error) => {
                    problem.set(error.clone());
                    status.phase.set("error");
                    status
                        .activity
                        .set(Some(format!("Explanation failed · {error}")));
                }
            }
            busy.set(false);
        }
    });
    let publish_root = root.clone();
    use_effect(move || {
        let value = if validated() {
            active().and_then(|t| t.concept).zip(content()).map(
                |(id, e)| json!({"root":publish_root,"conceptId":format!("model-{id}"),"explanation":e.explanation}),
            )
        } else {
            None
        };
        let _ = document::eval(&format!("window.dadoNodeInsight={};", json!(value)));
    });
    let explanation = content().map(|c| c.explanation);
    let target = active();
    rsx! {
        section {class:"project-model node-insight",hidden:request().is_none(),aria_label:"Node relationships",
            if request().is_some() {section {class:"concept-detail",aria_label:"Node explanation","data-concept-id":target.as_ref().and_then(|t|t.concept.clone()).unwrap_or_else(||target.as_ref().map(|t|format!("source:{}",t.sources[0].path)).unwrap_or_default()),"data-node-path":target.as_ref().filter(|t|t.concept.is_none()).map(|t|root.join(&t.sources[0].path).display().to_string()).unwrap_or_default(),
                div {class:"concept-detail-actions",
                    if let Some(t)=target.clone() {
                        for (kind,icon,label) in [("comment","comment","Annotate explanation"),("idea","idea","Add idea to explanation")] {UiButton {label,icon,class:"ui-icon-button",onpress:{let root=root.clone();let id=t.concept.clone().unwrap_or_else(||format!("source:{}",t.sources[0].path));let name=t.name.clone();move |_|{let _=document::eval(&format!("window.dadoConceptNotes?.begin({},{},{},{});",json!(kind),json!(id),json!(name),json!(root)));}}}}
                    }
                    UiButton {label:"Close explanation",icon:"close",class:"ui-icon-button",onpress:move |_|{let path=request.peek().as_ref().and_then(|r|r["path"].as_str().map(str::to_string));request.set(None);oninspect.call(false);let _=document::eval(&format!("window.dispatchEvent(new CustomEvent('dado-insight-close'));window.dadoScene?.focusNode({});",json!(path)));}}
                }
                div {class:"concept-detail-content",aria_busy:busy(),
                    if let Some(t)=target.clone(){p {class:"insight-name","{t.name}"}}
                    if let Some(explanation)=explanation {
                        p {class:"concept-description","{explanation.description}"}
                        for r in explanation.relationships {if let Some(n)=target.as_ref().and_then(|t|t.neighbors.iter().find(|n|n.id==r.node)){
                            div {"data-relationship-path":if target.as_ref().is_some_and(|t|t.concept.is_some()){format!("concept:{}",n.id)}else{root.join(&n.id).display().to_string()},
                            p {class:"insight-name","{n.name}"} p {class:"concept-description","{r.description}"}
                            InsightSources {root:root.clone(),sources:r.sources,view}
                            }
                        }}
                        InsightSources {root:root.clone(),sources:explanation.sources,view}
                    } else if problem().is_empty(){p {class:"concept-description","Understanding this node…"}}
                    if !problem().is_empty(){p {class:"concept-description",role:"alert","{problem}"}}
                }
            }
            }
        }
    }
}
#[component]
fn InsightSources(root: PathBuf, sources: Vec<Source>, mut view: Signal<String>) -> Element {
    let mut workbench = use_context::<Workbench>();
    rsx! {div {class:"concept-sources",for source in sources {button {r#type:"button",onclick:{let root=root.clone();let source=source.clone();move |_|{if let Ok(path)=project_model::source_path(&root,&source){if path.is_file(){workbench.shown.set(true);workbench.selected_file.set(Some(path.clone()));let _=document::eval(&format!("window.dadoWorkbench?.openFile({},{});",json!(path),json!({"start":source.start,"end":source.end})));}else{view.set("folders".into());let _=document::eval(&format!("window.dadoScene?.openFolder({});",json!(path)));}}}},"{source.path}",if let(Some(a),Some(b))=(source.start,source.end){" L{a}–{b}"}}}}}
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample() -> (tempfile::TempDir, Target) {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("main.rs"), "fn main() {}\n").unwrap();
        (
            root,
            Target {
                id: "concept:camera".into(),
                name: "Camera capture".into(),
                description: "Capture frames".into(),
                sources: vec![Source {
                    path: "main.rs".into(),
                    kind: "file".into(),
                    symbol: None,
                    start: None,
                    end: None,
                }],
                neighbors: vec![],
                concept: Some("camera".into()),
            },
        )
    }
    #[test]
    fn saved_explanations_reuse_verified_sources_until_inputs_change() {
        let (root, mut target) = sample();
        let explanation = Explanation {
            description: "Camera capture produces frames.".into(),
            sources: target.sources.clone(),
            relationships: vec![],
        };
        let entry = validate(root.path(), &target, explanation).unwrap();
        save(root.path(), &target.id, entry).unwrap();
        let cache = load(root.path()).unwrap();
        let saved = &cache.entries[&target.id];
        assert!(fresh(root.path(), &target, saved));
        target.description.push_str(" with a new input");
        assert!(!fresh(root.path(), &target, saved));
        target.description = "Capture frames".into();
        std::fs::write(
            root.path().join("main.rs"),
            "fn main() { println!(\"updated\"); }\n",
        )
        .unwrap();
        assert!(!fresh(root.path(), &target, saved));
        std::fs::write(cache_file(root.path()).unwrap(), "broken json").unwrap();
        assert!(load(root.path()).is_err());
        assert_eq!(
            std::fs::read_to_string(cache_file(root.path()).unwrap()).unwrap(),
            "broken json"
        );
    }
    #[test]
    fn on_demand_protocol_corrects_bad_sources_without_changing_terminology() {
        let (root, mut target) = sample();
        let settings = crate::user_settings::Settings {
            small_model: Some("fixture".into()),
            small_reasoning_effort: Some("low".into()),
            ..Default::default()
        };
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap();
        let result = runtime
            .block_on(generate(
                root.path(),
                &target,
                None,
                &settings,
                crate::ai::Client::fixture(),
                |_| {},
            ))
            .unwrap();
        assert_eq!(target.name, "Camera capture");
        assert_eq!(result.explanation.sources[0].path, "main.rs");
        assert_eq!(
            result.explanation.sources[0].symbol.as_deref(),
            Some("main")
        );
        assert!(result.explanation.description.contains("Camera capture"));
        assert!(fresh(root.path(), &target, &result));
        target.description = "fixture: uncorrectable references".into();
        let error = runtime
            .block_on(generate(
                root.path(),
                &target,
                Some(&result),
                &settings,
                crate::ai::Client::fixture(),
                |_| {},
            ))
            .err()
            .expect("Unverified references must never be published");
        assert!(error.contains("Hold the node again to retry"));
        assert!(!error.contains("missing_capture"));
    }
}
