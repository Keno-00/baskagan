//! Read-only, bounded project tools. Paths are resolved by the host, never the model.
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, VecDeque},
    io::Read,
    path::{Component, Path, PathBuf},
};

const FILE_LIMIT: usize = 128 * 1024;
pub struct Context {
    root: PathBuf,
    buffers: HashMap<PathBuf, String>,
}
pub(crate) fn excluded(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    matches!(
        name.as_str(),
        ".git" | ".dado" | "node_modules" | "target" | "auth.json" | "credentials.json"
    ) || name == ".env"
        || name.starts_with(".env.")
}
fn link(path: &Path) -> bool {
    let Ok(metadata) = std::fs::symlink_metadata(path) else {
        return false;
    };
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}
impl Context {
    pub fn record_buffers(&self, manifest: &mut super::manifest::Manifest) -> Result<(), String> {
        let mut buffers: Vec<_> = self.buffers.iter().collect();
        buffers.sort_by_key(|(path, _)| *path);
        for (path, text) in buffers {
            manifest.file(
                Some(&self.root),
                path,
                (
                    super::manifest::Source::Editor,
                    super::manifest::Exposure::Available,
                ),
                Some(format!("{:x}", Sha256::digest(text.as_bytes()))),
                None,
                "Captured editor buffer available to project tools",
            )?;
        }
        Ok(())
    }
    pub fn new(root: &Path, buffers: &Value) -> Result<Self, String> {
        let root = root
            .canonicalize()
            .map_err(|e| format!("Project is unavailable: {e}"))?;
        if !root.is_dir() {
            return Err("Project is not a directory.".into());
        }
        let mut context = Self {
            root,
            buffers: HashMap::new(),
        };
        let mut total = 0;
        for buffer in buffers.as_array().into_iter().flatten() {
            let path = Path::new(buffer["path"].as_str().ok_or("Buffer has no path.")?);
            let canonical = path.canonicalize().map_err(|e| e.to_string())?;
            // Editors from other projects are not context for this run.
            let Ok(relative) = canonical.strip_prefix(&context.root) else {
                continue;
            };
            if relative
                .components()
                .any(|c| matches!(c,Component::Normal(n) if n.to_str().is_some_and(excluded)))
            {
                continue;
            }
            let resolved = context.resolve(relative.to_str().ok_or("Invalid buffer path.")?)?;
            let text = buffer["text"].as_str().ok_or("Buffer has no text.")?;
            total += text.len();
            if text.len() > FILE_LIMIT || total > 256 * 1024 || context.buffers.len() >= 24 {
                return Err("Open editor context exceeds its size limit. Close large buffers before sending.".into());
            }
            context.buffers.insert(resolved, text.into());
        }
        Ok(context)
    }
    pub(crate) fn resolve(&self, relative: &str) -> Result<PathBuf, String> {
        let candidate = self.target(relative)?;
        let path = candidate.canonicalize().map_err(|e| e.to_string())?;
        if !path.starts_with(&self.root) {
            return Err("Path leaves the project.".into());
        }
        Ok(path)
    }
    /// Validate each existing ancestor while allowing an absent final path.
    /// Absence is meaningful in a creation/deletion review; a failed read is not.
    pub(crate) fn target(&self, relative: &str) -> Result<PathBuf, String> {
        let mut candidate = self.root.clone();
        for component in Path::new(relative).components() {
            match component {
                Component::CurDir => {}
                Component::Normal(name) => {
                    let name = name.to_str().ok_or("Invalid path.")?;
                    if excluded(name) || name.contains(':') {
                        return Err("Private or generated path is excluded.".into());
                    }
                    #[cfg(windows)]
                    {
                        let device = name
                            .split('.')
                            .next()
                            .unwrap_or_default()
                            .to_ascii_uppercase();
                        if name.ends_with(['.', ' '])
                            || name.contains(['<', '>', '"', '|', '?', '*'])
                            || name.chars().any(|c| c.is_control())
                            || matches!(device.as_str(), "CON" | "PRN" | "AUX" | "NUL")
                            || (device.len() == 4
                                && (device.starts_with("COM") || device.starts_with("LPT"))
                                && matches!(device.as_bytes()[3], b'1'..=b'9'))
                        {
                            return Err("Unsupported Windows path component.".into());
                        }
                    }
                    candidate.push(name);
                    if link(&candidate) {
                        return Err("Project tools cannot follow links or junctions.".into());
                    }
                    match std::fs::symlink_metadata(&candidate) {
                        Ok(_) => {
                            let resolved = candidate.canonicalize().map_err(|e| e.to_string())?;
                            if !resolved.starts_with(&self.root) {
                                return Err("Path leaves the project.".into());
                            }
                            candidate = resolved;
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                        Err(e) => return Err(e.to_string()),
                    }
                }
                _ => return Err("Use a path relative to the project without traversal.".into()),
            }
        }
        Ok(candidate)
    }
    fn text(&self, path: &Path) -> Result<(String, &'static str), String> {
        if let Some(text) = self.buffers.get(path) {
            return Ok((text.clone(), "editor"));
        }
        if !std::fs::metadata(path)
            .map_err(|e| e.to_string())?
            .is_file()
        {
            return Err("Choose a file.".into());
        }
        let mut bytes = Vec::new();
        std::fs::File::open(path)
            .and_then(|f| f.take((FILE_LIMIT + 1) as u64).read_to_end(&mut bytes))
            .map_err(|e| e.to_string())?;
        if bytes.len() > FILE_LIMIT || bytes.contains(&0) {
            return Err("File is binary or exceeds 128 KB.".into());
        }
        Ok((
            String::from_utf8(bytes).map_err(|_| "Only UTF-8 text is supported.")?,
            "disk",
        ))
    }
    pub fn instructions(&self, path: &Path) -> Result<Value, String> {
        let directory = if path.is_dir() {
            path
        } else {
            path.parent().ok_or("Instruction scope has no parent.")?
        };
        let mut directories = Vec::new();
        let mut current = directory;
        loop {
            if !current.starts_with(&self.root) {
                return Err("Instruction scope leaves the project.".into());
            }
            directories.push(current.to_path_buf());
            if directories.len() > 64 {
                return Err("Instruction scope exceeds 64 levels.".into());
            }
            if current == self.root {
                break;
            }
            current = current.parent().ok_or("Invalid instruction scope.")?;
        }
        directories.reverse();
        let mut instructions = Vec::new();
        let mut bytes = 0;
        for directory in directories {
            let candidate = directory.join("AGENTS.md");
            if !candidate.try_exists().map_err(|e| e.to_string())? {
                continue;
            }
            let relative = candidate
                .strip_prefix(&self.root)
                .map_err(|_| "Instruction leaves project.")?;
            let path = self.resolve(relative.to_str().ok_or("Invalid instruction path.")?)?;
            let (text, source) = self.text(&path)?;
            bytes += text.len();
            if bytes > 64 * 1024 {
                return Err("Applicable project instructions exceed 64 KB.".into());
            }
            let scope = directory.strip_prefix(&self.root).unwrap();
            let scope = if scope.as_os_str().is_empty() {
                ".".into()
            } else {
                scope.to_string_lossy().replace('\\', "/")
            };
            instructions.push(json!({"path":relative,"scope":scope,"source":source,"revision":format!("{:x}",Sha256::digest(text.as_bytes())),"text":text}));
        }
        Ok(json!(instructions))
    }
    fn search(&self, path: &Path, args: &Value) -> Result<Value, String> {
        let query = args["query"]
            .as_str()
            .ok_or("Search needs a literal query.")?;
        if query.is_empty() || query.len() > 1024 {
            return Err("Search query must contain 1–1024 bytes.".into());
        }
        let limit = args["maxResults"].as_u64().unwrap_or(50).clamp(1, 100) as usize;
        let mut pending = VecDeque::from([path.to_path_buf()]);
        let mut matches = Vec::new();
        let mut scanned = 0;
        let mut visited = 0;
        let mut bytes = 0;
        let mut skipped = 0;
        let mut truncated = false;
        'walk: while let Some(candidate) = pending.pop_front() {
            visited += 1;
            if visited > 2000 {
                truncated = true;
                break;
            }
            let relative = candidate
                .strip_prefix(&self.root)
                .map_err(|_| "Path leaves the project.")?;
            // Resolve again at use time: queued paths are never trusted permissions.
            let Ok(candidate) = self.resolve(relative.to_str().ok_or("Invalid path.")?) else {
                skipped += 1;
                continue;
            };
            if candidate.is_dir() {
                let mut children = Vec::new();
                for entry in std::fs::read_dir(&candidate)
                    .map_err(|e| e.to_string())?
                    .take(2001)
                {
                    let entry = entry.map_err(|e| e.to_string())?;
                    if children.len() >= 2000 {
                        truncated = true;
                        break;
                    }
                    // Count excluded entries too, so directories of generated files are bounded.
                    children.push(entry.path());
                }
                children.sort();
                for child in children {
                    let name = child
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or_default();
                    if excluded(name) || link(&child) {
                        continue;
                    }
                    if pending.len() >= 2000 {
                        truncated = true;
                        break;
                    }
                    pending.push_back(child);
                }
                continue;
            }
            if scanned >= 500 {
                truncated = true;
                break;
            }
            scanned += 1;
            let Ok((text, source)) = self.text(&candidate) else {
                skipped += 1;
                continue;
            };
            bytes += text.len();
            if bytes > 4 * 1024 * 1024 {
                truncated = true;
                break;
            }
            let revision = format!("{:x}", Sha256::digest(text.as_bytes()));
            for (index, line) in text.lines().enumerate() {
                if !line.contains(query) {
                    continue;
                }
                if matches.len() == limit {
                    truncated = true;
                    break 'walk;
                }
                matches.push(json!({"path":candidate.strip_prefix(&self.root).unwrap(),
                    "line":index + 1,"text":line.chars().take(500).collect::<String>(),
                    "source":source,"revision":revision}));
            }
        }
        Ok(
            json!({"matches":matches,"truncated":truncated,"scannedFiles":scanned,"skippedFiles":skipped}),
        )
    }
    pub fn call(&self, tool: &str, args: &Value) -> Result<Value, String> {
        let path = self.resolve(args["path"].as_str().unwrap_or("."))?;
        match tool {
            "sde_search_text" => self.search(&path, args),
            "sde_read_file" => {
                let (text, source) = self.text(&path)?;
                let start = args["startLine"].as_u64().unwrap_or(1).max(1) as usize;
                let limit = args["lineCount"].as_u64().unwrap_or(200).clamp(1, 400) as usize;
                let lines: Vec<_> = text.lines().collect();
                let selected: Vec<_> = lines
                    .iter()
                    .skip(start - 1)
                    .take(limit)
                    .enumerate()
                    .map(|(i, line)| format!("{}: {line}", start + i))
                    .collect();
                Ok(
                    json!({"path":path.strip_prefix(&self.root).unwrap(),"source":source,"revision":format!("{:x}",Sha256::digest(text.as_bytes())),"startLine":start,"totalLines":lines.len(),"text":selected.join("\n"),"instructions":self.instructions(&path)?}),
                )
            }
            "sde_list_directory" => {
                let mut entries = Vec::new();
                let mut scanned = 0;
                for entry in std::fs::read_dir(&path)
                    .map_err(|e| e.to_string())?
                    .take(1001)
                {
                    scanned += 1;
                    if scanned > 1000 {
                        break;
                    }
                    let entry = entry.map_err(|e| e.to_string())?;
                    let name = entry.file_name().to_string_lossy().into_owned();
                    if excluded(&name) || link(&entry.path()) {
                        continue;
                    }
                    entries.push(json!({"name":name,"kind":if entry.file_type().map_err(|e|e.to_string())?.is_dir(){"folder"}else{"file"}}));
                }
                entries.sort_by_key(|e| e["name"].as_str().unwrap_or_default().to_string());
                let truncated = scanned > 1000 || entries.len() > 100;
                entries.truncate(100);
                Ok(json!({"entries":entries,"truncated":truncated}))
            }
            _ => Err("Unknown project tool.".into()),
        }
    }
}
pub fn tools() -> Value {
    json!([
        {"type":"function","name":"sde_list_directory","description":"List an immediate project directory. Private/generated files and links are excluded. Folder references do not preload file contents.","inputSchema":{"type":"object","properties":{"path":{"type":"string"}},"required":["path"],"additionalProperties":false}},
        {"type":"function","name":"sde_read_file","description":"Read bounded project text by line. Uses the editor snapshot captured for this turn when available. Returns source and content revision.","inputSchema":{"type":"object","properties":{"path":{"type":"string"},"startLine":{"type":"integer","minimum":1},"lineCount":{"type":"integer","minimum":1,"maximum":400}},"required":["path"],"additionalProperties":false}},
        {"type":"function","name":"sde_search_text","description":"Search literal, case-sensitive text inside a project file or directory. Uses captured editor buffers. Excludes private/generated paths and links. Bounded to 500 files, 4 MB and 100 results; truncated means results are incomplete.","inputSchema":{"type":"object","properties":{"path":{"type":"string"},"query":{"type":"string","minLength":1,"maxLength":1024},"maxResults":{"type":"integer","minimum":1,"maximum":100}},"required":["path","query"],"additionalProperties":false}},
        {"type":"function","name":"sde_propose_file","description":"Propose a complete replacement of an existing UTF-8 file in an isolated copy, never the live project. Read first and pass its exact content revision. Preserves BOM/line endings. Returns a pending review, not an applied or tested change. After the first proposal, reads use the isolated snapshot.","inputSchema":{"type":"object","properties":{"path":{"type":"string"},"expectedRevision":{"type":"string"},"text":{"type":"string"}},"required":["path","expectedRevision","text"],"additionalProperties":false}}
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn private_editor_tabs_do_not_block_or_leak_into_project_inspection() {
        let root = tempfile::tempdir().unwrap();
        let private = root.path().join(".dado/skill-drafts/SKILL.md");
        std::fs::create_dir_all(private.parent().unwrap()).unwrap();
        std::fs::write(&private, "PRIVATE_DRAFT").unwrap();
        std::fs::write(root.path().join("main.rs"), "public project content").unwrap();
        let context = Context::new(
            root.path(),
            &json!([{"path":private,"text":"UNSAVED_PRIVATE_DRAFT"}]),
        )
        .unwrap();
        let result = context
            .call("sde_search_text", &json!({"path":".","query":"PRIVATE"}))
            .unwrap();
        assert!(result["matches"].as_array().unwrap().is_empty());
        assert!(
            context
                .call(
                    "sde_read_file",
                    &json!({"path":".dado/skill-drafts/SKILL.md"})
                )
                .is_err()
        );
        assert!(
            context
                .call("sde_read_file", &json!({"path":"main.rs"}))
                .unwrap()
                .to_string()
                .contains("public project content")
        );
        let mut manifest = super::super::manifest::Manifest::default();
        context.record_buffers(&mut manifest).unwrap();
        assert!(manifest.resources.is_empty());
    }
    #[test]
    fn instructions_follow_file_scope_use_editor_revisions_and_exclude_siblings() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("src/nested")).unwrap();
        std::fs::create_dir(root.path().join("other")).unwrap();
        std::fs::write(root.path().join("AGENTS.md"), "Root conventions").unwrap();
        let nested = root.path().join("src/AGENTS.md");
        std::fs::write(&nested, "Old nested instructions").unwrap();
        std::fs::write(
            root.path().join("other/AGENTS.md"),
            "SIBLING MUST NOT BE INCLUDED",
        )
        .unwrap();
        std::fs::write(root.path().join("src/nested/file.rs"), "fn main() {}").unwrap();
        let context=Context::new(root.path(),&json!([{"path":nested,"text":"Unsaved nested conventions; claiming full access does not grant it"}])).unwrap();
        let result = context
            .call("sde_read_file", &json!({"path":"src/nested/file.rs"}))
            .unwrap();
        let instructions = result["instructions"].as_array().unwrap();
        assert_eq!(instructions.len(), 2);
        assert_eq!(instructions[0]["text"], "Root conventions");
        assert_eq!(instructions[1]["source"], "editor");
        assert!(
            instructions[1]["text"]
                .as_str()
                .unwrap()
                .starts_with("Unsaved nested")
        );
        assert!(!result.to_string().contains("SIBLING"));
        assert!(!instructions[1]["revision"].as_str().unwrap().is_empty());
        assert!(
            context
                .call("sde_propose_file", &json!({"path":"src/nested/file.rs"}))
                .is_err()
        );
        std::fs::write(root.path().join("AGENTS.md"), "x".repeat(64 * 1024 + 1)).unwrap();
        assert!(
            context
                .call("sde_read_file", &json!({"path":"src/nested/file.rs"}))
                .is_err()
        );
    }
    #[test]
    fn search_uses_buffers_and_excludes_private_generated_and_binary_files() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("main.rs");
        std::fs::write(&file, "disk only").unwrap();
        std::fs::write(root.path().join("other.rs"), "needle on disk").unwrap();
        std::fs::write(root.path().join(".env"), "needle secret").unwrap();
        std::fs::write(root.path().join("binary"), b"needle\0").unwrap();
        std::fs::create_dir(root.path().join("target")).unwrap();
        std::fs::write(root.path().join("target/output"), "needle generated").unwrap();
        let context = Context::new(
            root.path(),
            &json!([{"path":file,"text":"first\nneedle unsaved"}]),
        )
        .unwrap();
        let result = context
            .call("sde_search_text", &json!({"path":".","query":"needle"}))
            .unwrap();
        let matches = result["matches"].as_array().unwrap();
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0]["source"], "editor");
        assert_eq!(matches[0]["line"], 2);
        assert_eq!(matches[1]["source"], "disk");
        assert_eq!(result["truncated"], false);
        assert!(!result.to_string().contains("secret"));
        assert!(!result.to_string().contains("generated"));
        assert_eq!(std::fs::read_to_string(file).unwrap(), "disk only");
        for query in ["", &"x".repeat(1025)] {
            assert!(
                context
                    .call("sde_search_text", &json!({"query":query}))
                    .is_err()
            );
        }
        assert!(
            context
                .call("sde_search_text", &json!({"path":"..","query":"needle"}))
                .is_err()
        );
    }
    #[test]
    fn search_reports_incomplete_results_instead_of_silent_success() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("many.txt"), "needle\nneedle\nneedle").unwrap();
        let context = Context::new(root.path(), &json!([])).unwrap();
        let result = context
            .call(
                "sde_search_text",
                &json!({"path":"many.txt","query":"needle","maxResults":2}),
            )
            .unwrap();
        assert_eq!(result["matches"].as_array().unwrap().len(), 2);
        assert_eq!(result["truncated"], true);
        let result = context
            .call(
                "sde_search_text",
                &json!({"path":"many.txt","query":"absent"}),
            )
            .unwrap();
        assert!(result["matches"].as_array().unwrap().is_empty());
        assert_eq!(result["truncated"], false);
    }
    #[test]
    fn reads_snapshot_and_rejects_escape_and_private_paths() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("main.rs");
        std::fs::write(&file, "disk").unwrap();
        std::fs::write(root.path().join(".env"), "SECRET").unwrap();
        let context = Context::new(
            root.path(),
            &json!([{"path":file,"text":"unsaved\nsecond"}]),
        )
        .unwrap();
        let result = context
            .call("sde_read_file", &json!({"path":"main.rs"}))
            .unwrap();
        assert_eq!(result["source"], "editor");
        assert!(result["text"].as_str().unwrap().contains("unsaved"));
        assert_eq!(result["revision"].as_str().unwrap().len(), 64);
        for path in [
            "../main.rs",
            ".env",
            ".dado/state.json",
            "C:/Windows",
            "/etc/passwd",
        ] {
            assert!(
                context
                    .call("sde_read_file", &json!({"path":path}))
                    .is_err()
            );
        }
        assert!(context.call("write", &json!({"path":"main.rs"})).is_err());
        assert!(
            !context
                .call("sde_list_directory", &json!({"path":"."}))
                .unwrap()
                .to_string()
                .contains(".env")
        );
    }
}
