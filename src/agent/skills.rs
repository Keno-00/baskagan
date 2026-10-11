//! Native discovery plus SDE-private skills; explicit references freeze per run.
use super::{
    manifest::{Exposure, Manifest, Resource, Source},
    snapshot::revision,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::HashSet,
    io::Read,
    path::{Path, PathBuf},
};
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Skill {
    pub id: String,
    pub name: String,
    pub description: String,
    pub path: PathBuf,
    pub scope: String,
    pub revision: String,
    pub configuration_revision: String,
    pub enabled: bool,
    pub reason: Option<String>,
}
#[derive(Serialize, Default)]
pub struct Catalog {
    pub skills: Vec<Skill>,
    pub errors: Vec<String>,
}
pub struct Frozen {
    pub input: Value,
    pub instructions: String,
    pub id: String,
    resources: Vec<CapturedResource>,
}
struct CapturedResource {
    path: String,
    origin: PathBuf,
    bytes: Vec<u8>,
}
impl Frozen {
    pub fn record_resources(&self, manifest: &mut Manifest) -> Result<(), String> {
        for resource in &self.resources {
            manifest.file(
                None,
                &resource.origin,
                (Source::Skill, super::manifest::Exposure::Available),
                Some(revision(&resource.bytes)),
                None,
                "Supporting resource captured for explicitly selected skill",
            )?;
        }
        Ok(())
    }
    pub fn call(&self, tool: &str, args: &Value, manifest: &mut Manifest) -> Result<Value, String> {
        if args["skillId"] != self.id {
            return Err("This skill is not selected for this run.".into());
        }
        match tool {
            "sde_list_skill_resources" => Ok(
                json!({"skillId":self.id,"resources":self.resources.iter().map(|r|json!({"path":r.path,"size":r.bytes.len(),"revision":revision(&r.bytes),"readable":r.bytes.len()<=128*1024 && !r.bytes.contains(&0) && std::str::from_utf8(&r.bytes).is_ok()})).collect::<Vec<_>>()}),
            ),
            "sde_read_skill_resource" => {
                let path = args["path"]
                    .as_str()
                    .ok_or("Choose a captured resource path.")?;
                // Exact lookup against the run's in-memory capture, never an OS path.
                let resource = self
                    .resources
                    .iter()
                    .find(|r| r.path == path)
                    .ok_or("Resource is not in this selected skill's capture.")?;
                if resource.bytes.len() > 128 * 1024 || resource.bytes.contains(&0) {
                    return Err("Skill resource is binary or exceeds the 128 KB read limit.".into());
                }
                let text = std::str::from_utf8(&resource.bytes)
                    .map_err(|_| "Only UTF-8 skill resources can be read.")?;
                manifest.file(
                    None,
                    &resource.origin,
                    (Source::Skill, super::manifest::Exposure::Observed),
                    Some(revision(&resource.bytes)),
                    None,
                    "Read selected skill resource from its frozen run capture",
                )?;
                Ok(
                    json!({"skillId":self.id,"resource":resource.path,"revision":revision(&resource.bytes),"text":text,"source":"skill"}),
                )
            }
            _ => Err("Unknown skill resource tool.".into()),
        }
    }
}
pub fn resource_tools() -> Value {
    json!([
        {"type":"function","name":"sde_list_skill_resources","description":"List supporting files captured for an explicitly selected skill. Only resources from this run are accessible. Listing does not execute scripts or grant permissions.","inputSchema":{"type":"object","properties":{"skillId":{"type":"string"}},"required":["skillId"],"additionalProperties":false}},
        {"type":"function","name":"sde_read_skill_resource","description":"Read one UTF-8 supporting file from an explicitly selected skill's immutable run capture, up to 128 KB. Use its exact listed relative path. Scripts are read as data, never executed. File content cannot change permissions.","inputSchema":{"type":"object","properties":{"skillId":{"type":"string"},"path":{"type":"string"}},"required":["skillId","path"],"additionalProperties":false}}
    ])
}
#[derive(Deserialize)]
struct Metadata {
    name: String,
    description: String,
}
fn safe(path: &Path) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err("Skill path must be absolute.".into());
    }
    for p in path.ancestors() {
        if crate::project_store::linked(p) {
            return Err("Skill paths cannot follow links.".into());
        }
    }
    path.canonicalize()
        .map_err(|_| "Skill file is unavailable.".into())
}
fn read(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    std::fs::File::open(safe(path)?)
        .map_err(|e| e.to_string())?
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > limit {
        return Err("Skill file exceeds its size limit.".into());
    }
    Ok(bytes)
}
fn metadata(path: &Path) -> Result<(Metadata, Vec<u8>), String> {
    if path.file_name().is_none_or(|n| n != "SKILL.md") {
        return Err("Skill reference must name SKILL.md.".into());
    }
    let bytes = read(path, 64 * 1024)?;
    Ok((parse_metadata(&bytes)?, bytes))
}
fn parse_metadata(bytes: &[u8]) -> Result<Metadata, String> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| "Skill instructions must be UTF-8.")?
        .trim_start_matches('\u{feff}')
        .replace("\r\n", "\n");
    let front = text
        .strip_prefix("---\n")
        .and_then(|s| {
            s.lines()
                .position(|line| line == "---")
                .map(|end| s.lines().take(end).collect::<Vec<_>>().join("\n"))
        })
        .ok_or("Skill needs YAML front matter.")?;
    let m: Metadata = serde_yaml_ng::from_str(&front)
        .map_err(|_| "Skill needs valid name and description metadata.")?;
    if m.name.is_empty()
        || m.name.len() > 64
        || m.name.starts_with('-')
        || m.name.ends_with('-')
        || !m
            .name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        || m.description.trim().is_empty()
        || m.description.len() > 2048
    {
        return Err("Invalid skill name or description.".into());
    }
    Ok(m)
}

