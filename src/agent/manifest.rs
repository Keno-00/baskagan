//! Provenance for bytes supplied to a turn; never stores executable grants or contents.
use super::snapshot::revision;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Disk,
    Editor,
    Snapshot,
    User,
    Conversation,
    Harness,
    Skill,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Exposure {
    Supplied,
    Available,
    Observed,
    /// Read by the host to validate prerequisites, not supplied to the model.
    Validated,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RangeUnit {
    Lines,
    Utf16CodeUnits,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Resource {
    pub id: String,
    pub kind: String,
    pub identity: String,
    pub relative_path: Option<String>,
    pub source: Source,
    pub exposure: Exposure,
    pub revision: Option<String>,
    pub range: Option<[usize; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range_unit: Option<RangeUnit>,
    pub reasons: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub version: u32,
    pub resources: Vec<Resource>,
}
impl Default for Manifest {
    fn default() -> Self {
        Self {
            version: 1,
            resources: Vec::new(),
        }
    }
}
impl Manifest {
    pub fn record(&mut self, mut resource: Resource) -> Result<(), String> {
        resource.id = revision(
            format!(
                "{}\0{}\0{}",
                resource.kind,
                resource.identity,
                serde_json::to_string(&resource.source).map_err(|e| e.to_string())?
            )
            .as_bytes(),
        );
        if let Some(existing) = self.resources.iter_mut().find(|item| {
            item.id == resource.id
                && item.revision == resource.revision
                && item.range == resource.range
                && item.range_unit == resource.range_unit
                && item.exposure == resource.exposure
        }) {
            for reason in resource.reasons {
                if !existing.reasons.contains(&reason) {
                    existing.reasons.push(reason);
                }
            }
            return Ok(());
        }
        if self.resources.len() >= 1024 {
            return Err(
                "Context provenance exceeds 1,024 resources. Narrow this run's context.".into(),
            );
        }
        self.resources.push(resource);
        Ok(())
    }
    pub fn content(
        &mut self,
        kind: &str,
        identity: &str,
        content: &str,
        source: Source,
        reason: &str,
    ) -> Result<(), String> {
        self.record(Resource {
            id: String::new(),
            kind: kind.into(),
            identity: identity.into(),
            relative_path: None,
            source,
            exposure: Exposure::Supplied,
            revision: Some(revision(content.as_bytes())),
            range: None,
            range_unit: None,
            reasons: vec![reason.into()],
        })
    }
    pub fn file(
        &mut self,
        root: Option<&Path>,
        path: &Path,
        provenance: (Source, Exposure),
        hash: Option<String>,
        range: Option<[usize; 2]>,
        reason: &str,
    ) -> Result<(), String> {
        let path = path.to_path_buf();
        self.record(Resource {
            id: String::new(),
            kind: "file".into(),
            identity: path.to_string_lossy().into_owned(),
            relative_path: root
                .and_then(|root| path.strip_prefix(root).ok())
                .map(|p| p.to_string_lossy().replace('\\', "/")),
            source: provenance.0,
            exposure: provenance.1,
            revision: hash,
            range,
            range_unit: range.map(|_| RangeUnit::Lines),
            reasons: vec![reason.into()],
        })
    }
    pub fn directory(
        &mut self,
        root: Option<&Path>,
        path: &Path,
        listing: &str,
        reason: &str,
    ) -> Result<(), String> {
        self.record(Resource {
            id: String::new(),
            kind: "directory".into(),
            identity: path.to_string_lossy().into_owned(),
            relative_path: root
                .and_then(|root| path.strip_prefix(root).ok())
                .map(|p| p.to_string_lossy().replace('\\', "/")),
            source: Source::Disk,
            exposure: Exposure::Supplied,
            revision: Some(revision(listing.as_bytes())),
            range: None,
            range_unit: None,
            reasons: vec![reason.into()],
        })
    }
    pub fn tool(
        &mut self,
        root: &Path,
        tool: &str,
        args: &Value,
        result: &Value,
    ) -> Result<(), String> {
        self.record(Resource {
            id: String::new(),
            kind: "tool_result".into(),
            identity: format!("{tool}:{}", revision(args.to_string().as_bytes())),
            relative_path: args["path"].as_str().map(|p| p.replace('\\', "/")),
            source: Source::Harness,
            exposure: Exposure::Observed,
            revision: Some(revision(result.to_string().as_bytes())),
            range: None,
            range_unit: None,
            reasons: vec![format!("Result supplied by {tool}")],
        })?;
        let source = |value: &Value| match value["source"].as_str() {
            Some("editor") => Source::Editor,
            Some("snapshot") => Source::Snapshot,
            _ => Source::Disk,
        };
        let mut record = |item: &Value, range: Option<[usize; 2]>| -> Result<(), String> {
            if let (Some(path), Some(hash)) = (item["path"].as_str(), item["revision"].as_str()) {
                self.file(
                    Some(root),
                    &root.join(path),
                    (source(item), Exposure::Observed),
                    Some(hash.into()),
                    range,
                    tool,
                )?;
            }
            Ok(())
        };
        if tool == "sde_read_file" {
            let start = result["startLine"].as_u64().unwrap_or(1) as usize;
            let count = result["text"].as_str().unwrap_or_default().lines().count();
            record(result, (count > 0).then(|| [start, start + count - 1]))?;
        } else if tool == "sde_search_text" {
            for item in result["matches"].as_array().into_iter().flatten() {
                let line = item["line"].as_u64().unwrap_or(1) as usize;
                record(item, Some([line, line]))?;
            }
        } else if tool == "sde_propose_file" {
            let mut proposal = result.clone();
            proposal["source"] = Value::String("snapshot".into());
            record(&proposal, None)?;
        }
        for instruction in result["instructions"].as_array().into_iter().flatten() {
            self.instruction(root, instruction)?;
        }
        Ok(())
    }
    pub fn instruction(&mut self, root: &Path, item: &Value) -> Result<(), String> {
        let path = root.join(item["path"].as_str().ok_or("Instruction has no path.")?);
        let resource = Resource {
            id: String::new(),
            kind: "project_instruction".into(),
            identity: path.to_string_lossy().into_owned(),
            relative_path: Some(item["path"].as_str().unwrap().replace('\\', "/")),
            source: if item["source"] == "editor" {
                Source::Editor
            } else if item["source"] == "snapshot" {
                Source::Snapshot
            } else {
                Source::Disk
            },
            exposure: Exposure::Supplied,
            revision: Some(
                item["revision"]
                    .as_str()
                    .ok_or("Instruction has no revision.")?
                    .into(),
            ),
            range: None,
            range_unit: None,
            reasons: vec![format!(
                "Instructions for {}",
                item["scope"].as_str().unwrap_or(".")
            )],
        };
        self.record(resource)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn revisions_and_ranges_keep_provenance_and_bounds_never_silently_drop_it() {
        let mut manifest = Manifest::default();
        manifest
            .content(
                "message",
                "request",
                "PRIVATE CONTENT",
                Source::User,
                "first reason",
            )
            .unwrap();
        manifest
            .content(
                "message",
                "request",
                "PRIVATE CONTENT",
                Source::User,
                "second reason",
            )
            .unwrap();
        assert_eq!(manifest.resources.len(), 1);
        assert_eq!(manifest.resources[0].reasons.len(), 2);
        manifest
            .content(
                "message",
                "request",
                "different content",
                Source::User,
                "later revision",
            )
            .unwrap();
        assert_eq!(manifest.resources.len(), 2);
        assert_eq!(manifest.resources[0].id, manifest.resources[1].id);
        assert_ne!(
            manifest.resources[0].revision,
            manifest.resources[1].revision
        );
        assert!(
            !serde_json::to_string(&manifest)
                .unwrap()
                .contains("PRIVATE CONTENT")
        );
        for index in 2..1024 {
            manifest
                .content(
                    "message",
                    &index.to_string(),
                    "content",
                    Source::User,
                    "test",
                )
                .unwrap();
        }
        assert!(
            manifest
                .content("message", "overflow", "content", Source::User, "test")
                .is_err()
        );
        assert_eq!(manifest.resources.len(), 1024);
    }
    #[test]
    fn empty_searches_still_record_the_exact_result_without_inventing_file_reads() {
        let root = tempfile::tempdir().unwrap();
        let mut manifest = Manifest::default();
        let result = serde_json::json!({"matches":[],"scannedFiles":3,"truncated":false});
        manifest
            .tool(
                root.path(),
                "sde_search_text",
                &serde_json::json!({"path":".","query":"missing"}),
                &result,
            )
            .unwrap();
        assert_eq!(manifest.resources.len(), 1);
        assert_eq!(manifest.resources[0].kind, "tool_result");
        assert_eq!(
            manifest.resources[0].revision,
            Some(revision(result.to_string().as_bytes()))
        );
    }
}
