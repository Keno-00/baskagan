use serde_json::{Value, json};
use std::{io::Write, path::Path};

pub(crate) fn linked(path: &Path) -> bool {
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

pub(crate) fn storage(root: &Path) -> Result<std::path::PathBuf, String> {
    if !root.is_dir() {
        return Err("Project folder is unavailable.".into());
    }
    let folder = root.join(".dado");
    if linked(&folder) {
        return Err("Project storage cannot be a symbolic link.".into());
    }
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let ignore = root.join(".gitignore");
    if ignore.is_file() && !linked(&ignore) {
        let text = std::fs::read_to_string(&ignore).map_err(|e| e.to_string())?;
        if !text
            .lines()
            .any(|line| matches!(line.trim(), "/.dado/" | ".dado/" | ".dado" | "/.dado"))
        {
            let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
            let mut file = std::fs::OpenOptions::new()
                .append(true)
                .open(ignore)
                .map_err(|e| e.to_string())?;
            if !text.is_empty() && !text.ends_with('\n') {
                file.write_all(newline.as_bytes())
                    .map_err(|e| e.to_string())?;
            }
            file.write_all(format!("/.dado/{newline}").as_bytes())
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(folder)
}

pub fn load(root: &Path) -> Result<Value, String> {
    let path = storage(root)?.join("state.json");
    if linked(&path) {
        return Err("Project state cannot be a symbolic link.".into());
    }
    if !path.exists() {
        return Ok(json!({"version":1,"settings":{},"conversations":[],"comments":[]}));
    }
    let state: Value = serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
        .map_err(|e| format!("Could not read project state: {e}"))?;
    if !state.is_object()
        || state.get("conversations").is_some_and(|v| !v.is_array())
        || state.get("comments").is_some_and(|v| !v.is_array())
        || state.get("settings").is_some_and(|v| !v.is_object())
    {
        return Err("Project state has an unsupported format.".into());
    }
    Ok(state)
}

pub fn save(root: &Path, state: &Value) -> Result<(), String> {
    if !state.is_object() {
        return Err("Invalid project state.".into());
    }
    let folder = storage(root)?;
    let path = folder.join("state.json");
    if linked(&path) {
        return Err("Project state cannot be a symbolic link.".into());
    }
    let mut temporary = tempfile::NamedTempFile::new_in(folder).map_err(|e| e.to_string())?;
    serde_json::to_writer_pretty(&mut temporary, state).map_err(|e| e.to_string())?;
    temporary.as_file().sync_all().map_err(|e| e.to_string())?;
    temporary.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn project_state_isolated_and_ignore_preserved() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        std::fs::write(a.path().join(".gitignore"), "target/\r\n# Keep me").unwrap();
        let state = json!({"version":1,"conversations":[{"messages":[{"text":"Hello"}],"draft":"Draft"}],"settings":{"layout":2}});
        save(a.path(), &state).unwrap();
        assert_eq!(load(a.path()).unwrap(), state);
        assert_eq!(load(b.path()).unwrap()["conversations"], json!([]));
        assert!(!b.path().join(".gitignore").exists());
        assert_eq!(
            std::fs::read_to_string(a.path().join(".gitignore")).unwrap(),
            "target/\r\n# Keep me\r\n/.dado/\r\n"
        );
        load(a.path()).unwrap();
        assert_eq!(
            std::fs::read_to_string(a.path().join(".gitignore"))
                .unwrap()
                .matches("/.dado/")
                .count(),
            1
        );
    }
    #[test]
    fn malformed_state_is_not_silently_replaced() {
        let root = tempfile::tempdir().unwrap();
        storage(root.path()).unwrap();
        std::fs::write(root.path().join(".dado/state.json"), "broken").unwrap();
        assert!(load(root.path()).is_err());
        assert_eq!(
            std::fs::read_to_string(root.path().join(".dado/state.json")).unwrap(),
            "broken"
        );
    }
}
