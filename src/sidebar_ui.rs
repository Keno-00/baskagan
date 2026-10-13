use crate::{
    Status,
    ui_controls::{Icon, UiButton},
    user_settings::Settings,
};
use dioxus::prelude::*;
use serde::Deserialize;
use serde_json::{Value, json};
#[derive(Clone, Default, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct State {
    pub settings_open: bool,
    settings_tab: String,
    revision: u64,
    avatar: String,
    active: String,
    show_all: bool,
    expanded: Expanded,
    recents: Vec<Chat>,
    more: bool,
    concepts: Vec<Concept>,
    popup: Option<Popup>,
    preferences: Settings,
    ai: Ai,
    model_preferences: Value,
    usage: String,
    problem: String,
}
#[derive(Clone, Default, Deserialize, PartialEq)]
#[serde(default)]
struct Expanded {
    recents: bool,
    concepts: bool,
}
#[derive(Clone, Default, Deserialize, PartialEq)]
#[serde(default)]
struct Chat {
    id: String,
    title: String,
}
#[derive(Clone, Default, Deserialize, PartialEq)]
#[serde(default)]
struct Concept {
    id: String,
    name: String,
    open: bool,
    paths: Vec<ContextPath>,
    chats: Vec<Chat>,
}
#[derive(Clone, Default, Deserialize, PartialEq)]
#[serde(default)]
struct ContextPath {
    path: String,
}
#[derive(Clone, Default, Deserialize, PartialEq)]
#[serde(default)]
struct Popup {
    kind: String,
    id: String,
    name: String,
    paths: Vec<ContextPath>,
    bottom: f64,
}
#[derive(Clone, Default, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
struct Ai {
    ready: bool,
    signing_in: bool,
    inspection_available: bool,
    account: Option<Value>,
    models: Vec<Model>,
    error: Option<String>,
}
#[derive(Clone, Default, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
struct Model {
    model: String,
    display_name: String,
    supported_reasoning_efforts: Vec<Effort>,
}
#[derive(Clone, Default, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
struct Effort {
    reasoning_effort: String,
}
fn command(action: &str, key: &str, value: Value) {
    let _ = document::eval(&format!(
        "window.dadoSidebar?.ui({},{},{});",
        json!(action),
        json!(key),
        value
    ));
}
#[component]
pub fn SidebarUi() -> Element {
    let mut state = use_context::<Signal<State>>();
    use_future(move || async move {
        let mut bridge = document::eval(
            "window.dadoSidebarUI={restore:state=>dioxus.send(state)};window.dadoSidebar?.publish();",
        );
        while let Ok(next) = bridge.recv::<State>().await {
            if *state.peek() != next {
                state.set(next);
            }
        }
    });
    let s = state.read().clone();
    rsx! {
     style {{include_str!("sidebar_ui.css")}}
     div {class:"sidebar-content",
      div {class:"sidebar-brand","ᜊᜐ᜔ᜃᜄᜈ᜔"}
      UiButton {label:"New Chat",icon:"new-chat",text:"New Chat".to_string(),class:"sidebar-row",onpress:move |_|command("new","",Value::Null)}
      button {class:"sidebar-heading",r#type:"button",aria_expanded:s.expanded.recents,aria_controls:"recent-chats",onclick:move |_|command("expand","recents",Value::Null),span {"Recents"} Icon {name:"chevron"}}
      div {class:"sidebar-section",id:"recent-chats",hidden:!s.expanded.recents,
       for chat in s.recents {ChatRow {key:"{chat.id}",selected:chat.id==s.active,chat}}
       UiButton {label:if s.show_all {"Show less"}else{"Show more"},text:(if s.show_all {"Show less"}else{"Show more"}).to_string(),class:"sidebar-row sidebar-more",disabled:!s.more,onpress:move |_|command("more","",Value::Null)}
      }
      div {class:"sidebar-heading-row",
       button {class:"sidebar-heading",r#type:"button",aria_expanded:s.expanded.concepts,aria_controls:"concept-list",onclick:move |_|command("expand","concepts",Value::Null),span {"Concepts"} Icon {name:"chevron"}}
       UiButton {id:"create-concept",label:"Create concept",icon:"plus",class:"sidebar-icon",onpress:move |_|command("concept-edit","",Value::Null)}
      }
      div {class:"sidebar-section",id:"concept-list",hidden:!s.expanded.concepts,
       for concept in s.concepts {ConceptRow {key:"{concept.id}",concept,active:s.active.clone()}}
      }
      if !s.problem.is_empty() {p {class:"sidebar-storage-error",role:"alert","{s.problem}"}}
     }
     button {class:"sidebar-account",r#type:"button",aria_label:"Account and settings",aria_expanded:s.popup.as_ref().is_some_and(|p|p.kind=="account"),onclick:move |_|command("account","",Value::Null),
      img {class:"dioxus-avatar",src:s.avatar,alt:"Dioxus"} span {class:"account-name",{if s.preferences.name.trim().is_empty(){"You"}else{&s.preferences.name}}}
     }
     if let Some(popup)=s.popup.clone() {
      if popup.kind=="account" {section {class:"sidebar-popup",role:"dialog",aria_label:"Account and settings",style:format!("bottom:{}px;",popup.bottom),
       div {class:"account-menu-name",{if s.preferences.name.trim().is_empty(){"You"}else{&s.preferences.name}}}
       div {class:"account-usage",Icon {name:"usage"} span {"Usage remaining"} span {class:"usage-value","{s.usage}"}}
       UiButton {label:"Settings",text:"Settings".to_string(),icon:"settings",class:"sidebar-row",onpress:move |_|command("settings","",Value::Null)}
      }} else {ConceptEditor {key:"{s.revision}",popup}}
     }
    }
}
#[component]
fn ChatRow(chat: Chat, selected: bool) -> Element {
    let mut status = use_context::<Status>();
    let title = chat.title.clone();
    rsx! {button {class:if selected {"sidebar-row sidebar-chat selected"}else{"sidebar-row sidebar-chat"},r#type:"button",aria_label:chat.title.clone(),aria_current:if selected {"true"}else{"false"},onmouseenter:move |_|status.hint.set(Some(title.clone())),onmouseleave:move |_|status.hint.set(None),onclick:move |_|command("select",&chat.id,Value::Null),span {"{chat.title}"}}}
}
#[component]
fn ConceptRow(concept: Concept, active: String) -> Element {
    let toggle = concept.id.clone();
    let edit = concept.id.clone();
    let add = concept.id.clone();
    rsx! {div {class:"concept-group",
     div {class:"concept-row",
      button {r#type:"button",class:"sidebar-row",aria_expanded:concept.open,onclick:move |_|command("concept-toggle",&toggle,Value::Null),Icon {name:"folder"} span {"{concept.name}"}}
      div {"data-concept":concept.id.clone(),UiButton {id:format!("concept-{}",concept.id),label:format!("Edit concept {}",concept.name),icon:"layout",class:"sidebar-icon",onpress:move |_|command("concept-edit",&edit,Value::Null)}}
      UiButton {label:format!("New chat in {}",concept.name),icon:"new-chat",class:"sidebar-icon",onpress:move |_|command("new",&add,Value::Null)}
     }
     if concept.open {div {class:"concept-chats",for chat in concept.chats {ChatRow {key:"{chat.id}",selected:chat.id==active,chat}}}}
    }}
}
#[component]
fn ConceptEditor(popup: Popup) -> Element {
    let mut name = use_signal(|| popup.name.clone());
    let save_id = popup.id.clone();
    let pick_id = popup.id.clone();
    rsx! {section {class:"sidebar-popup concept-editor",role:"dialog",aria_label:if popup.id.is_empty(){"Create concept"}else{"Edit concept"},style:format!("bottom:{}px;",popup.bottom),
     div {class:"sheet-heading",strong {if popup.id.is_empty(){"New concept"}else{"Concept"}} UiButton {label:"Close concept details",icon:"close",class:"sidebar-icon",onpress:move |_|command("popup-close","",Value::Null)}}
     form {class:"concept-name-form",onsubmit:move |event|{event.prevent_default();command("concept-save",&save_id,json!(name()));},
      label {r#for:"concept-name","Name"}
      input {id:"concept-name",class:"settings-input",required:true,maxlength:100,value:name,oninput:move |event|name.set(event.value())}
      button {r#type:"submit",class:"chat-compact-action",if popup.id.is_empty(){"Create"}else{"Save"}}
     }
     if !popup.id.is_empty() {
      div {class:"concept-paths",for path in popup.paths {{let id=popup.id.clone();let value=path.path.clone();let label=path.path.rsplit(['\\','/']).next().unwrap_or(&path.path).to_string();rsx! {
       div {class:"concept-path",span {"{label}"} UiButton {label:format!("Remove {label}"),icon:"close",class:"sidebar-icon",onpress:move |_|command("path-remove",&id,json!(value))}}
      }}}}
      div {class:"concept-path-actions",for (kind,label) in [("file","Add files"),("folder","Add folders")] {{let id=pick_id.clone();rsx! {
       UiButton {label,icon:"plus",text:label.to_string(),class:"chat-compact-action",onpress:move |_|command("pick",&id,json!(kind))}
      }}}}
     }
    }}
}
#[component]
pub fn SettingsUi() -> Element {
    let state = use_context::<Signal<State>>();
    let s = state.read().clone();
    let title = match s.settings_tab.as_str() {
        "general" => "General",
        "model" => "Model",
        "agent" => "Agent",
        "appearance" => "Appearance",
        _ => "Personalization",
    };
    rsx! {
     style {{include_str!("appearance.css")}}
     section {id:"settings-host",hidden:!s.settings_open,aria_label:"Settings",
      nav {class:"settings-nav",aria_label:"Settings sections",h2 {"Settings"}
       for (key,label,icon) in [("general","General","settings"),("personalization","Personalization","profile"),("appearance","Appearance","appearance"),("model","Model","model"),("agent","Agent","terminal")] {
        button {class:if s.settings_tab==key {"sidebar-row selected"}else{"sidebar-row"},r#type:"button",aria_current:if s.settings_tab==key {"page"}else{"false"},onclick:move |_|command("settings",key,Value::Null),Icon {name:icon} span {"{label}"}}
       }
      }
      div {class:"settings-page",
       div {class:"settings-page-heading",h1 {"{title}"} UiButton {label:"Close settings",icon:"close",class:"sidebar-icon",onpress:move |_|command("settings-close","",Value::Null)}}
       if !s.problem.is_empty() {p {role:"alert",class:"settings-error","{s.problem}"}}
       div {class:"settings-group",
        match s.settings_tab.as_str() {
         "general"=>rsx! {GeneralSettings {general:s.preferences.general.clone()}},
         "appearance"=>rsx! {
          SettingsRow {label:"Mode",target:"appearance-theme",select {id:"appearance-theme",class:"settings-input",value:s.preferences.appearance.theme,onchange:move |e|command("appearance","theme",json!(e.value())),option {value:"system","System"} option {value:"light","Light"} option {value:"dark","Dark"}}}
          SettingsRow {label:"Accent color",target:"appearance-accent-color",select {id:"appearance-accent-color",class:"settings-input",value:s.preferences.appearance.accent_color,onchange:move |e|command("appearance","accentColor",json!(e.value())),for (value,label) in [("blue","Blue"),("teal","Teal"),("green","Green"),("violet","Violet"),("rose","Rose"),("amber","Amber")] {option {value,"{label}"}}}}
          SettingsRow {label:"Font",target:"appearance-font",select {id:"appearance-font",class:"settings-input",value:s.preferences.appearance.font,onchange:move |e|command("appearance","font",json!(e.value())),option {value:"system","System"} option {value:"segoe","Segoe UI"} option {value:"arial","Arial"}}}
          SettingsRow {label:"Text size",target:"appearance-size",select {id:"appearance-size",class:"settings-input",value:s.preferences.appearance.text_size.to_string(),onchange:move |e|{if let Ok(size)=e.value().parse::<u8>(){command("appearance","textSize",json!(size));}},for size in [10,11,12,13,14,15,16] {option {value:size.to_string(),"{size} px"}}}}
          SettingsRow {label:"Reduce motion",target:"appearance-motion",input {id:"appearance-motion",r#type:"checkbox",role:"switch",checked:s.preferences.appearance.reduced_motion,onchange:move |e|command("appearance","reducedMotion",json!(e.checked()))}}
          h2 {class:"settings-section-heading","Motion"}
          SettingsRow {label:"Background motion",target:"appearance-background-motion",input {id:"appearance-background-motion",r#type:"checkbox",role:"switch",checked:s.preferences.appearance.background_motion,onchange:move |e|command("appearance","backgroundMotion",json!(e.checked()))}}
          SettingsRow {label:"Motion intensity",target:"appearance-motion-intensity",select {id:"appearance-motion-intensity",class:"settings-input",value:s.preferences.appearance.motion_intensity,onchange:move |e|command("appearance","motionIntensity",json!(e.value())),for (value,label) in [("low","Low"),("balanced","Balanced"),("lively","Lively")] {option {value,"{label}"}}}}
          SettingsRow {label:"React to navigation",target:"appearance-navigation-motion",input {id:"appearance-navigation-motion",r#type:"checkbox",role:"switch",checked:s.preferences.appearance.navigation_motion,onchange:move |e|command("appearance","navigationMotion",json!(e.checked()))}}
          SettingsRow {label:"React to agent activity",target:"appearance-agent-motion",input {id:"appearance-agent-motion",r#type:"checkbox",role:"switch",checked:s.preferences.appearance.agent_motion,onchange:move |e|command("appearance","agentMotion",json!(e.checked()))}}
          SettingsRow {label:"Animate node changes",target:"appearance-node-animations",input {id:"appearance-node-animations",r#type:"checkbox",role:"switch",checked:s.preferences.appearance.node_animations,onchange:move |e|command("appearance","nodeAnimations",json!(e.checked()))}}
          SettingsRow {label:"Respect reduced motion",target:"appearance-respect-motion",input {id:"appearance-respect-motion",r#type:"checkbox",role:"switch",checked:s.preferences.appearance.respect_reduced_motion,onchange:move |e|command("appearance","respectReducedMotion",json!(e.checked()))}}
         },
         "agent"=>rsx! {
          SettingsRow {label:"Permissions",target:"settings-agent-mode",select {id:"settings-agent-mode",class:"settings-input",value:s.model_preferences["agentMode"].as_str().unwrap_or(if s.ai.inspection_available{"inspect"}else{"chat"}),onchange:move |e|command("setting","agentMode",json!(e.value())),
           for (key,label) in [("chat","Chat"),("inspect","Inspect"),("approve","On approval"),("full","Full access")] {option {value:key,disabled:key!="chat" && !s.ai.inspection_available,"{label}"}}
          }}
         },
         "model"=>rsx! {ModelSettings {ai:s.ai.clone(),prefs:serde_json::to_value(&s.preferences).unwrap_or_default()}},
         _=>rsx! {
          SettingsRow {label:"Name",target:"personalization-name",input {id:"personalization-name",class:"settings-input",maxlength:100,autocomplete:"given-name",value:s.preferences.name,oninput:move |e|command("preference","name",json!(e.value()))}}
          SettingsRow {label:"Personality",target:"personalization-personality",textarea {id:"personalization-personality",class:"settings-input",maxlength:12000,rows:4,value:s.preferences.personality,oninput:move |e|command("preference","personality",json!(e.value()))}}
         }
        }
       }
       if s.settings_tab=="appearance" {div {class:"settings-reset",UiButton {label:"Reset appearance to default",text:"Reset to default".to_string(),class:"chat-compact-action",onpress:move |_|command("appearance-reset","",Value::Null)}}}
      }
     }
    }
}
#[component]
fn SettingsRow(
    label: String,
    target: String,
    #[props(default)] hint: Option<String>,
    children: Element,
) -> Element {
    let mut status = use_context::<Status>();
    let hover_hint = hint.clone();
    rsx! {div {class:"settings-row",
        onmouseenter:move |_|{if let Some(text)=&hover_hint {status.hint.set(Some(text.clone()));}},
        onmouseleave:move |_|status.hint.set(None),
        onfocusin:move |_|{if let Some(text)=&hint {status.hint.set(Some(text.clone()));}},
        onfocusout:move |_|status.hint.set(None),
        label {r#for:target,"{label}"} div {class:"settings-control",{children}}
    }}
}
#[component]
fn GeneralSettings(general: crate::user_settings::General) -> Element {
    let mut understanding = use_context::<crate::project_model::Understanding>();
    let mut status = use_context::<Status>();
    let systems_hint = if general.show_systems_view {
        "why would you do this?"
    } else {
        "Explore your project through folders, concepts, and their connections."
    };
    rsx! {
        SettingsRow {label:"Show systems view",target:"general-systems",hint:Some(systems_hint.to_string()),
            input {id:"general-systems",r#type:"checkbox",role:"switch",checked:general.show_systems_view,
                onchange:move |e|{command("general","showSystemsView",json!(e.checked()));status.hint.set(Some(if e.checked(){"why would you do this?"}else{"Explore your project through folders, concepts, and their connections."}.into()));}
            }
        }
        SettingsRow {label:"Automatically update project understanding",target:"general-understanding",hint:Some("Update affected concepts after source changes while preserving established names and terminology.".into()),
            input {id:"general-understanding",r#type:"checkbox",role:"switch",checked:general.automatic_project_understanding,onchange:move |e|command("general","automaticProjectUnderstanding",json!(e.checked()))}
        }
        SettingsRow {label:"Reopen last project on startup",target:"general-reopen",hint:Some("Resume your previous project when launching the app.".into()),
            input {id:"general-reopen",r#type:"checkbox",role:"switch",checked:general.reopen_last_project,onchange:move |e|command("general","reopenLastProject",json!(e.checked()))}
        }
        SettingsRow {label:"Redo project understanding",target:"general-rebuild",hint:Some("Warning: rebuilds the whole mental model using model tokens and may change concepts and terminology. Existing understanding remains until it succeeds.".into()),
            UiButton {id:"general-rebuild",label:"Rebuild project understanding",text:"Rebuild".to_string(),icon:"warning",class:"chat-compact-action",
                disabled:(status.path)().is_empty() || ["checking", "working"].contains(&(status.phase)()),
                hint:Some("Warning: rebuilds the whole mental model using model tokens and may change concepts and terminology. Existing understanding remains until it succeeds.".into()),
                onpress:move |_| { understanding.rebuild += 1; status.phase.set("checking"); status.activity.set(Some("Preparing rebuild…".into())); }
            }
        }

    }
}
#[component]
fn ModelSettings(ai: Ai, prefs: Value) -> Element {
    let signed_in = ai.account.is_some();
    let (action, label) = if ai.signing_in {
        ("ai-login-cancel", "Cancel sign-in")
    } else if signed_in {
        ("ai-logout", "Sign out")
    } else if ai.ready {
        ("ai-login", "Sign in")
    } else {
        ("ai-refresh", "Reconnect")
    };
    rsx! {
     h2 {class:"settings-section-heading","Providers"}
     SettingsRow {label:"Codex Sign-in",target:"settings-auth",div {class:"settings-auth-control",if let Some(account)=ai.account {span {class:"settings-account",{account["email"].as_str().unwrap_or("Signed in")}}} UiButton {id:"settings-auth",label,text:label.to_string(),icon:"external",class:"chat-compact-action",onpress:move |_|command("ai",action,Value::Null)}}}
     if let Some(error)=ai.error {p {class:"settings-error",role:"alert","{error}"}}
     div {class:"settings-section-heading",h2 {"Defaults"} UiButton {label:"Refresh models",icon:"refresh",class:"sidebar-icon",onpress:move |_|command("ai","ai-refresh",Value::Null)}}
     for (model_key,effort_key,model_label,effort_label) in [("model","reasoningEffort","Default Model","Default Reasoning"),("smallModel","smallReasoningEffort","Default Small Model","Default Small Reasoning")] {
      DefaultModelSettings {models:ai.models.clone(),prefs:prefs.clone(),model_key,effort_key,model_label,effort_label}
     }
    }
}
#[component]
fn DefaultModelSettings(
    models: Vec<Model>,
    prefs: Value,
    model_key: &'static str,
    effort_key: &'static str,
    model_label: &'static str,
    effort_label: &'static str,
) -> Element {
    let selected = prefs[model_key].as_str().unwrap_or_default();
    let efforts = models
        .iter()
        .find(|m| m.model == selected)
        .map(|m| m.supported_reasoning_efforts.clone())
        .unwrap_or_default();
    let unavailable = !selected.is_empty() && !models.iter().any(|m| m.model == selected);
    rsx! {
     SettingsRow {label:model_label,target:format!("default-{model_key}"),select {id:format!("default-{model_key}"),class:"settings-input",value:selected,onchange:move |e|command("default-model",model_key,json!(e.value())),option {value:"","Provider default"} if unavailable {option {value:selected,"{selected}"}} for model in models {option {value:model.model,"{model.display_name}"}}}}
     SettingsRow {label:effort_label,target:format!("default-{effort_key}"),select {id:format!("default-{effort_key}"),class:"settings-input",value:prefs[effort_key].as_str().unwrap_or_default(),onchange:move |e|command("preference",effort_key,if e.value().is_empty(){Value::Null}else{json!(e.value())}),option {value:"","Provider default"} for effort in efforts {option {value:effort.reasoning_effort.clone(),{crate::model_selector::effort_label(&effort.reasoning_effort)}}}}}
    }
}