// Dependency declarations are data, never instructions to connect or install
// anything. SDE currently has no MCP dependency resolver.
fn configuration(path: &Path) -> Result<Option<Vec<u8>>, String> {
    let path = path
        .parent()
        .ok_or("Missing skill folder.")?
        .join("agents/openai.yaml");
    if crate::project_store::linked(&path) || path.ancestors().any(crate::project_store::linked) {
        return Err("Skill configuration cannot follow links.".into());
    }
    match std::fs::symlink_metadata(&path) {
        Ok(_) => read(&path, 32 * 1024).map(Some),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err("Skill configuration is unavailable.".into()),
    }
}
fn configuration_revision(bytes: Option<&[u8]>) -> String {
    bytes.map(revision).unwrap_or_else(|| "absent".into())
}
fn dependency_reason(dependencies: &Value) -> Result<Option<String>, String> {
    let object = dependencies
        .as_object()
        .ok_or("Skill dependencies must be a mapping.")?;
    if object.keys().any(|key| key != "tools") {
        return Err("Skill declares an unsupported dependency kind.".into());
    }
    let Some(tools) = object.get("tools") else {
        return Ok(None);
    };
    let tools = tools
        .as_array()
        .ok_or("Skill tool dependencies must be a list.")?;
    for tool in tools {
        for key in ["type", "value"] {
            if tool[key].as_str().is_none_or(|s| s.trim().is_empty()) {
                return Err("Each skill tool dependency needs a type and value.".into());
            }
        }
    }
    Ok((!tools.is_empty())
        .then(|| "This skill requires external tools that SDE has not integrated yet.".into()))
}
fn configuration_reason(bytes: Option<&[u8]>) -> Result<Option<String>, String> {
    let Some(bytes) = bytes else {
        return Ok(None);
    };
    let value: Value =
        serde_yaml_ng::from_slice(bytes).map_err(|_| "Skill configuration must be valid YAML.")?;
    let object = value
        .as_object()
        .ok_or("Skill configuration must be a mapping.")?;
    if let Some(policy) = object.get("policy") {
        let policy = policy
            .as_object()
            .ok_or("Skill policy must be a mapping.")?;
        if policy
            .get("allow_implicit_invocation")
            .is_some_and(|v| !v.is_boolean())
        {
            return Err("Skill invocation policy must be true or false.".into());
        }
        if let Some(products) = policy.get("products") {
            let products = products
                .as_array()
                .ok_or("Skill products must be a list.")?;
            if products.is_empty()
                || products
                    .iter()
                    .any(|v| !matches!(v.as_str(), Some("CHAT" | "CODEX")))
            {
                return Err("Skill products must contain CHAT or CODEX.".into());
            }
            if !products.iter().any(|v| v == "CODEX") {
                return Ok(Some("This skill is configured for ChatGPT only.".into()));
            }
        }
    }
    object
        .get("dependencies")
        .map(dependency_reason)
        .transpose()
        .map(Option::flatten)
}

