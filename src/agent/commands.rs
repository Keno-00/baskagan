//! The same catalog and validation serve pointer and slash invocation.
use serde::Serialize;

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Command {
    pub id: &'static str,
    pub label: &'static str,
    pub alias: &'static str,
    pub keywords: &'static str,
    pub icon: &'static str,
    pub group: &'static str,
    pub kind: &'static str,
    pub help: &'static str,
}

pub const COMMANDS: &[Command] = &[
    Command {
        id: "create-skill",
        label: "Create skill",
        alias: "create-skill",
        keywords: "draft instructions workflow publish",
        icon: "M5 4h14v16H5zM8 8h8M12 11v6M9 14h6",
        group: "Create",
        kind: "local",
        help: "Draft a skill with the agent, review it in the editor, then publish",
    },
    Command {
        id: "skills",
        label: "Skills",
        alias: "skills",
        keywords: "instructions workflow",
        icon: "M5 4h14v16H5zM8 8h8M8 12h8M8 16h5",
        group: "Add",
        kind: "local",
        help: "Choose a skill for the next message",
    },
    Command {
        id: "permissions",
        label: "Permissions",
        alias: "permissions",
        keywords: "agent inspect chat",
        icon: "M12 3 4 6v6c0 5 8 9 8 9s8-4 8-9V6z",
        group: "Configure",
        kind: "local",
        help: "Choose the project agent permissions for the next turn",
    },
    Command {
        id: "attach-file",
        label: "Add files",
        alias: "file",
        keywords: "attach context",
        icon: "M14 3H6v18h12V7zM14 3v5h4",
        group: "Add",
        kind: "local",
        help: "Attach selected files to this message",
    },
    Command {
        id: "attach-folder",
        label: "Add folders",
        alias: "folder",
        keywords: "attach directory context",
        icon: "M3 7h7l2-2h9v15H3z",
        group: "Add",
        kind: "local",
        help: "Attach folder references to this message",
    },
    Command {
        id: "models",
        label: "Models",
        alias: "model",
        keywords: "reasoning select",
        icon: "M5 7h14M5 12h14M5 17h14",
        group: "Configure",
        kind: "local",
        help: "Choose a model and reasoning level",
    },
    Command {
        id: "model-settings",
        label: "Model Settings",
        alias: "model-settings",
        keywords: "configure codex account login",
        icon: "M4 7h16M4 17h16M8 4v6M16 14v6",
        group: "Configure",
        kind: "local",
        help: "Open model authentication and settings",
    },
    Command {
        id: "new-chat",
        label: "New chat",
        alias: "new",
        keywords: "conversation start",
        icon: "M12 5v14M5 12h14",
        group: "Chat",
        kind: "local",
        help: "Start a new conversation in this project",
    },
];

pub fn resolve(id: &str, project_open: bool) -> Result<Command, &'static str> {
    if !project_open {
        return Err("Open a project first.");
    }
    COMMANDS
        .iter()
        .find(|command| command.id == id)
        .copied()
        .ok_or("Unknown action.")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn commands_require_project_and_exact_identity() {
        assert!(resolve("attach-file", false).is_err());
        assert!(resolve("/file", true).is_err());
        assert!(resolve("shell", true).is_err());
        assert_eq!(resolve("attach-file", true).unwrap().alias, "file");
        let ids: std::collections::HashSet<_> = COMMANDS.iter().map(|c| c.id).collect();
        assert_eq!(ids.len(), COMMANDS.len());
    }
}
