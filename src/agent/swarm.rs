//! Observed native child ownership. This does not enable delegation or enforce spawn limits.
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default)]
pub struct Children {
    threads: BTreeMap<String, Option<String>>,
    parents: BTreeMap<String, String>,
    seen_turns: BTreeSet<(String, String)>,
    deferred: Vec<(String, String, bool)>,
}
impl Children {
    pub fn owns(&self, thread: &str) -> bool {
        self.threads.contains_key(thread)
    }
    pub fn observe(&mut self, lead: &str, item: &Value) -> Result<(), String> {
        if item["type"] != "collabAgentToolCall" || item["tool"] != "spawnAgent" {
            return Ok(());
        }
        let sender = item["senderThreadId"]
            .as_str()
            .ok_or("Missing delegation owner.")?;
        if sender != lead && !self.threads.contains_key(sender) {
            return Err("Delegation notification has no owned parent.".into());
        }
        for receiver in item["receiverThreadIds"].as_array().into_iter().flatten() {
            self.link(
                lead,
                sender,
                receiver.as_str().ok_or("Invalid child identity.")?,
            )?;
        }
        Ok(())
    }
    pub fn thread_started(&mut self, lead: &str, thread: &Value) -> Result<(), String> {
        let direct = thread["parentThreadId"].as_str();
        let source = thread["source"]["subAgent"]["thread_spawn"]["parent_thread_id"].as_str();
        if direct.is_some() && source.is_some() && direct != source {
            return Err("Child thread metadata disagrees about its parent.".into());
        }
        let Some(parent) = direct.or(source) else {
            return Ok(());
        };
        self.link(
            lead,
            parent,
            thread["id"]
                .as_str()
                .ok_or("Missing child thread identity.")?,
        )
    }
    fn link(&mut self, lead: &str, parent: &str, id: &str) -> Result<(), String> {
        self.validate_turn(parent, id)?;
        if id == lead || id == parent {
            return Err("Delegation notification contains an ownership cycle.".into());
        }
        if let Some(previous) = self.parents.get(id) {
            if previous != parent {
                return Err("Child thread changed its recorded parent.".into());
            }
        } else {
            if self.parents.len() >= 128 {
                return Err("Native child tracking exceeded its 128-thread bound.".into());
            }
            let mut ancestor = parent;
            while let Some(next) = self.parents.get(ancestor) {
                if next == id {
                    return Err("Delegation notification contains an ownership cycle.".into());
                }
                ancestor = next;
            }
            self.parents.insert(id.into(), parent.into());
        }
        loop {
            let ready: Vec<_> = self
                .parents
                .iter()
                .filter(|(id, parent)| {
                    !self.owns(id) && (parent.as_str() == lead || self.owns(parent))
                })
                .map(|(id, _)| id.clone())
                .collect();
            if ready.is_empty() {
                break;
            }
            for id in ready {
                self.threads.entry(id.clone()).or_default();
                let pending: Vec<_> = self
                    .deferred
                    .iter()
                    .filter(|(thread, _, _)| thread == &id)
                    .cloned()
                    .collect();
                self.deferred.retain(|(thread, _, _)| thread != &id);
                for (thread, turn, started) in pending {
                    if started {
                        self.turn_started(&thread, &turn)?;
                    } else {
                        self.turn_finished(&thread, &turn)?;
                    }
                }
            }
        }
        Ok(())
    }
    pub fn turn_started(&mut self, thread: &str, turn: &str) -> Result<bool, String> {
        self.validate_turn(thread, turn)?;
        let Some(current) = self.threads.get_mut(thread) else {
            self.defer(thread, turn, true)?;
            return Ok(false);
        };
        if turn.is_empty() || turn.len() > 200 {
            return Err("Invalid child turn identity.".into());
        }
        let identity = (thread.to_owned(), turn.to_owned());
        if self.seen_turns.contains(&identity) {
            return Ok(false);
        }
        if self.seen_turns.len() >= 512 {
            return Err("Native child turn tracking exceeded its bound.".into());
        }
        self.seen_turns.insert(identity);
        *current = Some(turn.into());
        Ok(true)
    }
    pub fn turn_finished(&mut self, thread: &str, turn: &str) -> Result<bool, String> {
        self.validate_turn(thread, turn)?;
        let Some(current) = self.threads.get_mut(thread) else {
            self.defer(thread, turn, false)?;
            return Ok(false);
        };
        if !turn.is_empty() && turn.len() <= 200 && self.seen_turns.len() < 512 {
            self.seen_turns.insert((thread.into(), turn.into()));
        }
        if current.as_deref() != Some(turn) {
            return Ok(false);
        }
        *current = None;
        Ok(true)
    }
    fn validate_turn(&self, thread: &str, turn: &str) -> Result<(), String> {
        if thread.is_empty() || thread.len() > 200 || turn.is_empty() || turn.len() > 200 {
            return Err("Invalid child turn identity.".into());
        }
        Ok(())
    }
    fn defer(&mut self, thread: &str, turn: &str, started: bool) -> Result<(), String> {
        let event = (thread.to_owned(), turn.to_owned(), started);
        if self.deferred.contains(&event) {
            return Ok(());
        }
        if self.deferred.len() >= 512 {
            return Err("Unresolved native child events exceeded their bound.".into());
        }
        self.deferred.push(event);
        Ok(())
    }
    pub fn running(&self) -> Vec<(String, String)> {
        self.threads
            .iter()
            .filter_map(|(thread, turn)| turn.as_ref().map(|turn| (thread.clone(), turn.clone())))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn parent_metadata_promotes_out_of_order_descendants_and_rejects_reparenting() {
        let mut children = Children::default();
        children.turn_started("nested", "early").unwrap();
        children
            .thread_started("lead", &json!({"id":"nested","parentThreadId":"parent"}))
            .unwrap();
        assert!(children.running().is_empty());
        children
            .thread_started("lead", &json!({"id":"stranger","parentThreadId":"outside"}))
            .unwrap();
        children.thread_started("lead", &json!({"id":"parent","source":{"subAgent":{"thread_spawn":{"parent_thread_id":"lead","depth":1}}}})).unwrap();
        assert_eq!(children.running(), vec![("nested".into(), "early".into())]);
        assert!(!children.owns("stranger"));
        assert!(
            children
                .thread_started("lead", &json!({"id":"parent","parentThreadId":"stranger"}))
                .is_err()
        );
        assert!(children.thread_started("lead", &json!({"id":"contradiction","parentThreadId":"lead","source":{"subAgent":{"thread_spawn":{"parent_thread_id":"outside"}}}})).is_err());
        assert!(
            children
                .thread_started("lead", &json!({"id":"outside","parentThreadId":"stranger"}))
                .is_err()
        );
    }
    #[test]
    fn early_events_wait_for_ownership_and_finished_turns_cannot_revive() {
        let mut children = Children::default();
        children.turn_started("child", "turn").unwrap();
        children.turn_started("stranger", "other").unwrap();
        assert!(children.running().is_empty());
        let spawn = |id| json!({"type":"collabAgentToolCall","tool":"spawnAgent","senderThreadId":"lead","receiverThreadIds":[id]});
        children.observe("lead", &spawn("child")).unwrap();
        assert_eq!(children.running(), vec![("child".into(), "turn".into())]);
        children.turn_finished("ended", "done").unwrap();
        children.turn_started("ended", "done").unwrap();
        children.observe("lead", &spawn("ended")).unwrap();
        assert_eq!(children.running().len(), 1);
        assert!(!children.owns("stranger"));
        for n in 0..511 {
            children.turn_started(&format!("unknown-{n}"), "t").unwrap();
        }
        assert!(children.turn_started("overflow", "t").is_err());
    }
    #[test]
    fn only_owned_children_are_cancel_targets_and_late_finishes_preserve_new_turns() {
        let mut children = Children::default();
        let spawn = |parent, child| json!({"type":"collabAgentToolCall","tool":"spawnAgent","senderThreadId":parent,"receiverThreadIds":[child]});
        children.observe("lead", &spawn("lead", "child")).unwrap();
        children
            .observe("lead", &spawn("child", "grandchild"))
            .unwrap();
        assert!(
            children
                .observe("lead", &spawn("stranger", "unrelated"))
                .is_err()
        );
        assert!(children.observe("lead", &spawn("child", "lead")).is_err());
        assert!(!children.turn_started("unrelated", "x").unwrap());
        assert!(
            !children
                .turn_finished("grandchild", "already-ended")
                .unwrap()
        );
        assert!(
            !children
                .turn_started("grandchild", "already-ended")
                .unwrap()
        );
        children.turn_started("child", "old").unwrap();
        children.turn_started("child", "new").unwrap();
        assert!(!children.turn_started("child", "old").unwrap());
        children.turn_started("grandchild", "nested").unwrap();
        assert!(!children.turn_finished("child", "old").unwrap());
        assert_eq!(children.running().len(), 2);
        assert!(children.turn_finished("child", "new").unwrap());
        assert!(!children.turn_started("child", "new").unwrap());
        assert_eq!(
            children.running(),
            vec![("grandchild".into(), "nested".into())]
        );
    }
}
