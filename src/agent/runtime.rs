//! Project ownership and durable run metadata. Requests and grants are never persisted.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, VecDeque},
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Queued,
    Starting,
    Running,
    WaitingForApproval,
    Stopping,
    Completed,
    Failed,
    Cancelled,
    Interrupted,
}
impl Phase {
    pub fn terminal(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Failed | Self::Cancelled | Self::Interrupted
        )
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunRecord {
    pub version: u32,
    pub project_id: String,
    pub generation: u64,
    pub run_id: String,
    pub conversation_id: String,
    pub phase: Phase,
    pub mode: String,
    pub model: String,
    pub thread_id: Option<String>,
    pub turn_id: Option<String>,
    pub error: Option<String>,
    #[serde(default)]
    pub workspace_id: Option<String>,
    #[serde(default)]
    pub context: Option<super::manifest::Manifest>,
}

pub struct ProjectRuntime {
    pub id: String,
    pub generation: u64,
    pub root: PathBuf,
    /// Retain the UI's spelling; filesystem operations use the canonical root.
    pub display_root: Value,
    pub runs: HashMap<String, RunRecord>,
    queue: VecDeque<String>,
    requests: HashMap<String, Value>,
    storage: PathBuf,
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 100
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}
fn write_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    if crate::project_store::linked(path) {
        return Err("Runtime storage cannot follow links.".into());
    }
    let parent = path.parent().ok_or("Missing storage directory.")?;
    if crate::project_store::linked(parent) {
        return Err("Runtime storage cannot follow links.".into());
    }
    let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    serde_json::to_writer_pretty(&mut temporary, value).map_err(|e| e.to_string())?;
    temporary.as_file().sync_all().map_err(|e| e.to_string())?;
    temporary.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}
