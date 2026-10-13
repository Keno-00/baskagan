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
    Mcp,
    WebSearch,
    Delegation,
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
        let tool = match item.kind {
            Kind::Command => "command",
            Kind::FileEdit => "file_edit",
            Kind::Mcp => "mcp",
            Kind::WebSearch => "web_search",
            Kind::Delegation => "agent_activity",
            Kind::Message => unreachable!(),
        };
        json!({"kind":"tool","itemId":id,"tool":tool,"status":item.status.label(),"output":item.text})
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
            Some("mcpToolCall") => Kind::Mcp,
            Some("webSearch") => Kind::WebSearch,
            Some("collabAgentToolCall") => Kind::Delegation,
            Some("subAgentActivity") => Kind::Delegation,
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
        } else if kind == Kind::WebSearch {
            Status::Completed
        } else if value["type"] == "subAgentActivity" {
            match value["kind"].as_str() {
                Some("completed") => Status::Completed,
                Some("interrupted") => Status::Interrupted,
                _ => Status::Unknown,
            }
        } else if kind == Kind::Mcp && !value["error"].is_null() {
            Status::Failed
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
        } else if kind == Kind::Mcp {
            // Keep credentials and arbitrary call arguments out of the activity row.
            format!(
                "{} / {}\n{}",
                value["server"].as_str().unwrap_or_default(),
                value["tool"].as_str().unwrap_or_default(),
                if status == Status::Failed {
                    "Tool reported an error"
                } else {
                    ""
                }
            )
        } else if kind == Kind::WebSearch {
            value["query"].as_str().unwrap_or_default().into()
        } else if kind == Kind::Delegation {
            if value["type"] == "subAgentActivity" {
                json!({"activity":value["kind"],"agent":value["agentThreadId"],"path":value["agentPath"]}).to_string()
            } else {
                json!({"tool":value["tool"],"agents":value["receiverThreadIds"],"states":value["agentsStates"]}).to_string()
            }
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
        let mut event = Self::tool(id, item);
        if kind == Kind::FileEdit {
            event["paths"] = json!(
                value["changes"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|c| c["path"].as_str())
                    .take(128)
                    .collect::<Vec<_>>()
            );
        }
        if kind == Kind::Command && status == Status::Completed {
            event["readPaths"] = json!(
                value["commandActions"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|a| a["type"] == "read")
                    .filter_map(|a| a["path"].as_str())
                    .take(128)
                    .collect::<Vec<_>>()
            );
        }
        Ok(Some(event))
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
    fn file_activity_keeps_structured_paths_without_parsing_output() {
        let mut items = Items::default();
        let event=items.item(&json!({"type":"fileChange","id":"edit","changes":[{"path":"src/main.rs"}],"status":"completed"}),true).unwrap().unwrap();
        assert_eq!(event["paths"], json!(["src/main.rs"]));
        assert_eq!(event["tool"], "file_edit");
        assert_eq!(event["status"], "completed");
    }
    #[test]
    fn subagent_activity_does_not_claim_started_children_finished() {
        for (activity, expected) in [
            ("started", "outcome unknown"),
            ("interacted", "outcome unknown"),
            ("interrupted", "interrupted"),
            ("completed", "completed"),
        ] {
            let mut items = Items::default();
            let item = json!({"type":"subAgentActivity","id":"activity","kind":activity,"agentThreadId":"child","agentPath":"/alpha","result":"SECRET"});
            assert_eq!(
                items.item(&item, false).unwrap().unwrap()["status"],
                "running"
            );
            let event = items.item(&item, true).unwrap().unwrap();
            assert_eq!(event["status"], expected);
            assert_eq!(event["tool"], "agent_activity");
            assert!(!event.to_string().contains("SECRET"));
            assert!(items.item(&item, true).unwrap().is_none());
        }
    }
    #[test]
    fn native_services_and_delegation_reconcile_without_claiming_child_success() {
        let mut items = Items::default();
        let fixture: Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/codex-0.162.0-alpha.2-activity.json"
        ))
        .unwrap();
        let mcp = fixture["items"][0].clone();
        assert_eq!(
            items.item(&mcp, false).unwrap().unwrap()["status"],
            "running"
        );
        let event = items.item(&mcp, true).unwrap().unwrap();
        assert_eq!(event["status"], "failed");
        assert_eq!(event["tool"], "mcp");
        assert!(!event.to_string().contains("SECRET"));
        assert!(items.item(&mcp, true).unwrap().is_none());
        let web = fixture["items"][1].clone();
        assert_eq!(
            items.item(&web, true).unwrap().unwrap()["status"],
            "completed"
        );
        let child = fixture["items"][2].clone();
        let event = items.item(&child, true).unwrap().unwrap();
        assert_eq!(event["tool"], "agent_activity");
        assert!(event["output"].as_str().unwrap().contains("running"));
        assert!(items.item(&child, false).unwrap().is_none());
        items
            .item(
                &json!({"type":"mcpToolCall","id":"unfinished","server":"local","tool":"wait"}),
                false,
            )
            .unwrap();
        let interrupted = items.finish(true);
        assert_eq!(interrupted.len(), 1);
        assert_eq!(interrupted[0]["status"], "interrupted");
    }
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
