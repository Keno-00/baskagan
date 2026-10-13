//! Persistent Settings preferences live in <user home>/.dado/sde/settings.json.
//! Add new preferences here with defaults and validation, then wire updatePreferences.
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub version: u32,
    pub name: String,
    pub personality: String,
    // Defaults for new conversations; active choices belong to project conversations.
    pub model: Option<String>,
    pub reasoning_effort: Option<String>,
    pub small_model: Option<String>,
    pub small_reasoning_effort: Option<String>,
    pub agent_mode: Option<String>,
    pub appearance: Appearance,
    pub general: General,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct General {
    pub show_systems_view: bool,
    pub automatic_project_understanding: bool,
    pub reopen_last_project: bool,
}
impl Default for General {
    fn default() -> Self {
        Self {
            show_systems_view: true,
            automatic_project_understanding: true,
            reopen_last_project: true,
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Appearance {
    pub theme: String,
    pub accent_color: String,
    pub font: String,
    pub text_size: u8,
    pub reduced_motion: bool,
    pub background_motion: bool,
    pub motion_intensity: String,
    pub navigation_motion: bool,
    pub agent_motion: bool,
    pub node_animations: bool,
    pub respect_reduced_motion: bool,
}
impl Default for Appearance {
    fn default() -> Self {
        Self {
            theme: "system".into(),
            accent_color: "blue".into(),
            font: "system".into(),
            text_size: 12,
            reduced_motion: false,
            background_motion: true,
            motion_intensity: "balanced".into(),
            navigation_motion: true,
            agent_motion: true,
            node_animations: true,
            respect_reduced_motion: true,
        }
    }
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            version: 1,
            name: String::new(),
            personality: String::new(),
            model: None,
            reasoning_effort: None,
            small_model: None,
            small_reasoning_effort: None,
            agent_mode: None,
            appearance: Appearance::default(),
            general: General::default(),
        }
    }
}
pub fn home() -> Result<PathBuf, String> {
    // Disposable native UI tests supply a separate home, never the real user's preferences.
    std::env::var_os("DADO_USER_HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .ok_or("User folder is unavailable.".into())
}
fn file(home: &Path) -> Result<PathBuf, String> {
    let folder = home.join(".dado").join("sde");
    for path in [
        home.join(".dado"),
        folder.clone(),
        folder.join("settings.json"),
    ] {
        if crate::project_store::linked(&path) {
            return Err("User settings storage cannot be a symbolic link.".into());
        }
    }
    Ok(folder.join("settings.json"))
}
fn validate(settings: &Settings) -> Result<(), String> {
    if settings.version != 1
        || settings.name.chars().count() > 100
        || settings.personality.chars().count() > 12000
        || !["system", "light", "dark"].contains(&settings.appearance.theme.as_str())
        || !["blue", "teal", "green", "violet", "rose", "amber"]
            .contains(&settings.appearance.accent_color.as_str())
        || !["system", "segoe", "arial"].contains(&settings.appearance.font.as_str())
        || !(10..=16).contains(&settings.appearance.text_size)
        || !["low", "balanced", "lively"].contains(&settings.appearance.motion_intensity.as_str())
        || settings
            .agent_mode
            .as_deref()
            .is_some_and(|mode| !["chat", "inspect", "approve", "full"].contains(&mode))
        || settings.small_model.as_ref().is_some_and(|s| s.len() > 200)
        || settings
            .small_reasoning_effort
            .as_ref()
            .is_some_and(|s| s.len() > 40)
        || settings.model.as_ref().is_some_and(|s| s.len() > 200)
        || settings
            .reasoning_effort
            .as_ref()
            .is_some_and(|s| s.len() > 40)
    {
        return Err("User settings have an unsupported value.".into());
    }
    Ok(())
}
pub fn load(home: &Path) -> Result<(Settings, bool), String> {
    let path = file(home)?;
    if !path.exists() {
        return Ok((Settings::default(), false));
    }
    let settings: Settings =
        serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
            .map_err(|e| format!("Could not read user settings: {e}"))?;
    validate(&settings)?;
    Ok((settings, true))
}
pub fn save(home: &Path, settings: &Settings) -> Result<(), String> {
    validate(settings)?;
    let path = file(home)?;
    let folder = path.parent().unwrap();
    std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
    let mut temp = tempfile::NamedTempFile::new_in(folder).map_err(|e| e.to_string())?;
    serde_json::to_writer_pretty(&mut temp, settings).map_err(|e| e.to_string())?;
    temp.as_file().sync_all().map_err(|e| e.to_string())?;
    temp.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preferences_survive_restart_in_required_user_subfolder() {
        let home = tempfile::tempdir().unwrap();
        let (mut s, exists) = load(home.path()).unwrap();
        assert!(!exists);
        s.name = "User".into();
        s.personality = "Keep things concise.".into();
        s.model = Some("test-model".into());
        s.reasoning_effort = Some("high".into());
        s.small_model = Some("small-test-model".into());
        s.small_reasoning_effort = Some("low".into());
        s.agent_mode = Some("full".into());
        s.appearance.theme = "dark".into();
        s.appearance.accent_color = "violet".into();
        s.appearance.font = "segoe".into();
        s.appearance.text_size = 14;
        s.appearance.reduced_motion = true;
        s.appearance.background_motion = false;
        s.appearance.motion_intensity = "lively".into();
        s.appearance.navigation_motion = false;
        s.appearance.agent_motion = false;
        s.appearance.node_animations = false;
        s.appearance.respect_reduced_motion = false;
        s.general.show_systems_view = false;
        s.general.automatic_project_understanding = false;
        s.general.reopen_last_project = false;
        save(home.path(), &s).unwrap();
        assert!(home.path().join(".dado/sde/settings.json").is_file());
        assert_eq!(load(home.path()).unwrap(), (s, true));
    }
    #[test]
    fn invalid_preferences_preserve_existing_file() {
        let home = tempfile::tempdir().unwrap();
        save(home.path(), &Settings::default()).unwrap();
        let mut s = Settings::default();
        s.appearance.text_size = 99;
        assert!(save(home.path(), &s).is_err());
        assert_eq!(load(home.path()).unwrap().0, Settings::default());
        s.appearance.text_size = 12;
        s.appearance.motion_intensity = "unsupported".into();
        assert!(save(home.path(), &s).is_err());
        assert_eq!(load(home.path()).unwrap().0, Settings::default());
        std::fs::write(home.path().join(".dado/sde/settings.json"), "broken").unwrap();
        assert!(load(home.path()).is_err());
        assert_eq!(
            std::fs::read_to_string(home.path().join(".dado/sde/settings.json")).unwrap(),
            "broken"
        );
    }
}