impl ProjectRuntime {
    pub fn open(root: &Path, generation: u64) -> Result<Self, String> {
        let canonical = root.canonicalize().map_err(|e| e.to_string())?;
        let folder = crate::project_store::storage(&canonical)?;
        let manifest_path = folder.join("project.json");
        if crate::project_store::linked(&manifest_path) {
            return Err("Project identity cannot follow a link.".into());
        }
        let id = if manifest_path.exists() {
            let manifest: Value =
                serde_json::from_slice(&std::fs::read(&manifest_path).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
            if manifest["version"] != 1 {
                return Err("Unsupported project identity version.".into());
            }
            let id = manifest["id"].as_str().ok_or("Missing project identity.")?;
            uuid::Uuid::parse_str(id).map_err(|_| "Invalid project identity.")?;
            id.to_string()
        } else {
            let id = uuid::Uuid::new_v4().to_string();
            write_json(&manifest_path, &json!({"version":1,"id":id}))?;
            id
        };
        let storage = folder.join("runs");
        if crate::project_store::linked(&storage) {
            return Err("Run storage cannot follow a link.".into());
        }
        std::fs::create_dir_all(&storage).map_err(|e| e.to_string())?;
        let mut runtime = Self {
            id,
            generation,
            root: canonical,
            display_root: json!(root),
            runs: HashMap::new(),
            queue: VecDeque::new(),
            requests: HashMap::new(),
            storage,
        };
        // Read metadata only. No persisted request is executable on restart.
        for entry in std::fs::read_dir(&runtime.storage).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.extension().is_none_or(|e| e != "json") {
                continue;
            }
            if crate::project_store::linked(&path) {
                return Err("Run records cannot follow links.".into());
            }
            let mut record: RunRecord =
                serde_json::from_slice(&std::fs::read(&path).map_err(|e| e.to_string())?)
                    .map_err(|e| format!("Could not recover run metadata: {e}"))?;
            if record.version != 1
                || record.project_id != runtime.id
                || !valid_id(&record.run_id)
                || path.file_stem().and_then(|s| s.to_str()) != Some(&record.run_id)
            {
                return Err(
                    "Run metadata belongs to a different project or has an unsupported format."
                        .into(),
                );
            }
            if !record.phase.terminal() {
                record.phase = Phase::Interrupted;
                record.error = Some("SDE restarted before this run reached a confirmed outcome. It was not replayed.".into());
                write_json(&path, &record)?;
            }
            runtime.runs.insert(record.run_id.clone(), record);
        }
        Ok(runtime)
    }
    pub fn session_event(&self) -> Value {
        json!({"kind":"project","root":self.display_root,"projectId":self.id,"generation":self.generation})
    }
    pub fn accepts(&self, request: &Value) -> bool {
        request["projectId"] == self.id
            && request["generation"] == self.generation
            && request["root"] == self.display_root
    }
    pub fn enqueue(&mut self, mut request: Value) -> Result<Value, String> {
        if !self.accepts(&request) {
            return Err("This request belongs to an inactive project session.".into());
        }
        if self.requests.len() >= 32 {
            return Err(
                "This project already has 32 queued runs. Wait or cancel a queued run.".into(),
            );
        }
        let id = request["requestId"]
            .as_str()
            .filter(|id| valid_id(id))
            .ok_or("Invalid run identity.")?
            .to_string();
        let chat = request["chat"]
            .as_str()
            .filter(|id| valid_id(id))
            .ok_or("Invalid conversation identity.")?
            .to_string();
        if self.runs.contains_key(&id) {
            return Err("Run identity has already been used.".into());
        }
        let mode = request["mode"].as_str().unwrap_or("chat");
        super::permissions::Profile::parse(&request["mode"])?;
        let record = RunRecord {
            version: 1,
            project_id: self.id.clone(),
            generation: self.generation,
            run_id: id.clone(),
            conversation_id: chat,
            phase: Phase::Queued,
            mode: mode.into(),
            model: request["model"].as_str().unwrap_or_default().into(),
            thread_id: None,
            turn_id: None,
            error: None,
            workspace_id: None,
            context: None,
        };
        self.persist(&record)?;
        // Backend cannot infer scope from a noncanonical UI spelling.
        request["projectRoot"] = json!(self.root);
        self.runs.insert(id.clone(), record);
        self.queue.push_back(id.clone());
        self.requests.insert(id.clone(), request);
        Ok(self.lifecycle(&id))
    }
    fn persist(&self, record: &RunRecord) -> Result<(), String> {
        write_json(
            &self.storage.join(format!("{}.json", record.run_id)),
            record,
        )
    }
    pub fn lifecycle(&self, id: &str) -> Value {
        let record = &self.runs[id];
        json!({"kind":"lifecycle","root":self.display_root,"projectId":self.id,"generation":record.generation,"chat":record.conversation_id,"requestId":id,"phase":record.phase})
    }
    pub fn next(&mut self) -> Result<Option<Value>, String> {
        while let Some(id) = self.queue.pop_front() {
            let Some(request) = self.requests.remove(&id) else {
                continue;
            };
            self.set_phase(&id, Phase::Starting, None)?;
            return Ok(Some(request));
        }
        Ok(None)
    }
    pub fn set_phase(
        &mut self,
        id: &str,
        phase: Phase,
        error: Option<String>,
    ) -> Result<bool, String> {
        let Some(record) = self.runs.get(id) else {
            return Ok(false);
        };
        if record.phase.terminal() {
            return Ok(false);
        }
        // A late start notification must never undo a stop request.
        if record.phase == Phase::Stopping && !phase.terminal() {
            return Ok(false);
        }
        let mut next = record.clone();
        next.phase = phase;
        next.error = error;
        self.persist(&next)?;
        self.runs.insert(id.to_string(), next);
        Ok(true)
    }
    pub fn stop(&mut self, id: &str) -> Result<Option<Value>, String> {
        let Some(record) = self.runs.get(id) else {
            return Ok(None);
        };
        if record.phase.terminal() {
            return Ok(None);
        }
        let queued = record.phase == Phase::Queued;
        self.set_phase(
            id,
            if queued {
                Phase::Cancelled
            } else {
                Phase::Stopping
            },
            None,
        )?;
        self.requests.remove(id);
        Ok(Some(if queued {
            self.completion(id)
        } else {
            self.lifecycle(id)
        }))
    }
    pub fn completion(&self, id: &str) -> Value {
        let record = &self.runs[id];
        let status = match record.phase {
            Phase::Completed => "completed",
            Phase::Cancelled | Phase::Interrupted => "interrupted",
            _ => "failed",
        };
        json!({"kind":"completed","root":self.display_root,"projectId":self.id,"generation":record.generation,"chat":record.conversation_id,"requestId":id,"status":status,"phase":record.phase,"error":record.error})
    }
    pub fn reduce(&mut self, mut event: Value) -> Result<Option<Value>, String> {
        let Some(id) = event["requestId"].as_str().map(str::to_string) else {
            return Ok(None);
        };
        let Some(record) = self.runs.get(&id) else {
            return Ok(None);
        };
        if record.phase.terminal()
            || record.generation != self.generation
            || event["chat"] != record.conversation_id
            || event["root"] != self.display_root
            || (event.get("projectId").is_some() && event["projectId"] != self.id)
            || (event.get("generation").is_some() && event["generation"] != self.generation)
        {
            return Ok(None);
        }
        if event["kind"] == "completed" {
            let phase = if record.phase == Phase::Stopping {
                Phase::Cancelled
            } else {
                match event["status"].as_str() {
                    Some("completed") => Phase::Completed,
                    Some("interrupted") => Phase::Cancelled,
                    _ => Phase::Failed,
                }
            };
            self.set_phase(&id, phase, event["error"].as_str().map(str::to_string))?;
            return Ok(Some(self.completion(&id)));
        }
        if event["kind"] == "approval" || event["kind"] == "approval_resolved" {
            self.set_phase(
                &id,
                if event["kind"] == "approval" {
                    Phase::WaitingForApproval
                } else {
                    Phase::Running
                },
                None,
            )?;
            event["projectId"] = json!(self.id);
            event["generation"] = json!(self.generation);
            return Ok(Some(event));
        }
        if event["kind"] == "context" {
            let manifest: super::manifest::Manifest =
                serde_json::from_value(event["manifest"].clone()).map_err(|e| e.to_string())?;
            if manifest.version != 1 || manifest.resources.len() > 1024 {
                return Err("Invalid context manifest.".into());
            }
            let mut next = record.clone();
            next.context = Some(manifest);
            self.persist(&next)?;
            self.runs.insert(id.clone(), next);
            event["projectId"] = json!(self.id);
            event["generation"] = json!(self.generation);
            return Ok(Some(event));
        }
        if event["kind"] == "thread" || event["kind"] == "turn" || event["kind"] == "changes" {
            let mut next = record.clone();
            if event["kind"] == "thread" {
                next.thread_id = event["threadId"].as_str().map(str::to_string);
            } else if event["kind"] == "turn" {
                next.turn_id = event["turnId"].as_str().map(str::to_string);
                if next.phase != Phase::Stopping {
                    next.phase = Phase::Running;
                }
            } else {
                let workspace_id = event["workspaceId"]
                    .as_str()
                    .ok_or("Missing workspace identity.")?;
                uuid::Uuid::parse_str(workspace_id).map_err(|_| "Invalid workspace identity.")?;
                next.workspace_id = Some(workspace_id.into());
            }
            self.persist(&next)?;
            self.runs.insert(id.clone(), next);
        }
        event["projectId"] = json!(self.id);
        event["generation"] = json!(self.generation);
        Ok(Some(event))
    }
    pub fn finish_all(&mut self, phase: Phase, error: &str) -> Result<Vec<Value>, String> {
        let ids: Vec<_> = self
            .runs
            .values()
            .filter(|r| !r.phase.terminal())
            .map(|r| r.run_id.clone())
            .collect();
        let mut events = Vec::new();
        for id in ids {
            if let Err(storage_error) = self.set_phase(&id, phase, Some(error.into())) {
                // The backend has already stopped. Report that outcome even if
                // storage failed; the old durable record recovers as interrupted.
                let record = self.runs.get_mut(&id).unwrap();
                record.phase = phase;
                record.error = Some(format!(
                    "{error} Could not save run metadata: {storage_error}"
                ));
            }
            events.push(self.completion(&id));
        }
        self.queue.clear();
        self.requests.clear();
        Ok(events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request(runtime: &ProjectRuntime, id: &str) -> Value {
        json!({"root":runtime.display_root,"projectId":runtime.id,"generation":runtime.generation,"requestId":id,"chat":"chat-1","model":"fixture","mode":"inspect","text":"PRIVATE DRAFT"})
    }
    #[test]
    fn identity_persists_and_recovery_never_replays_requests() {
        let root = tempfile::tempdir().unwrap();
        let mut runtime = ProjectRuntime::open(root.path(), 1).unwrap();
        runtime.enqueue(request(&runtime, "run-1")).unwrap();
        runtime.next().unwrap();
        let recovered = ProjectRuntime::open(root.path(), 2).unwrap();
        assert_eq!(runtime.id, recovered.id);
        assert_eq!(recovered.runs["run-1"].phase, Phase::Interrupted);
        assert!(recovered.requests.is_empty());
        assert!(
            !std::fs::read_to_string(root.path().join(".dado/runs/run-1.json"))
                .unwrap()
                .contains("PRIVATE DRAFT")
        );
        assert!(!recovered.accepts(&request(&runtime, "run-2")));
    }
    #[test]
    fn queued_cancel_and_terminal_events_are_idempotent() {
        let root = tempfile::tempdir().unwrap();
        let mut runtime = ProjectRuntime::open(root.path(), 1).unwrap();
        runtime.enqueue(request(&runtime, "run-1")).unwrap();
        assert_eq!(
            runtime.stop("run-1").unwrap().unwrap()["phase"],
            "cancelled"
        );
        assert!(runtime.next().unwrap().is_none());
        assert!(runtime.stop("run-1").unwrap().is_none());
        runtime.enqueue(request(&runtime, "run-2")).unwrap();
        runtime.next().unwrap();
        runtime.stop("run-2").unwrap();
        let event = json!({"root":runtime.display_root,"chat":"chat-1","requestId":"run-2","kind":"completed","status":"completed"});
        assert_eq!(
            runtime.reduce(event.clone()).unwrap().unwrap()["phase"],
            "cancelled"
        );
        assert!(runtime.reduce(event).unwrap().is_none());
        assert!(runtime.enqueue(request(&runtime, "../escape")).is_err());
    }
}
