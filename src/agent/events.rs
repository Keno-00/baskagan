//! Per-turn item reduction. Transport routing establishes ownership before this runs.
use serde_json::{Value, json};
use std::collections::HashMap;

const ITEMS: usize = 512;
const TEXT: usize = 2 * 1024 * 1024;
const OUTPUT: usize = 64 * 1024;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Message,
    Command,
    FileEdit,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Status {
    Running,
    Completed,
    Failed,
    Declined,
    Interrupted,
    Unknown,
}
impl Status {
    fn label(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Declined => "declined",
            Self::Interrupted => "interrupted",
            Self::Unknown => "outcome unknown",
        }
    }
}
struct Item {
    kind: Kind,
    status: Status,
    text: String,
}
#[derive(Default)]
pub struct Items {
    items: HashMap<String, Item>,
    text_bytes: usize,
}
impl Items {
    fn get(&mut self, id: &str, kind: Kind) -> Result<&mut Item, String> {
        if id.is_empty() || id.len() > 200 {
            return Err("Codex item has an invalid identity.".into());
        }
        if !self.items.contains_key(id) {
            if self.items.len() >= ITEMS {
                return Err("Codex turn exceeded its 512-item limit.".into());
            }
            self.items.insert(
                id.into(),
                Item {
                    kind,
                    status: Status::Running,
                    text: String::new(),
                },
            );
        }
        let item = self.items.get_mut(id).unwrap();
        if item.kind != kind {
            return Err("Codex reused an item identity for a different operation.".into());
        }
        Ok(item)
    }
    fn tool(id: &str, item: &Item) -> Value {
        json!({"kind":"tool","itemId":id,"tool":if item.kind==Kind::Command {"command"}else{"file_edit"},"status":item.status.label(),"output":item.text})
    }
    pub fn delta(&mut self, id: &str, text: &str) -> Result<Option<Value>, String> {
        if self.get(id, Kind::Message)?.status != Status::Running {
            return Ok(None);
        }
        if self.text_bytes.saturating_add(text.len()) > TEXT {
            return Err("Codex turn exceeded its 2 MB message limit.".into());
        }
        self.get(id, Kind::Message)?.text.push_str(text);
        self.text_bytes += text.len();
        Ok(Some(json!({"kind":"delta","itemId":id,"delta":text})))
    }
    pub fn item(&mut self, value: &Value, complete: bool) -> Result<Option<Value>, String> {
        let kind = match value["type"].as_str() {
            Some("agentMessage") => Kind::Message,
            Some("commandExecution") => Kind::Command,
            Some("fileChange") => Kind::FileEdit,
            _ => return Ok(None),
        };
        let id = value["id"].as_str().ok_or("Missing Codex item identity.")?;
        let previous = self.get(id, kind)?;
        if previous.status != Status::Running {
            return Ok(None);
        }
        if kind == Kind::Message {
            if !complete {
                return Ok(None);
            }
            let text = value["text"]
                .as_str()
                .ok_or("Missing final Codex message text.")?;
            let previous_len = previous.text.len();
            let size = self.text_bytes - previous_len;
            if size.saturating_add(text.len()) > TEXT {
                return Err("Codex turn exceeded its 2 MB message limit.".into());
            }
            let item = self.get(id, kind)?;
            item.text = text.into();
            item.status = Status::Completed;
            self.text_bytes = size + text.len();
            return Ok(Some(json!({"kind":"message","itemId":id,"text":text})));
        }
        let status = if !complete {
            Status::Running
        } else {
            match value["status"].as_str() {
                Some("completed") => Status::Completed,
                Some("failed") => Status::Failed,
                Some("declined") => Status::Declined,
                Some("interrupted") => Status::Interrupted,
                _ => Status::Unknown,
            }
        };
        // A successful item label cannot turn a nonzero command exit into success.
        let status = if kind == Kind::Command && status == Status::Completed {
            match value["exitCode"].as_i64() {
                Some(0) => Status::Completed,
                Some(_) => Status::Failed,
                None => Status::Unknown,
            }
        } else {
            status
        };
        let raw = if kind == Kind::Command {
            format!(
                "{}\n{}\nExit: {}",
                value["command"].as_str().unwrap_or_default(),
                value["aggregatedOutput"].as_str().unwrap_or_default(),
                value["exitCode"]
            )
        } else {
            value["changes"].to_string()
        };
        let mut text = String::new();
        for c in raw.chars() {
            if text.len() + c.len_utf8() > OUTPUT {
                text.push_str("\n[Output truncated]");
                break;
            }
            text.push(c);
        }
        let item = self.get(id, kind)?;
        if item.status == status && item.text == text {
            return Ok(None);
        }
        item.status = status;
        item.text = text;
        Ok(Some(Self::tool(id, item)))
    }
    pub fn finish(&mut self, interrupted: bool) -> Vec<Value> {
        let mut events = Vec::new();
        for (id, item) in &mut self.items {
            if item.status != Status::Running {
                continue;
            }
            item.status = if interrupted {
                Status::Interrupted
            } else {
                Status::Unknown
            };
            if item.kind != Kind::Message {
                events.push(Self::tool(id, item));
            }
        }
        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn final_messages_replace_partial_text_and_ignore_late_updates() {
        let mut items = Items::default();
        items.delta("a", "partial").unwrap();
        let complete = json!({"type":"agentMessage","id":"a","text":"Final answer"});
        assert_eq!(
            items.item(&complete, true).unwrap().unwrap()["text"],
            "Final answer"
        );
        assert!(items.item(&complete, true).unwrap().is_none());
        assert!(items.delta("a", "late").unwrap().is_none());
        assert!(
            items
                .item(&json!({"type":"fileChange","id":"a"}), false)
                .is_err()
        );
    }
    #[test]
    fn command_outcomes_never_infer_success_and_cannot_restart() {
        for (status, exit, expected) in [
            ("completed", json!(0), "completed"),
            ("completed", json!(1), "failed"),
            ("completed", Value::Null, "outcome unknown"),
            ("failed", json!(0), "failed"),
            ("declined", Value::Null, "declined"),
        ] {
            let mut items = Items::default();
            let mut item = json!({"type":"commandExecution","id":"c","command":"check","status":status,"exitCode":exit,"aggregatedOutput":"result"});
            items.item(&item, false).unwrap();
            assert_eq!(
                items.item(&item, true).unwrap().unwrap()["status"],
                expected
            );
            assert!(items.item(&item, true).unwrap().is_none());
            item["status"] = json!("inProgress");
            assert!(items.item(&item, false).unwrap().is_none());
        }
    }
    #[test]
    fn unfinished_tools_keep_unknown_or_interrupted_outcomes_and_output_is_bounded() {
        for interrupted in [true, false] {
            let mut items = Items::default();
            items
                .item(&json!({"type":"fileChange","id":"f","changes":[]}), false)
                .unwrap();
            assert_eq!(
                items.finish(interrupted)[0]["status"],
                if interrupted {
                    "interrupted"
                } else {
                    "outcome unknown"
                }
            );
            assert!(items.finish(interrupted).is_empty());
        }
        let mut items = Items::default();
        let event=items.item(&json!({"type":"commandExecution","id":"c","command":"check","aggregatedOutput":"ᜊ".repeat(OUTPUT),"exitCode":0,"status":"completed"}),true).unwrap().unwrap();
        assert!(event["output"].as_str().unwrap().len() < OUTPUT + 30);
        assert!(
            event["output"]
                .as_str()
                .unwrap()
                .ends_with("[Output truncated]")
        );
        assert!(items.delta("m", &"a".repeat(TEXT + 1)).is_err());
    }
}
