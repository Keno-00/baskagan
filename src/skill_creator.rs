use crate::ui_controls::{Icon, UiButton};
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
#[derive(Clone, Default, Deserialize, PartialEq)]
#[serde(default)]
struct State {
    shown: bool,
    version: u64,
    reset: u64,
    loading: bool,
    error: String,
    drafts: Vec<Item>,
    skills: Vec<Item>,
    busy: Vec<String>,
}
#[derive(Clone, Default, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
struct Item {
    id: String,
    name: String,
    scope: String,
    can_publish: bool,
}
#[derive(Clone, Serialize)]
struct Spec {
    name: String,
    task: String,
    when: String,
    scope: String,
}
impl Default for Spec {
    fn default() -> Self {
        Self {
            name: String::new(),
            task: String::new(),
            when: String::new(),
            scope: "project-private".into(),
        }
    }
}
fn action(action: &str, key: &str, value: Value) {
    let _ = document::eval(&format!(
        "window.dadoChat?.skillCreator({},{},{});",
        json!(action),
        json!(key),
        value
    ));
}
#[component]
pub fn SkillCreator() -> Element {
    let mut state = use_signal(State::default);
    let mut spec = use_signal(Spec::default);
    let mut previous_reset = use_signal(|| 0);
    use_future(move || async move {
        let mut bridge = document::eval(
            "window.dadoSkillCreator={restore:state=>dioxus.send(state)};window.dadoChat?.skillCreatorReady();",
        );
        while let Ok(next) = bridge.recv::<State>().await {
            state.set(next);
        }
    });
    use_effect(move || {
        let reset = state.read().reset;
        if reset != *previous_reset.peek() {
            spec.set(Spec::default());
            previous_reset.set(reset);
        }
    });
    let s = state.read().clone();
    rsx! {
     style {{include_str!("skill_creator.css")}}
     if s.shown {dialog {class:"skill-creator",aria_label:"Create skill",
      onmounted:move |_|{let _=document::eval("const dialog=document.querySelector('.skill-creator');if(dialog && !dialog.open)dialog.showModal();");},
      oncancel:move |e|{e.prevent_default();action("close","",Value::Null);},onkeydown:move |e|e.stop_propagation(),
      div {class:"sheet-heading",h2 {"Create skill"} UiButton {label:"Close skill creation",icon:"close",class:"sidebar-icon",onpress:move |_|action("close","",Value::Null)}}
      form {onsubmit:move |e|{e.prevent_default();action("draft","",json!(spec()));},
       div {class:"settings-group",
        div {class:"settings-row",label {r#for:"skill-name","Name"} div {class:"settings-control",input {id:"skill-name",class:"settings-input",required:true,pattern:"[a-z0-9]+(?:-[a-z0-9]+)*",maxlength:64,placeholder:"review-changes",value:spec.read().name.clone(),oninput:move |e|spec.write().name=e.value()}}}
        div {class:"settings-row",label {r#for:"skill-task","Task"} div {class:"settings-control",textarea {id:"skill-task",class:"settings-input",required:true,maxlength:4000,rows:3,value:spec.read().task.clone(),oninput:move |e|spec.write().task=e.value()}}}
        div {class:"settings-row",label {r#for:"skill-when","When to use"} div {class:"settings-control",textarea {id:"skill-when",class:"settings-input",required:true,maxlength:2000,rows:2,value:spec.read().when.clone(),oninput:move |e|spec.write().when=e.value()}}}
        div {class:"settings-row",label {r#for:"skill-scope","Scope"} div {class:"settings-control",select {id:"skill-scope",class:"settings-input",value:spec.read().scope.clone(),onchange:move |e|spec.write().scope=e.value(),option {value:"project-private","This project · private"} option {value:"repo","Repository · shared"}}}}
       }
       div {class:"skill-creator-actions",button {r#type:"submit",class:"chat-compact-action",Icon {name:"skill"} span {"Draft with agent"}}}
      }
      if !s.error.is_empty() {p {role:"alert",class:"settings-error","{s.error}"}}
      if s.loading {p {class:"skill-loading","Loading…"}}
      if !s.drafts.is_empty() || !s.skills.is_empty() {details {class:"skill-existing",summary {"Drafts and private skills"} div {class:"skill-draft-list",
       for item in s.drafts {SkillRow {key:"draft:{item.id}",busy:s.busy.contains(&item.id),item,private:false}}
       for item in s.skills {SkillRow {key:"skill:{item.id}",busy:s.busy.contains(&item.id),item,private:true}}
      }}}
     }}
    }
}
#[component]
fn SkillRow(item: Item, busy: bool, private: bool) -> Element {
    let review = item.id.clone();
    let publish = item.id.clone();
    let publish_label = if private {
        format!("Share {} with repository", item.name)
    } else {
        format!("Publish {}", item.name)
    };
    let publish_icon = if private { "external" } else { "check" };
    let publish_text = if private { "Share" } else { "Publish" }.to_string();
    rsx! {div {class:"skill-draft-row",Icon {name:"skill"} span {class:"skill-item-name","{item.name}"} span {class:"skill-item-scope",if private {"Private"}else{"Draft"}}
     UiButton {label:format!("Review {}{}",if private {"private skill "}else{""},item.name),icon:"editor",class:"sidebar-icon",onpress:move |_|action(if private {"review-private"}else{"review"},&review,Value::Null)}
     UiButton {label:publish_label,icon:publish_icon,text:publish_text,class:"chat-compact-action",disabled:busy || (!private && !item.can_publish),onpress:move |_|action(if private {"share"}else{"publish"},&publish,Value::Null)}
    }}
}
