//! Execution profiles are captured by each run; UI changes cannot widen it.
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Profile {
    Chat,
    Inspect,
    Approve,
    Full,
}
impl Profile {
    pub fn parse(value: &Value) -> Result<Self, String> {
        match value.as_str().unwrap_or("chat") {
            "chat" => Ok(Self::Chat),
            "inspect" => Ok(Self::Inspect),
            "approve" => Ok(Self::Approve),
            "full" => Ok(Self::Full),
            _ => Err("Unknown execution profile.".into()),
        }
    }
    pub fn tools(self) -> bool {
        self != Self::Chat
    }
    pub fn full(self) -> bool {
        self == Self::Full
    }
    pub fn sandbox(self) -> &'static str {
        if self.full() {
            "danger-full-access"
        } else {
            "read-only"
        }
    }
    pub fn approval(self) -> &'static str {
        if self.full() { "never" } else { "on-request" }
    }
    pub fn policy(self) -> Value {
        if self.full() {
            json!({"type":"dangerFullAccess"})
        } else {
            json!({"type":"readOnly","networkAccess":false})
        }
    }
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CommandRequest {
    pub command: String,
    pub reason: String,
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
}
fn default_timeout() -> u64 {
    60
}
impl CommandRequest {
    pub fn parse(args: &Value) -> Result<Self, String> {
        let command: Self =
            serde_json::from_value(args.clone()).map_err(|_| "Invalid command request.")?;
        if command.command.trim().is_empty()
            || command.command.len() > 16 * 1024
            || command.command.contains('\0')
            || command.reason.trim().is_empty()
            || command.reason.len() > 2048
            || !(1..=120).contains(&command.timeout_seconds)
        {
            return Err(
                "Commands require a reason, at most 16 KB of text and a timeout of 1–120 seconds."
                    .into(),
            );
        }
        Ok(command)
    }
    pub fn argv(&self) -> Vec<String> {
        if cfg!(windows) {
            vec![
                "powershell.exe".into(),
                "-NoLogo".into(),
                "-NoProfile".into(),
                "-NonInteractive".into(),
                "-Command".into(),
                self.command.clone(),
            ]
        } else {
            vec!["/bin/sh".into(), "-c".into(), self.command.clone()]
        }
    }
}
pub fn command_tool() -> Value {
    json!({"type":"function","name":"sde_run_command","description":"Request one shell command in the owned project working copy. Every command requires the user's one-time approval. Commands run with the user's OS permissions after approval; the working copy is not an OS sandbox. Output is bounded; timeout at most 120 seconds. Existing text changes become reviewable proposals, never automatically applied to live files. Dependencies/generated/private files are not copied. Do not use paths to the live project to bypass the working copy.","inputSchema":{"type":"object","properties":{"command":{"type":"string","minLength":1,"maxLength":16384},"reason":{"type":"string","minLength":1,"maxLength":2048},"timeoutSeconds":{"type":"integer","minimum":1,"maximum":120}},"required":["command","reason"],"additionalProperties":false}})
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn execution_profiles_and_commands_are_explicit_and_bounded() {
        assert!(!Profile::parse(&json!("inspect")).unwrap().full());
        assert_eq!(
            Profile::parse(&json!("full")).unwrap().policy(),
            json!({"type":"dangerFullAccess"})
        );
        assert!(Profile::parse(&json!("arbitrary")).is_err());
        for args in [
            json!({"command":"","reason":"test"}),
            json!({"command":"echo test","reason":"test","timeoutSeconds":121}),
            json!({"command":"echo test","reason":"test","approval":true}),
        ] {
            assert!(CommandRequest::parse(&args).is_err());
        }
        let command =
            CommandRequest::parse(&json!({"command":"echo '$()'","reason":"literal command"}))
                .unwrap();
        assert_eq!(command.argv().last().unwrap(), "echo '$()'");
    }
}
