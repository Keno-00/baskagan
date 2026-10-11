//! Owned isolated working copies and reviewable file proposals. No live writes.
use super::context::{Context, excluded};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    path::{Path, PathBuf},
};

const FILE_LIMIT: usize = 4 * 1024 * 1024;
const TOTAL_LIMIT: usize = 64 * 1024 * 1024;
const ABSENT: &str = "absent";
pub fn revision(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Baseline {
    disk_revision: String,
    base_revision: String,
    source: String,
    proposed_revision: Option<String>,
    #[serde(default)]
    saved_revision: Option<String>,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    version: u32,
    project_id: String,
    run_id: String,
    workspace_id: String,
    state: String,
    files: BTreeMap<String, Baseline>,
}
pub struct Snapshot {
    root: PathBuf,
    folder: PathBuf,
    manifest: Manifest,
}
fn relative_key(path: &Path) -> Result<String, String> {
    Ok(path
        .to_str()
        .ok_or("Unsupported file name.")?
        .replace('\\', "/"))
}
fn read(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
    if crate::project_store::linked(path) {
        return Err("Snapshot operations cannot follow links.".into());
    }
    if !std::fs::symlink_metadata(path)
        .map_err(|e| e.to_string())?
        .is_file()
    {
        return Err("Choose a regular file.".into());
    }
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    if !file.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("Choose a regular file.".into());
    }
    let mut bytes = Vec::new();
    file.take((limit + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > limit {
        return Err("Snapshot file exceeds its size limit.".into());
    }
    Ok(bytes)
}
fn optional_read(
    context: &Context,
    relative: &str,
    limit: usize,
) -> Result<Option<Vec<u8>>, String> {
    let path = context.target(relative)?;
    match std::fs::symlink_metadata(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
        Ok(_) => read(&path, limit).map(Some),
    }
}
fn state_revision(bytes: Option<&[u8]>) -> String {
    bytes.map(revision).unwrap_or_else(|| ABSENT.into())
}
fn reviewable(bytes: &[u8]) -> bool {
    bytes.len() <= 128 * 1024 && !bytes.contains(&0) && std::str::from_utf8(bytes).is_ok()
}
impl Snapshot {
    pub fn create(
        root: &Path,
        project_id: &str,
        run_id: &str,
        buffers: &Value,
        cancelled: &std::sync::atomic::AtomicBool,
    ) -> Result<Self, String> {
        uuid::Uuid::parse_str(project_id).map_err(|_| "Invalid project identity.")?;
        let root = root.canonicalize().map_err(|e| e.to_string())?;
        let context = Context::new(&root, buffers)?;
        let parent = crate::project_store::storage(&root)?.join("workspaces");
        if crate::project_store::linked(&parent) {
            return Err("Workspace storage cannot follow a link.".into());
        }
        std::fs::create_dir_all(&parent).map_err(|e| e.to_string())?;
        let workspace_id = uuid::Uuid::new_v4().to_string();
        // Remove incomplete construction only through the owned TempDir guard.
        let guard = tempfile::Builder::new()
            .prefix(&workspace_id)
            .rand_bytes(0)
            .tempdir_in(&parent)
            .map_err(|e| e.to_string())?;
        let folder = guard.path().to_path_buf();
        let mut snapshot = Self {
            root,
            folder,
            manifest: Manifest {
                version: 1,
                project_id: project_id.into(),
                run_id: run_id.into(),
                workspace_id,
                state: "pending".into(),
                files: BTreeMap::new(),
            },
        };
        for name in ["base", "work"] {
            std::fs::create_dir(snapshot.folder.join(name)).map_err(|e| e.to_string())?;
        }
        let mut pending = vec![PathBuf::new()];
        let mut total = 0;
        let mut visited = 0;
        while let Some(relative) = pending.pop() {
            if cancelled.load(std::sync::atomic::Ordering::Acquire) {
                return Err("Snapshot construction was cancelled.".into());
            }
            let directory = context.resolve(relative.to_str().ok_or("Unsupported path.")?)?;
            for entry in std::fs::read_dir(directory).map_err(|e| e.to_string())? {
                if cancelled.load(std::sync::atomic::Ordering::Acquire) {
                    return Err("Snapshot construction was cancelled.".into());
                }
                visited += 1;
                if visited > 10000 {
                    return Err("Project snapshot exceeds 10,000 entries.".into());
                }
                let entry = entry.map_err(|e| e.to_string())?;
                let name = entry
                    .file_name()
                    .to_str()
                    .ok_or("Unsupported file name.")?
                    .to_string();
                if excluded(&name) || crate::project_store::linked(&entry.path()) {
                    continue;
                }
                let relative = relative.join(name);
                let source = context.resolve(relative.to_str().ok_or("Unsupported path.")?)?;
                if source.is_dir() {
                    for name in ["base", "work"] {
                        std::fs::create_dir_all(snapshot.folder.join(name).join(&relative))
                            .map_err(|e| e.to_string())?;
                    }
                    pending.push(relative);
                    continue;
                }
                if snapshot.manifest.files.len() >= 2000 {
                    return Err("Project snapshot exceeds 2,000 files.".into());
                }
                let bytes = read(&source, FILE_LIMIT)?;
                total += bytes.len();
                if total > TOTAL_LIMIT {
                    return Err("Project snapshot exceeds 64 MB.".into());
                }
                let disk_revision = revision(&bytes);
                for name in ["base", "work"] {
                    std::fs::write(snapshot.folder.join(name).join(&relative), &bytes)
                        .map_err(|e| e.to_string())?;
                }
                snapshot.manifest.files.insert(
                    relative_key(&relative)?,
                    Baseline {
                        disk_revision: disk_revision.clone(),
                        base_revision: disk_revision,
                        source: "disk".into(),
                        proposed_revision: None,
                        saved_revision: None,
                    },
                );
            }
        }
        for buffer in buffers.as_array().into_iter().flatten() {
            let Some(path) = buffer["path"].as_str() else {
                continue;
            };
            let canonical = Path::new(path).canonicalize().map_err(|e| e.to_string())?;
            let Ok(relative) = canonical.strip_prefix(&snapshot.root) else {
                continue;
            };
            if relative.components().any(
                |c| matches!(c,std::path::Component::Normal(n) if n.to_str().is_some_and(excluded)),
            ) {
                continue;
            }
            let key = relative_key(relative)?;
            let Some(baseline) = snapshot.manifest.files.get_mut(&key) else {
                return Err("An editor file is excluded from the snapshot.".into());
            };
            let text = buffer["text"].as_str().ok_or("Missing editor snapshot.")?;
            if let Some(original) = buffer["original"].as_str() {
                baseline.disk_revision = revision(original.as_bytes());
            }
            baseline.base_revision = revision(text.as_bytes());
            baseline.source = "editor".into();
            for name in ["base", "work"] {
                std::fs::write(snapshot.folder.join(name).join(relative), text)
                    .map_err(|e| e.to_string())?;
            }
        }
        snapshot.persist()?;
        let _ = guard.keep();
        Ok(snapshot)
    }
    pub fn load(
        root: &Path,
        project_id: &str,
        run_id: &str,
        workspace_id: &str,
    ) -> Result<Self, String> {
        let workspace_id = uuid::Uuid::parse_str(workspace_id)
            .map_err(|_| "Invalid workspace identity.")?
            .to_string();
        let root = root.canonicalize().map_err(|e| e.to_string())?;
        let folder = crate::project_store::storage(&root)?
            .join("workspaces")
            .join(workspace_id);
        for path in [
            folder.parent().unwrap().to_path_buf(),
            folder.clone(),
            folder.join("manifest.json"),
            folder.join("base"),
            folder.join("work"),
        ] {
            if crate::project_store::linked(&path) {
                return Err("Workspace storage cannot follow links.".into());
            }
        }
        let manifest: Manifest =
            serde_json::from_slice(&read(&folder.join("manifest.json"), 2 * 1024 * 1024)?)
                .map_err(|e| e.to_string())?;
        if manifest.version != 1
            || manifest.project_id != project_id
            || manifest.run_id != run_id
            || folder.file_name().and_then(|s| s.to_str()) != Some(&manifest.workspace_id)
        {
            return Err("Workspace belongs to another run or project.".into());
        }
        Ok(Self {
            root,
            folder,
            manifest,
        })
    }
    pub fn work_root(&self) -> PathBuf {
        self.folder.join("work")
    }
    pub fn id(&self) -> &str {
        &self.manifest.workspace_id
    }
    fn persist(&self) -> Result<(), String> {
        let mut temporary =
            tempfile::NamedTempFile::new_in(&self.folder).map_err(|e| e.to_string())?;
        serde_json::to_writer_pretty(&mut temporary, &self.manifest).map_err(|e| e.to_string())?;
        temporary.as_file().sync_all().map_err(|e| e.to_string())?;
        temporary
            .persist(self.folder.join("manifest.json"))
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    pub fn propose(&mut self, args: &Value) -> Result<Value, String> {
        if self.manifest.state != "pending" {
            return Err("This proposal has already been applied to editors.".into());
        }
        let relative = args["path"]
            .as_str()
            .ok_or("Choose an existing project file.")?;
        let context = Context::new(&self.work_root(), &json!([]))?;
        let path = context.resolve(relative)?;
        let key = relative_key(
            path.strip_prefix(self.work_root().canonicalize().map_err(|e| e.to_string())?)
                .map_err(|_| "Path leaves workspace.")?,
        )?;
        let baseline = self
            .manifest
            .files
            .get(&key)
            .ok_or("File is not in the baseline snapshot.")?;
        let before = read(&path, 128 * 1024)?;
        if args["expectedRevision"].as_str() != Some(revision(&before).as_str()) {
            return Err(
                "File revision changed. Read it again before proposing a replacement.".into(),
            );
        }
        let text = args["text"].as_str().ok_or("Missing replacement text.")?;
        if text.len() > 128 * 1024 || text.contains('\0') {
            return Err("Proposals require UTF-8 text under 128 KB.".into());
        }
        if self
            .manifest
            .files
            .values()
            .filter(|f| f.proposed_revision.is_some())
            .count()
            >= 16
            && baseline.proposed_revision.is_none()
        {
            return Err("A proposal can change at most 16 files.".into());
        }
        // Preserve established BOM and line endings while the editor remains the writer.
        let base = std::fs::read(self.folder.join("base").join(&key)).map_err(|e| e.to_string())?;
        let normalized = text.trim_start_matches('\u{feff}').replace("\r\n", "\n");
        let text = format!(
            "{}{}",
            if base.starts_with(&[0xef, 0xbb, 0xbf]) {
                "\u{feff}"
            } else {
                ""
            },
            if base.windows(2).any(|w| w == b"\r\n") {
                normalized.replace('\n', "\r\n")
            } else {
                normalized
            }
        );
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .truncate(false)
            .open(&path)
            .map_err(|e| e.to_string())?;
        file.set_len(0).map_err(|e| e.to_string())?;
        file.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        let proposed_revision = revision(text.as_bytes());
        let baseline = self.manifest.files.get_mut(&key).unwrap();
        baseline.proposed_revision = if proposed_revision == baseline.base_revision {
            None
        } else {
            Some(proposed_revision.clone())
        };
        self.persist()?;
        Ok(
            json!({"workspaceId":self.id(),"path":key,"revision":proposed_revision,"changedFiles":self.changed_count(),"applied":false}),
        )
    }
    pub fn changed_count(&self) -> usize {
        self.manifest
            .files
            .values()
            .filter(|f| f.proposed_revision.is_some())
            .count()
    }
    /// Adopt bounded text-file changes, additions and deletions made by an
    /// approved command. Absence is recorded separately from empty content.
    pub fn reconcile_command(&mut self) -> Result<Vec<String>, String> {
        let work = Context::new(&self.work_root(), &json!([]))?;
        let mut notes = Vec::new();
        let mut files = self.manifest.files.clone();
        let mut changed = 0;
        for (path, baseline) in &mut files {
            let bytes = optional_read(&work, path, FILE_LIMIT);
            match bytes {
                Ok(bytes) if state_revision(bytes.as_deref()) == baseline.base_revision => {
                    baseline.proposed_revision = None;
                }
                Ok(bytes)
                    if bytes.as_deref().is_none_or(reviewable)
                        && (baseline.base_revision == ABSENT
                            || read(&self.folder.join("base").join(path), 128 * 1024)
                                .is_ok_and(|bytes| reviewable(&bytes))) =>
                {
                    let hash = state_revision(bytes.as_deref());
                    if hash == baseline.base_revision {
                        baseline.proposed_revision = None;
                    } else if changed < 16 {
                        baseline.proposed_revision = Some(hash);
                        changed += 1;
                    } else {
                        baseline.proposed_revision = None;
                        notes.push(format!(
                            "Not included in editor review (16-file limit): {path}"
                        ));
                    }
                }
                _ => {
                    baseline.proposed_revision = None;
                    notes.push(format!("Linked, binary, inaccessible or oversized change remains only in the working copy: {path}"));
                }
            }
        }
        let mut directories = vec![self.work_root()];
        let mut visited = 0;
        while let Some(directory) = directories.pop() {
            for entry in std::fs::read_dir(directory).map_err(|e| e.to_string())? {
                visited += 1;
                if visited > 5000 {
                    notes.push(
                        "New-file scan reached 5,000 entries; review may be incomplete.".into(),
                    );
                    break;
                }
                let path = entry.map_err(|e| e.to_string())?.path();
                if excluded(
                    path.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or_default(),
                ) || crate::project_store::linked(&path)
                {
                    continue;
                }
                if path.is_dir() {
                    directories.push(path);
                } else if let Ok(relative) = path.strip_prefix(self.work_root()) {
                    let key = relative_key(relative)?;
                    if let std::collections::btree_map::Entry::Vacant(entry) = files.entry(key) {
                        match optional_read(&work, entry.key(), 128 * 1024) {
                            Ok(Some(bytes)) if reviewable(&bytes) && changed < 16 => {
                                entry.insert(Baseline {
                                    disk_revision: ABSENT.into(),
                                    base_revision: ABSENT.into(),
                                    source: "new".into(),
                                    proposed_revision: Some(revision(&bytes)),
                                    saved_revision: None,
                                });
                                changed += 1;
                            }
                            _ => notes.push(format!("New linked, binary, oversized or excess file remains only in the working copy: {}", entry.key())),
                        }
                    }
                }
            }
            if visited > 5000 {
                break;
            }
        }
        self.manifest.files = files;
        self.persist()?;
        notes.truncate(32);
        Ok(notes)
    }
    pub fn review(&self) -> Result<Value, String> {
        let source = Context::new(&self.root, &json!([]))?;
        let base = Context::new(&self.folder.join("base"), &json!([]))?;
        let work = Context::new(&self.work_root(), &json!([]))?;
        let mut changes = Vec::new();
        for (path, baseline) in &self.manifest.files {
            let Some(expected) = &baseline.proposed_revision else {
                continue;
            };
            let before = optional_read(&base, path, 128 * 1024)?;
            let after = optional_read(&work, path, 128 * 1024)?;
            if state_revision(before.as_deref()) != baseline.base_revision
                || state_revision(after.as_deref()) != *expected
            {
                return Err("The isolated proposal changed outside this run. Review cannot use an unverified baseline.".into());
            }
            let live = optional_read(&source, path, 128 * 1024);
            let conflict = live.as_ref().map_or(true, |bytes| {
                state_revision(bytes.as_deref())
                    != baseline
                        .saved_revision
                        .as_deref()
                        .unwrap_or(&baseline.disk_revision)
            });
            let operation = if before.is_none() {
                "create"
            } else if after.is_none() {
                "delete"
            } else {
                "modify"
            };
            let before = String::from_utf8(before.unwrap_or_default())
                .map_err(|_| "Only UTF-8 proposals can be reviewed.")?;
            let after = String::from_utf8(after.unwrap_or_default())
                .map_err(|_| "Only UTF-8 proposals can be reviewed.")?;
            changes.push(json!({"path":path,"absolutePath":self.root.join(path),"operation":operation,"before":before,"after":after,"diskBefore":live.ok().flatten().and_then(|b|String::from_utf8(b).ok()),"diskRevision":baseline.disk_revision,"revision":expected,"source":baseline.source,"conflict":conflict,"saved":baseline.saved_revision.is_some()}));
        }
        Ok(json!({"workspaceId":self.id(),"state":self.manifest.state,"changes":changes}))
    }
    pub fn mark_applied_to_editor(&mut self) -> Result<(), String> {
        if self.review()?["changes"].as_array().is_some_and(|changes| {
            changes
                .iter()
                .any(|file| file["operation"] != "modify" || file["conflict"] == true)
        }) {
            return Err(
                "Resolve conflicts and unsupported file operations before applying to editors."
                    .into(),
            );
        }
        self.manifest.state = "applied_to_editor".into();
        self.persist()
    }
    pub fn save_structural_change(&mut self, args: &Value) -> Result<Value, String> {
        if self.manifest.state != "applied_to_editor" {
            return Err("Apply the reviewed change to an editor before saving it.".into());
        }
        let path = args["path"].as_str().ok_or("Choose a reviewed file.")?;
        let baseline = self
            .manifest
            .files
            .get(path)
            .ok_or("File is not part of this review.")?;
        if baseline.saved_revision.is_some() {
            return Err(
                "This structural change was already saved. Reopen the live file to edit it.".into(),
            );
        }
        let review = self.review()?;
        let file = review["changes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|file| file["path"] == path)
            .ok_or("File has no reviewed change.")?;
        if file["conflict"] == true || args["revision"] != file["revision"] {
            return Err("The reviewed file changed. Its live contents were preserved.".into());
        }
        let saved_revision = match file["operation"].as_str() {
            Some("create") => {
                let text = args["text"].as_str().ok_or("Missing editor text.")?;
                super::file_commit::create(&self.root, path, text)?;
                revision(text.as_bytes())
            }
            Some("delete") => {
                if args["text"] != file["before"] {
                    return Err("Editor text changed after the deletion review. Its contents were preserved.".into());
                }
                super::file_commit::delete(&self.root, path, &baseline.disk_revision)?;
                ABSENT.into()
            }
            _ => return Err("Use the normal editor Save action for an existing-file edit.".into()),
        };
        self.manifest.files.get_mut(path).unwrap().saved_revision = Some(saved_revision);
        self.persist().map_err(|e| format!("File operation completed, but recording its saved state failed: {e}. Inspect the live file before retrying."))?;
        Ok(json!({"path":self.root.join(path),"operation":file["operation"],"saved":true}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[cfg(windows)]
    fn structural_saves_require_staging_preserve_conflicts_and_cannot_replay() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("old.txt"), "baseline\r\n").unwrap();
        std::fs::write(root.path().join("keep.txt"), "unrelated").unwrap();
        let project = uuid::Uuid::new_v4().to_string();
        let mut snapshot = Snapshot::create(
            root.path(),
            &project,
            "structural-save",
            &json!([]),
            &std::sync::atomic::AtomicBool::new(false),
        )
        .unwrap();
        std::fs::create_dir(snapshot.work_root().join("notes")).unwrap();
        std::fs::write(snapshot.work_root().join("notes/new.txt"), "proposal").unwrap();
        std::fs::remove_file(snapshot.work_root().join("old.txt")).unwrap();
        snapshot.reconcile_command().unwrap();
        let create = json!({"path":"notes/new.txt","revision":revision(b"proposal"),"text":"edited in the editor\r\n"});
        let mut delete = json!({"path":"old.txt","revision":ABSENT,"text":"baseline\r\n"});
        assert!(snapshot.save_structural_change(&create).is_err());
        assert!(!root.path().join("notes").exists());
        // The editor handoff owns this transition; its UI wiring is separate.
        snapshot.manifest.state = "applied_to_editor".into();
        snapshot.persist().unwrap();
        let mut stale = create.clone();
        stale["revision"] = json!("stale");
        assert!(snapshot.save_structural_change(&stale).is_err());
        snapshot.save_structural_change(&create).unwrap();
        assert_eq!(
            std::fs::read_to_string(root.path().join("notes/new.txt")).unwrap(),
            "edited in the editor\r\n"
        );
        std::fs::write(root.path().join("old.txt"), "concurrent").unwrap();
        assert!(snapshot.save_structural_change(&delete).is_err());
        assert_eq!(
            std::fs::read_to_string(root.path().join("old.txt")).unwrap(),
            "concurrent"
        );
        std::fs::write(root.path().join("old.txt"), "baseline\r\n").unwrap();
        delete["text"] = json!("unreviewed editor change");
        assert!(snapshot.save_structural_change(&delete).is_err());
        delete["text"] = json!("baseline\r\n");
        snapshot.save_structural_change(&delete).unwrap();
        assert!(!root.path().join("old.txt").exists());
        let mut recovered =
            Snapshot::load(root.path(), &project, "structural-save", snapshot.id()).unwrap();
        assert!(recovered.save_structural_change(&create).is_err());
        assert!(recovered.save_structural_change(&delete).is_err());
        let review = recovered.review().unwrap();
        assert!(
            review["changes"]
                .as_array()
                .unwrap()
                .iter()
                .all(|f| f["saved"] == true && f["conflict"] == false)
        );
        assert_eq!(
            std::fs::read_to_string(root.path().join("keep.txt")).unwrap(),
            "unrelated"
        );
    }
    #[test]
    fn command_reconciliation_reviews_text_changes_and_reports_unsupported_changes() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("main.rs"), "disk\n").unwrap();
        std::fs::write(root.path().join("delete.txt"), "delete").unwrap();
        std::fs::write(root.path().join("binary.dat"), [0u8, 1, 2]).unwrap();
        let mut snapshot = Snapshot::create(
            root.path(),
            &uuid::Uuid::new_v4().to_string(),
            "command-review",
            &json!([{"path":root.path().join("main.rs"),"text":"unsaved\n","original":"disk\n"}]),
            &std::sync::atomic::AtomicBool::new(false),
        )
        .unwrap();
        std::fs::write(snapshot.work_root().join("main.rs"), "checked\n").unwrap();
        std::fs::write(snapshot.work_root().join("new.txt"), "new").unwrap();
        std::fs::remove_file(snapshot.work_root().join("delete.txt")).unwrap();
        let notes = snapshot.reconcile_command().unwrap();
        assert!(!notes.iter().any(|n| n.contains("new.txt")));
        assert!(!notes.iter().any(|n| n.contains("delete.txt")));
        assert!(!notes.iter().any(|n| n.contains("binary.dat")));
        let review = snapshot.review().unwrap();
        assert_eq!(review["changes"].as_array().unwrap().len(), 3);
        let changes = review["changes"].as_array().unwrap();
        let edited = changes.iter().find(|f| f["path"] == "main.rs").unwrap();
        assert_eq!(edited["before"], "unsaved\n");
        assert_eq!(edited["after"], "checked\n");
        assert_eq!(edited["operation"], "modify");
        assert_eq!(edited["conflict"], false);
        let added = changes.iter().find(|f| f["path"] == "new.txt").unwrap();
        assert_eq!(added["operation"], "create");
        assert_eq!(added["before"], "");
        assert!(added["diskBefore"].is_null());
        assert_eq!(added["conflict"], false);
        let deleted = changes.iter().find(|f| f["path"] == "delete.txt").unwrap();
        assert_eq!(deleted["operation"], "delete");
        assert_eq!(deleted["before"], "delete");
        assert_eq!(deleted["after"], "");
        assert_eq!(deleted["conflict"], false);
        assert_eq!(
            std::fs::read_to_string(root.path().join("main.rs")).unwrap(),
            "disk\n"
        );
        assert!(root.path().join("delete.txt").exists());
        assert!(!root.path().join("new.txt").exists());
    }
    #[test]
    fn additions_and_deletions_keep_absence_distinct_and_detect_external_changes() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("empty.txt"), "").unwrap();
        let project = uuid::Uuid::new_v4().to_string();
        let mut snapshot = Snapshot::create(
            root.path(),
            &project,
            "tree-changes",
            &json!([]),
            &std::sync::atomic::AtomicBool::new(false),
        )
        .unwrap();
        std::fs::create_dir(snapshot.work_root().join("notes")).unwrap();
        std::fs::write(snapshot.work_root().join("notes/new.txt"), "").unwrap();
        std::fs::remove_file(snapshot.work_root().join("empty.txt")).unwrap();
        snapshot.reconcile_command().unwrap();
        let mut snapshot =
            Snapshot::load(root.path(), &project, "tree-changes", snapshot.id()).unwrap();
        let review = snapshot.review().unwrap();
        assert_eq!(review["changes"][0]["operation"], "delete");
        assert_eq!(review["changes"][1]["operation"], "create");
        assert!(
            review["changes"]
                .as_array()
                .unwrap()
                .iter()
                .all(|f| f["conflict"] == false)
        );
        assert!(snapshot.mark_applied_to_editor().is_err());
        std::fs::create_dir(root.path().join("notes")).unwrap();
        std::fs::write(root.path().join("notes/new.txt"), "someone else's file").unwrap();
        std::fs::write(root.path().join("empty.txt"), "concurrent edit").unwrap();
        assert!(
            snapshot.review().unwrap()["changes"]
                .as_array()
                .unwrap()
                .iter()
                .all(|f| f["conflict"] == true)
        );
        std::fs::write(snapshot.work_root().join("notes/new.txt"), "tampered").unwrap();
        assert!(snapshot.review().is_err());
        assert_eq!(
            std::fs::read_to_string(root.path().join("notes/new.txt")).unwrap(),
            "someone else's file"
        );
    }
    #[test]
    fn command_additions_share_the_review_limit_and_disappear_when_removed() {
        let root = tempfile::tempdir().unwrap();
        let mut snapshot = Snapshot::create(
            root.path(),
            &uuid::Uuid::new_v4().to_string(),
            "bounded-new-files",
            &json!([]),
            &std::sync::atomic::AtomicBool::new(false),
        )
        .unwrap();
        for index in 0..18 {
            std::fs::write(snapshot.work_root().join(format!("new-{index}.txt")), "new").unwrap();
        }
        std::fs::write(snapshot.work_root().join(".env"), "PRIVATE").unwrap();
        let notes = snapshot.reconcile_command().unwrap();
        assert_eq!(snapshot.changed_count(), 16);
        assert_eq!(notes.len(), 2);
        assert_eq!(
            snapshot.review().unwrap()["changes"]
                .as_array()
                .unwrap()
                .len(),
            16
        );
        for index in 0..18 {
            std::fs::remove_file(snapshot.work_root().join(format!("new-{index}.txt"))).unwrap();
        }
        assert!(snapshot.reconcile_command().unwrap().is_empty());
        assert_eq!(snapshot.changed_count(), 0);
        assert!(
            snapshot.review().unwrap()["changes"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert!(!root.path().join(".env").exists());
    }
    #[test]
    fn cancelled_construction_removes_only_its_incomplete_owned_workspace() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("keep.txt"), "keep").unwrap();
        let cancelled = std::sync::atomic::AtomicBool::new(true);
        assert!(
            Snapshot::create(
                root.path(),
                &uuid::Uuid::new_v4().to_string(),
                "cancelled",
                &json!([]),
                &cancelled
            )
            .is_err()
        );
        assert_eq!(
            std::fs::read_dir(root.path().join(".dado/workspaces"))
                .unwrap()
                .count(),
            0
        );
        assert_eq!(
            std::fs::read_to_string(root.path().join("keep.txt")).unwrap(),
            "keep"
        );
    }
    #[test]
    fn isolated_proposal_preserves_dirty_untracked_and_live_files_and_detects_conflict() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("main.rs");
        std::fs::write(&file, "disk\r\n").unwrap();
        std::fs::write(root.path().join("untracked.txt"), "untracked").unwrap();
        std::fs::write(root.path().join(".env"), "PRIVATE").unwrap();
        let draft = root.path().join(".dado/skill-drafts/SKILL.md");
        std::fs::create_dir_all(draft.parent().unwrap()).unwrap();
        std::fs::write(&draft, "PRIVATE_DRAFT").unwrap();
        let project = uuid::Uuid::new_v4().to_string();
        let mut snapshot = Snapshot::create(
            root.path(),
            &project,
            "run-1",
            &json!([{"path":file,"text":"unsaved\r\n","original":"disk\r\n"},{"path":draft,"text":"UNSAVED_PRIVATE_DRAFT","original":"PRIVATE_DRAFT"}]),
            &std::sync::atomic::AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(
            std::fs::read_to_string(snapshot.work_root().join("untracked.txt")).unwrap(),
            "untracked"
        );
        assert!(!snapshot.work_root().join(".env").exists());
        assert!(!snapshot.work_root().join(".dado").exists());
        snapshot.propose(&json!({"path":"main.rs","expectedRevision":revision(b"unsaved\r\n"),"text":"proposed\n"})).unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "disk\r\n");
        let review = snapshot.review().unwrap();
        assert_eq!(review["changes"][0]["before"], "unsaved\r\n");
        assert_eq!(review["changes"][0]["after"], "proposed\r\n");
        assert_eq!(review["changes"][0]["conflict"], false);
        std::fs::write(&file, "external change").unwrap();
        assert_eq!(snapshot.review().unwrap()["changes"][0]["conflict"], true);
        let loaded = Snapshot::load(root.path(), &project, "run-1", snapshot.id()).unwrap();
        assert_eq!(loaded.changed_count(), 1);
        assert!(Snapshot::load(root.path(), &project, "other-run", snapshot.id()).is_err());
        assert!(
            snapshot
                .propose(&json!({"path":"../main.rs","expectedRevision":"x","text":"bad"}))
                .is_err()
        );
    }
}