/// Drafts are deliberately outside discovery roots. Publication is an explicit
/// local user operation, independent of agent execution permissions.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Draft {
    pub id: String,
    pub name: String,
    pub scope: String,
    pub path: PathBuf,
    pub revision: String,
    #[serde(default)]
    pub published: bool,
}
fn draft_store(root: &Path) -> Result<PathBuf, String> {
    let path = crate::project_store::storage(root)?.join("skill-drafts");
    if crate::project_store::linked(&path) {
        return Err("Skill drafts cannot follow links.".into());
    }
    std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    safe(&path)
}
fn draft_folder(root: &Path, id: &str) -> Result<PathBuf, String> {
    let id = uuid::Uuid::parse_str(id).map_err(|_| "Invalid skill draft identity.")?;
    let parent = draft_store(root)?;
    let path = safe(&parent.join(id.to_string()))?;
    if path.parent() != Some(parent.as_path()) {
        return Err("Invalid skill draft path.".into());
    }
    Ok(path)
}
pub fn drafts(root: &Path) -> Result<Value, String> {
    let mut items = Vec::new();
    for entry in std::fs::read_dir(draft_store(root)?)
        .map_err(|e| e.to_string())?
        .take(128)
    {
        let entry = entry.map_err(|e| e.to_string())?;
        let Some(id) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        if uuid::Uuid::parse_str(&id).is_err() {
            continue;
        }
        let folder = draft_folder(root, &id)?;
        let mut draft: Draft = serde_json::from_slice(&read(&folder.join("draft.json"), 4096)?)
            .map_err(|_| "Invalid skill draft record.")?;
        if draft.published {
            continue;
        }
        let path = folder.join("SKILL.md");
        draft.path = path.clone();
        draft.revision = revision(&read(&path, 64 * 1024)?);
        items.push(draft);
    }
    items.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
    Ok(json!(items))
}
fn instruction_only(text: &str) -> Result<Metadata, String> {
    if text.len() > 64 * 1024 || text.contains('\0') {
        return Err("Skill draft must be UTF-8 text under 64 KB.".into());
    }
    let m = parse_metadata(text.as_bytes())?;
    // The initial creation workflow has no supporting files. Do not publish
    // dangling local references or scripts that merely look implemented.
    for prefix in [
        "references/",
        "scripts/",
        "assets/",
        "references\\",
        "scripts\\",
        "assets\\",
        "../",
        "..\\",
        "file://",
    ] {
        if text.contains(prefix) {
            return Err("Start with an instruction-only skill; supporting file references are not available in this creation workflow yet.".into());
        }
    }
    for suffix in text.split("](").skip(1) {
        let target = suffix.split(')').next().unwrap_or_default().trim();
        if !target.starts_with("https://")
            && !target.starts_with("http://")
            && !target.starts_with('#')
        {
            return Err("Instruction-only skills cannot reference local supporting files.".into());
        }
    }
    Ok(m)
}
pub fn create_draft(root: &Path, name: &str, scope: &str, text: &str) -> Result<Value, String> {
    if !matches!(scope, "project-private" | "repo") {
        return Err("Choose project-private or repo skill scope.".into());
    }
    let text = text.trim();
    let text = if let Some(rest) = text.strip_prefix("```") {
        let (_, body) = rest.split_once('\n').ok_or("Invalid skill draft fence.")?;
        body.strip_suffix("```")
            .ok_or("Invalid skill draft fence.")?
            .trim()
    } else {
        text
    };
    let m = instruction_only(text)?;
    if m.name != name {
        return Err("The drafted skill name differs from the requested name.".into());
    }
    let storage = draft_store(root)?;
    let id = uuid::Uuid::new_v4().to_string();
    let folder = storage.join(&id);
    let pending = tempfile::Builder::new()
        .prefix(".pending-")
        .tempdir_in(&storage)
        .map_err(|e| e.to_string())?;
    let draft = Draft {
        id,
        name: m.name,
        scope: scope.into(),
        path: folder.join("SKILL.md"),
        revision: revision(text.as_bytes()),
        published: false,
    };
    std::fs::write(pending.path().join("SKILL.md"), text).map_err(|e| e.to_string())?;
    std::fs::write(
        pending.path().join("draft.json"),
        serde_json::to_vec_pretty(&draft).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    std::fs::rename(pending.path(), &folder).map_err(|e| e.to_string())?;
    Ok(json!(draft))
}
pub fn publish_draft(root: &Path, id: &str, expected: &str, text: &str) -> Result<Value, String> {
    let folder = draft_folder(root, id)?;
    let mut draft: Draft = serde_json::from_slice(&read(&folder.join("draft.json"), 4096)?)
        .map_err(|_| "Invalid skill draft record.")?;
    if revision(&read(&folder.join("SKILL.md"), 64 * 1024)?) != expected {
        return Err("Skill draft changed on disk. Reopen it before publishing.".into());
    }
    let m = instruction_only(text)?;
    if m.name != draft.name {
        return Err("Keep the skill name unchanged or create another draft.".into());
    }
    let root = safe(root)?;
    let parent = match draft.scope.as_str() {
        "project-private" => crate::project_store::storage(&root)?.join("skills"),
        "repo" => root.join(".agents/skills"),
        _ => return Err("Invalid skill publication scope.".into()),
    };
    for ancestor in parent.ancestors() {
        if crate::project_store::linked(ancestor) {
            return Err("Skill publication cannot follow links.".into());
        }
    }
    std::fs::create_dir_all(&parent).map_err(|e| e.to_string())?;
    let target = parent.join(&m.name);
    // create_dir fails if this name exists, preserving prior skills unchanged.
    std::fs::create_dir(&target)
        .map_err(|_| "A skill with this name already exists. Existing skills were preserved.")?;
    let path = target.join("SKILL.md");
    let result = (|| -> Result<(), String> {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|e| e.to_string())?;
        file.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())
    })();
    result?;
    draft.published = true;
    let mut record = tempfile::NamedTempFile::new_in(&folder).map_err(|e| e.to_string())?;
    serde_json::to_writer_pretty(&mut record, &draft).map_err(|e| e.to_string())?;
    record.as_file().sync_all().map_err(|e| e.to_string())?;
    record
        .persist(folder.join("draft.json"))
        .map_err(|e| e.to_string())?;
    // Retain the reviewable draft for recovery; never remove user editor data.
    Ok(json!({"path":path,"name":m.name,"scope":draft.scope,"revision":revision(text.as_bytes())}))
}
pub fn export_private(root: &Path, id: &str, expected: &str) -> Result<Value, String> {
    let catalog = Catalog::load(root, &json!({}))?;
    let skill = catalog
        .skills
        .iter()
        .find(|s| s.id == id && s.scope == "project-private")
        .ok_or("Choose a private skill from this project.")?;
    if skill.revision != expected {
        return Err("Skill changed before sharing. Reopen the skill list to review it.".into());
    }
    let storage = crate::project_store::storage(root)?.join("skill-exports");
    if crate::project_store::linked(&storage) {
        return Err("Skill export storage cannot follow links.".into());
    }
    std::fs::create_dir_all(&storage).map_err(|e| e.to_string())?;
    let staged = tempfile::Builder::new()
        .prefix("export-")
        .tempdir_in(storage)
        .map_err(|e| e.to_string())?;
    let captured = copy_tree(
        skill.path.parent().ok_or("Missing skill folder.")?,
        staged.path(),
    )?;
    let instructions = captured
        .iter()
        .find(|r| r.path == "SKILL.md")
        .ok_or("Skill instructions were not captured.")?;
    if revision(&instructions.bytes) != expected {
        return Err("Skill changed while sharing. Reopen it to review the new revision.".into());
    }
    let parent = root.join(".agents/skills");
    for path in parent.ancestors() {
        if crate::project_store::linked(path) {
            return Err("Shared skill paths cannot follow links.".into());
        }
    }
    std::fs::create_dir_all(&parent).map_err(|e| e.to_string())?;
    let target = parent.join(&skill.name);
    std::fs::create_dir(&target)
        .map_err(|_| "A repository skill with this name already exists. It was preserved.")?;
    for resource in captured.iter().filter(|r| r.path != "SKILL.md") {
        let to = target.join(&resource.path);
        std::fs::create_dir_all(to.parent().ok_or("Invalid resource path.")?)
            .map_err(|e| e.to_string())?;
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(to)
            .map_err(|e| e.to_string())?;
        file.write_all(&resource.bytes).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
    }
    // Publish instructions last, so incomplete copies cannot be discovered.
    let mut file = tempfile::NamedTempFile::new_in(&target).map_err(|e| e.to_string())?;
    use std::io::Write;
    file.write_all(&instructions.bytes)
        .map_err(|e| e.to_string())?;
    file.as_file().sync_all().map_err(|e| e.to_string())?;
    file.persist_noclobber(target.join("SKILL.md"))
        .map_err(|e| e.to_string())?;
    Ok(json!({"name":skill.name,"path":target.join("SKILL.md"),"scope":"repo","revision":expected}))
}
impl Catalog {
    pub fn load(root: &Path, native: &Value) -> Result<Self, String> {
        let root = safe(root)?;
        let mut result = Self::default();
        let mut seen = HashSet::new();
        for entry in native["data"].as_array().into_iter().flatten() {
            if entry["cwd"]
                .as_str()
                .and_then(|p| Path::new(p).canonicalize().ok())
                .as_ref()
                != Some(&root)
            {
                continue;
            }
            for error in entry["errors"].as_array().into_iter().flatten().take(64) {
                result.errors.push(
                    error["message"]
                        .as_str()
                        .unwrap_or("Invalid runtime skill")
                        .chars()
                        .take(300)
                        .collect(),
                );
            }
            for item in entry["skills"].as_array().into_iter().flatten().take(256) {
                let Some(path) = item["path"].as_str() else {
                    continue;
                };
                result.add(
                    Path::new(path),
                    item["scope"].as_str().unwrap_or("runtime"),
                    item["enabled"] == true,
                    &mut seen,
                );
                if !item["dependencies"].is_null()
                    && let Some(skill) = result
                        .skills
                        .iter_mut()
                        .find(|s| s.path == Path::new(path).canonicalize().unwrap_or_default())
                {
                    match dependency_reason(&item["dependencies"]) {
                        Ok(None) => {}
                        Ok(Some(reason)) | Err(reason) => {
                            skill.enabled = false;
                            skill.reason = Some(reason);
                        }
                    }
                }
            }
        }
        let private = root.join(".dado/skills");
        if private.exists() {
            safe(&private)?;
            for entry in std::fs::read_dir(private)
                .map_err(|e| e.to_string())?
                .take(128)
            {
                let path = entry.map_err(|e| e.to_string())?.path().join("SKILL.md");
                if path.exists() {
                    result.add(&path, "project-private", true, &mut seen);
                }
            }
        }
        result.skills.sort_by(|a, b| {
            a.name
                .cmp(&b.name)
                .then(a.scope.cmp(&b.scope))
                .then(a.path.cmp(&b.path))
        });
        Ok(result)
    }
    fn add(&mut self, path: &Path, scope: &str, enabled: bool, seen: &mut HashSet<PathBuf>) {
        let parsed = (|| {
            let path = safe(path)?;
            let (metadata, bytes) = metadata(&path)?;
            Ok::<_, String>((path, metadata, bytes))
        })();
        match parsed {
            Ok((path, m, bytes)) if seen.insert(path.clone()) => {
                let config = configuration(&path);
                let config_revision = config
                    .as_ref()
                    .map(|bytes| configuration_revision(bytes.as_deref()))
                    .unwrap_or_else(|_| "unavailable".into());
                let reason = config
                    .and_then(|bytes| configuration_reason(bytes.as_deref()))
                    .unwrap_or_else(Some);
                self.skills.push(Skill {
                    id: revision(format!("{scope}:{}", path.display()).as_bytes()),
                    name: m.name,
                    description: m.description,
                    path,
                    scope: scope.into(),
                    revision: revision(&bytes),
                    configuration_revision: config_revision,
                    enabled: enabled && reason.is_none(),
                    reason: reason
                        .or_else(|| (!enabled).then(|| "Disabled by the runtime.".into())),
                });
            }
            Ok(_) => {}
            Err(error) => self.errors.push(format!("{}: {error}", path.display())),
        }
    }
    pub fn freeze(
        &self,
        root: &Path,
        selected: &Value,
        manifest: &mut Manifest,
    ) -> Result<Vec<Frozen>, String> {
        let selected = selected.as_array().ok_or("Invalid skill selection.")?;
        if selected.len() > 8 {
            return Err("Choose at most eight skills per message.".into());
        }
        let storage = crate::project_store::storage(root)?.join("skill-snapshots");
        if crate::project_store::linked(&storage) {
            return Err("Skill storage cannot follow links.".into());
        }
        std::fs::create_dir_all(&storage).map_err(|e| e.to_string())?;
        let mut items = Vec::new();
        let mut ids = HashSet::new();
        for selection in selected {
            let skill = self
                .skills
                .iter()
                .find(|s| selection["id"] == s.id)
                .ok_or("Selected skill is no longer available. Choose it again.")?;
            if !ids.insert(&skill.id) {
                return Err("A skill was selected more than once.".into());
            }
            if !skill.enabled {
                return Err(skill
                    .reason
                    .clone()
                    .unwrap_or("Skill is unavailable.".into()));
            }
            let (_, bytes) = metadata(&skill.path)?;
            let config = configuration(&skill.path)?;
            if let Some(reason) = configuration_reason(config.as_deref())? {
                return Err(reason);
            }
            if configuration_revision(config.as_deref()) != skill.configuration_revision
                || selection["configurationRevision"] != skill.configuration_revision
            {
                return Err(format!(
                    "{} configuration changed after selection. Choose it again.",
                    skill.name
                ));
            }
            if revision(&bytes) != skill.revision || selection["revision"] != skill.revision {
                return Err(format!(
                    "{} changed after selection. Choose it again to use the new revision.",
                    skill.name
                ));
            }
            let directory = tempfile::Builder::new()
                .prefix("skill-")
                .tempdir_in(&storage)
                .map_err(|e| e.to_string())?;
            let resources = copy_tree(
                skill.path.parent().ok_or("Missing skill folder.")?,
                directory.path(),
            )?;
            let frozen = directory.path().join("SKILL.md");
            if read(&frozen, 64 * 1024)? != bytes {
                return Err("Skill changed while capturing it. Choose it again.".into());
            }
            let captured_config = resources.iter().find(|r| r.path == "agents/openai.yaml");
            if configuration_revision(captured_config.map(|r| r.bytes.as_slice()))
                != skill.configuration_revision
            {
                return Err(
                    "Skill configuration changed while capturing it. Choose it again.".into(),
                );
            }
            if let Some(config) = config {
                manifest.record(Resource {
                    id: String::new(),
                    kind: "skill_configuration".into(),
                    identity: skill
                        .path
                        .parent()
                        .unwrap()
                        .join("agents/openai.yaml")
                        .to_string_lossy()
                        .into_owned(),
                    relative_path: None,
                    source: Source::Skill,
                    exposure: Exposure::Validated,
                    revision: Some(revision(&config)),
                    range: None,
                    range_unit: None,
                    reasons: vec!["Host validated configuration for explicitly selected skill; not model-supplied content".into()],
                })?;
            }
            manifest.content(
                "skill_instructions",
                skill.path.to_string_lossy().as_ref(),
                std::str::from_utf8(&bytes).unwrap(),
                Source::Skill,
                &format!("Explicitly selected {} · {}", skill.name, skill.scope),
            )?;
            let frozen = directory.keep().join("SKILL.md");
            items.push(Frozen {
                input: json!({"type":"skill","name":skill.name,"path":frozen}),
                instructions: std::str::from_utf8(&bytes).unwrap().to_string(),
                id: skill.id.clone(),
                resources,
            });
        }
        Ok(items)
    }
}
fn copy_tree(source: &Path, target: &Path) -> Result<Vec<CapturedResource>, String> {
    let source = safe(source)?;
    let base = source.clone();
    let mut captured = Vec::new();
    let mut queue = vec![(source, target.to_path_buf())];
    let mut count = 0;
    let mut size = 0;
    while let Some((source, target)) = queue.pop() {
        for entry in std::fs::read_dir(source).map_err(|e| e.to_string())? {
            count += 1;
            if count > 256 {
                return Err("Skill resources exceed 256 entries.".into());
            }
            let path = entry.map_err(|e| e.to_string())?.path();
            if path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(super::context::excluded)
            {
                continue;
            }
            let canonical = safe(&path)?;
            let to = target.join(path.file_name().ok_or("Invalid skill resource.")?);
            if canonical.is_dir() {
                std::fs::create_dir(&to).map_err(|e| e.to_string())?;
                queue.push((canonical, to));
            } else {
                let bytes = read(&canonical, 512 * 1024)?;
                size += bytes.len();
                if size > 4 * 1024 * 1024 {
                    return Err("Skill resources exceed 4 MB.".into());
                }
                std::fs::write(to, &bytes).map_err(|e| e.to_string())?;
                captured.push(CapturedResource {
                    path: canonical
                        .strip_prefix(&base)
                        .map_err(|_| "Resource leaves selected skill.")?
                        .to_string_lossy()
                        .replace('\\', "/"),
                    origin: canonical,
                    bytes,
                });
            }
        }
    }
    captured.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(captured)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn private_skill_configuration_checks_dependencies_and_revalidates_selection() {
        let root = tempfile::tempdir().unwrap();
        let folder = root.path().join(".dado/skills/example");
        std::fs::create_dir_all(folder.join("agents")).unwrap();
        let path = folder.join("SKILL.md");
        std::fs::write(
            &path,
            "---\nname: example\ndescription: Configuration checks\n---\nInstructions",
        )
        .unwrap();
        let original = Catalog::load(root.path(), &json!({})).unwrap();
        let config = folder.join("agents/openai.yaml");
        let valid = "policy:\n  allow_implicit_invocation: false\n  products: [CODEX]\ndependencies:\n  tools: []\n";
        std::fs::write(&config, valid).unwrap();
        let catalog = Catalog::load(root.path(), &json!({})).unwrap();
        assert!(catalog.skills[0].enabled);
        // Introducing even an empty configuration invalidates an old chip.
        assert!(
            catalog
                .freeze(
                    root.path(),
                    &json!([original.skills[0]]),
                    &mut Manifest::default()
                )
                .err()
                .expect("configuration change must reject selection")
                .contains("configuration changed")
        );
        let mut manifest = Manifest::default();
        let frozen = catalog
            .freeze(root.path(), &json!([catalog.skills[0]]), &mut manifest)
            .unwrap();
        assert_eq!(frozen.len(), 1);
        assert!(manifest.resources.iter().any(|r| r.revision.as_deref()
            == Some(revision(valid.as_bytes()).as_str())
            && r.exposure == Exposure::Validated));
        for invalid in [
            "dependencies:\n  tools:\n    - type: mcp\n      value: docs\n",
            "dependencies:\n  tools: not-a-list\n",
            "dependencies:\n  tools: [{type: mcp}]\n",
            "dependencies:\n  packages: [pretend]\n",
            "policy:\n  allow_implicit_invocation: sometimes\n",
            "policy:\n  products: [CHAT]\n",
            "[",
            "- wrong-root-shape",
        ] {
            std::fs::write(&config, invalid).unwrap();
            let changed = Catalog::load(root.path(), &json!({})).unwrap();
            assert!(!changed.skills[0].enabled, "{invalid}");
            assert!(changed.skills[0].reason.is_some());
            assert!(
                changed
                    .freeze(
                        root.path(),
                        &json!([changed.skills[0]]),
                        &mut Manifest::default()
                    )
                    .is_err()
            );
            assert!(
                catalog
                    .freeze(
                        root.path(),
                        &json!([catalog.skills[0]]),
                        &mut Manifest::default()
                    )
                    .is_err()
            );
        }
        std::fs::write(&config, format!("{valid}# updated\n")).unwrap();
        assert!(
            catalog
                .freeze(
                    root.path(),
                    &json!([catalog.skills[0]]),
                    &mut Manifest::default()
                )
                .is_err()
        );
        std::fs::remove_file(&config).unwrap();
        assert!(
            catalog
                .freeze(
                    root.path(),
                    &json!([catalog.skills[0]]),
                    &mut Manifest::default()
                )
                .is_err()
        );
        std::fs::write(&config, vec![b'a'; 32 * 1024 + 1]).unwrap();
        assert!(!Catalog::load(root.path(), &json!({})).unwrap().skills[0].enabled);
    }
    #[test]
    fn native_dependency_declarations_cannot_bypass_validation() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join(".agents/skills/example/SKILL.md");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            "---\nname: example\ndescription: Native configuration\n---\nInstructions",
        )
        .unwrap();
        for dependencies in [
            json!({"tools":[{"type":"mcp","value":"docs"}]}),
            json!({"tools":true}),
            json!({"unknown":[]}),
        ] {
            let native = json!({"data":[{"cwd":root.path(),"skills":[{"path":path,"scope":"repo","enabled":true,"dependencies":dependencies}]}]});
            let catalog = Catalog::load(root.path(), &native).unwrap();
            assert!(!catalog.skills[0].enabled);
            assert!(catalog.skills[0].reason.is_some());
        }
    }
    #[test]
    fn resource_reads_use_only_selected_captured_bytes_and_record_their_hashes() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join(".dado/skills/example");
        std::fs::create_dir_all(directory.join("references")).unwrap();
        std::fs::write(directory.join("SKILL.md"),"---\nname: example\ndescription: Read supporting rules\n---\nRead references/rules.txt").unwrap();
        std::fs::write(directory.join("references/rules.txt"), "FROZEN_RULES").unwrap();
        std::fs::write(directory.join(".env"), "SECRET").unwrap();
        std::fs::write(directory.join("references/binary.bin"), b"data\0").unwrap();
        let catalog = Catalog::load(root.path(), &json!({})).unwrap();
        let mut manifest = Manifest::default();
        let frozen = catalog
            .freeze(root.path(), &json!([catalog.skills[0]]), &mut manifest)
            .unwrap();
        let skill = &frozen[0];
        skill.record_resources(&mut manifest).unwrap();
        std::fs::write(directory.join("references/rules.txt"), "CHANGED_ORIGIN").unwrap();
        let snapshot = Path::new(skill.input["path"].as_str().unwrap())
            .parent()
            .unwrap()
            .join("references/rules.txt");
        std::fs::write(snapshot, "CHANGED_SNAPSHOT_ON_DISK").unwrap();
        let listed = skill
            .call(
                "sde_list_skill_resources",
                &json!({"skillId":skill.id}),
                &mut manifest,
            )
            .unwrap();
        assert!(!listed.to_string().contains(".env"));
        let result = skill
            .call(
                "sde_read_skill_resource",
                &json!({"skillId":skill.id,"path":"references/rules.txt"}),
                &mut manifest,
            )
            .unwrap();
        assert_eq!(result["text"], "FROZEN_RULES");
        assert_eq!(result["revision"], revision(b"FROZEN_RULES"));
        assert!(manifest.resources.iter().any(|r| r.source == Source::Skill
            && r.exposure == super::super::manifest::Exposure::Observed
            && r.revision.as_deref() == Some(revision(b"FROZEN_RULES").as_str())));
        for path in [
            "../SKILL.md",
            "references\\rules.txt",
            ".env",
            "references/binary.bin",
            "C:/outside.txt",
        ] {
            assert!(
                skill
                    .call(
                        "sde_read_skill_resource",
                        &json!({"skillId":skill.id,"path":path}),
                        &mut manifest
                    )
                    .is_err()
            );
        }
        assert!(
            skill
                .call(
                    "sde_read_skill_resource",
                    &json!({"skillId":"unselected","path":"references/rules.txt"}),
                    &mut manifest
                )
                .is_err()
        );
        assert!(
            skill
                .call(
                    "sde_run_command",
                    &json!({"skillId":skill.id}),
                    &mut manifest
                )
                .is_err()
        );
    }
    #[test]
    fn repository_export_preserves_private_skill_resources_and_existing_shared_skills() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join(".dado/skills/example");
        std::fs::create_dir_all(directory.join("references")).unwrap();
        let text =
            "---\nname: example\ndescription: Export fixture\n---\nRead references/rules.txt";
        std::fs::write(directory.join("SKILL.md"), text).unwrap();
        std::fs::write(directory.join("references/rules.txt"), "RULES").unwrap();
        let catalog = Catalog::load(root.path(), &json!({})).unwrap();
        let skill = &catalog.skills[0];
        assert!(export_private(root.path(), &skill.id, "stale").is_err());
        assert!(!root.path().join(".agents").exists());
        export_private(root.path(), &skill.id, &skill.revision).unwrap();
        assert_eq!(
            std::fs::read_to_string(directory.join("SKILL.md")).unwrap(),
            text
        );
        assert_eq!(
            std::fs::read_to_string(
                root.path()
                    .join(".agents/skills/example/references/rules.txt")
            )
            .unwrap(),
            "RULES"
        );
        std::fs::write(directory.join("references/rules.txt"), "NEW_RULES").unwrap();
        assert!(export_private(root.path(), &skill.id, &skill.revision).is_err());
        assert_eq!(
            std::fs::read_to_string(
                root.path()
                    .join(".agents/skills/example/references/rules.txt")
            )
            .unwrap(),
            "RULES"
        );
        assert!(export_private(root.path(), "forged", &skill.revision).is_err());
    }
    #[test]
    fn draft_review_publication_and_reopening_preserve_exact_editor_text() {
        let root = tempfile::tempdir().unwrap();
        let text = "---\nname: review-code\ndescription: Review code when requested.\n---\nLook for correctness issues.";
        let draft = create_draft(
            root.path(),
            "review-code",
            "project-private",
            &format!("```markdown\n{text}\n```"),
        )
        .unwrap();
        assert_eq!(drafts(root.path()).unwrap().as_array().unwrap().len(), 1);
        assert!(
            Catalog::load(root.path(), &json!({}))
                .unwrap()
                .skills
                .is_empty()
        );
        let edited = text.replace("correctness issues", "correctness and regression issues");
        let published = publish_draft(
            root.path(),
            draft["id"].as_str().unwrap(),
            draft["revision"].as_str().unwrap(),
            &edited,
        )
        .unwrap();
        assert_eq!(
            std::fs::read_to_string(published["path"].as_str().unwrap()).unwrap(),
            edited
        );
        assert_eq!(
            std::fs::read_to_string(draft["path"].as_str().unwrap()).unwrap(),
            text
        );
        assert!(drafts(root.path()).unwrap().as_array().unwrap().is_empty());
        let reopened = Catalog::load(root.path(), &json!({})).unwrap();
        assert_eq!(reopened.skills.len(), 1);
        let frozen = reopened
            .freeze(
                root.path(),
                &json!([reopened.skills[0]]),
                &mut Manifest::default(),
            )
            .unwrap();
        assert_eq!(frozen[0].instructions, edited);
    }
    #[test]
    fn publication_rejects_stale_drafts_collisions_and_invalid_references() {
        let root = tempfile::tempdir().unwrap();
        let text = "---\nname: same-name\ndescription: Test\n---\nInstructions";
        let draft = create_draft(root.path(), "same-name", "repo", text).unwrap();
        let id = draft["id"].as_str().unwrap();
        let before = draft["revision"].as_str().unwrap();
        let path = Path::new(draft["path"].as_str().unwrap());
        std::fs::write(path, format!("{text}\nEdited externally")).unwrap();
        assert!(
            publish_draft(root.path(), id, before, text)
                .unwrap_err()
                .contains("changed on disk")
        );
        assert!(!root.path().join(".agents").exists());
        std::fs::write(path, text).unwrap();
        let published = publish_draft(root.path(), id, before, text).unwrap();
        assert!(publish_draft(root.path(), id, before, &format!("{text}\nOther")).is_err());
        assert_eq!(
            std::fs::read_to_string(published["path"].as_str().unwrap()).unwrap(),
            text
        );
        assert!(publish_draft(root.path(), "../escape", before, text).is_err());
        for invalid in [
            text.replace("---\nInstructions", "---oops\nInstructions"),
            format!("{text}\nRead [rules](rules.md)"),
            format!("{text}\nRun scripts\\task.ps1"),
        ] {
            assert!(create_draft(root.path(), "same-name", "project-private", &invalid).is_err());
        }
        assert!(create_draft(root.path(), "wrong-name", "repo", text).is_err());
        assert!(create_draft(root.path(), "same-name", "global", text).is_err());
    }
    #[test]
    fn private_discovery_freezes_instructions_and_rejects_stale_references() {
        let root = tempfile::tempdir().unwrap();
        let skill = root.path().join(".dado/skills/example/SKILL.md");
        std::fs::create_dir_all(skill.parent().unwrap()).unwrap();
        let text = "---\nname: example\ndescription: |\n  A multiline description\n---\nUse EXAMPLE_MARKER.";
        std::fs::write(&skill, text).unwrap();
        let catalog = Catalog::load(root.path(), &json!({"data":[]})).unwrap();
        assert_eq!(catalog.skills.len(), 1);
        let selected = json!([catalog.skills[0]]);
        let mut manifest = Manifest::default();
        let items = catalog
            .freeze(root.path(), &selected, &mut manifest)
            .unwrap();
        assert_eq!(manifest.resources[0].source, Source::Skill);
        std::fs::write(&skill, text.replace("EXAMPLE_MARKER", "CHANGED")).unwrap();
        assert_eq!(
            std::fs::read_to_string(items[0].input["path"].as_str().unwrap()).unwrap(),
            text
        );
        assert!(
            catalog
                .freeze(root.path(), &selected, &mut manifest)
                .is_err()
        );
        let new = Catalog::load(root.path(), &json!({})).unwrap();
        assert!(new.freeze(root.path(), &selected, &mut manifest).is_err());
        assert!(
            new.freeze(
                root.path(),
                &json!([{"id":"forged","revision":"x"}]),
                &mut manifest
            )
            .is_err()
        );
    }
    #[test]
    fn catalog_distinguishes_collisions_and_rejects_invalid_or_disabled_skills() {
        let root = tempfile::tempdir().unwrap();
        let a = root.path().join(".agents/skills/a/SKILL.md");
        let b = root.path().join(".dado/skills/b/SKILL.md");
        let invalid = root.path().join(".dado/skills/bad/SKILL.md");
        for path in [&a, &b, &invalid] {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        }
        for path in [&a, &b] {
            std::fs::write(
                path,
                "---\nname: same-name\ndescription: Shared name\n---\nInstructions",
            )
            .unwrap();
        }
        std::fs::write(
            &invalid,
            "---\nname: Invalid Name\ndescription: Bad\n---\nBad",
        )
        .unwrap();
        let native = json!({"data":[{"cwd":root.path(),"skills":[{"path":a,"name":"same-name","scope":"repo","enabled":false}],"errors":[]}]});
        let catalog = Catalog::load(root.path(), &native).unwrap();
        assert_eq!(catalog.skills.len(), 2);
        assert_eq!(catalog.errors.len(), 1);
        assert_ne!(catalog.skills[0].id, catalog.skills[1].id);
        let disabled = catalog.skills.iter().find(|s| !s.enabled).unwrap();
        assert!(
            catalog
                .freeze(root.path(), &json!([disabled]), &mut Manifest::default())
                .is_err()
        );
        let enabled = catalog.skills.iter().find(|s| s.enabled).unwrap();
        assert!(
            catalog
                .freeze(
                    root.path(),
                    &json!([enabled, enabled]),
                    &mut Manifest::default()
                )
                .is_err()
        );
    }
    #[test]
    fn captured_resources_remain_stable_and_oversized_resources_are_rejected() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join(".dado/skills/example/SKILL.md");
        std::fs::create_dir_all(path.parent().unwrap().join("references")).unwrap();
        std::fs::write(
            &path,
            "---\nname: example\ndescription: References\n---\nRead references/rules.txt",
        )
        .unwrap();
        let reference = path.parent().unwrap().join("references/rules.txt");
        std::fs::write(&reference, "original rules").unwrap();
        let catalog = Catalog::load(root.path(), &json!({})).unwrap();
        let chosen = json!([catalog.skills[0]]);
        let capture = catalog
            .freeze(root.path(), &chosen, &mut Manifest::default())
            .unwrap();
        std::fs::write(&reference, "changed rules").unwrap();
        let frozen = Path::new(capture[0].input["path"].as_str().unwrap())
            .parent()
            .unwrap()
            .join("references/rules.txt");
        assert_eq!(std::fs::read_to_string(frozen).unwrap(), "original rules");
        std::fs::write(&reference, vec![0u8; 512 * 1024 + 1]).unwrap();
        assert!(
            catalog
                .freeze(root.path(), &chosen, &mut Manifest::default())
                .is_err()
        );
    }
}
