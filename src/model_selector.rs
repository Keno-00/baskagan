use crate::Status;
use dioxus::prelude::*;
use dioxus_primitives::select::{
    Select, SelectGroup, SelectGroupLabel, SelectList, SelectOption, SelectTrigger,
};
use serde_json::{Value, json};

pub fn effort_label(effort: &str) -> &str {
    match effort {
        "none" => "None",
        "minimal" => "Minimal",
        "low" => "Low",
        "medium" => "Medium",
        "high" => "High",
        "xhigh" => "Extra high",
        "max" => "Maximum",
        "ultra" => "Ultra",
        other => other,
    }
}
#[component]
pub fn ModelSelector() -> Element {
    let mut open = use_signal(|| Some(false));
    let mut selection = use_signal(|| None::<String>);
    let mut models = use_signal(Vec::<Value>::new);
    let mut effort = use_signal(String::new);
    let mut status = use_context::<Status>();
    use_future(move || async move {
        let mut bridge = document::eval(
            r#"
            window.dadoModelSelector = {close() {dioxus.send({action:'close'});}, restore(value) {dioxus.send({action:'restore',value});}, catalog(value) {dioxus.send({action:'catalog',value});}};
            document.addEventListener('pointerdown', event => {if (!event.target.closest('#model-selector-mount')) dioxus.send({action:'close'});});
            window.addEventListener('resize', () => dioxus.send({action:'close'}));
            window.mountModelSelector?.();
        "#,
        );
        while let Ok(message) = bridge.recv::<Value>().await {
            if message["action"] == "close" {
                open.set(Some(false));
            }
            if message["action"] == "catalog" {
                models.set(message["value"].as_array().cloned().unwrap_or_default());
            }
            if message["action"] == "restore" {
                let next = message["value"]["model"]
                    .as_str()
                    .map(|s| format!("model:{s}"));
                if selection() != next {
                    open.set(Some(false));
                    selection.set(next);
                }
                effort.set(
                    message["value"]["reasoningEffort"]
                        .as_str()
                        .unwrap_or_default()
                        .into(),
                );
            }
        }
    });
    let list = models();
    let selected = selection().unwrap_or_default();
    let model = list
        .iter()
        .find(|m| selected == format!("model:{}", m["model"].as_str().unwrap_or_default()));
    let label = model
        .and_then(|m| m["displayName"].as_str())
        .unwrap_or("No Models")
        .to_string();
    let efforts: Vec<String> = model
        .into_iter()
        .flat_map(|m| {
            m["supportedReasoningEfforts"]
                .as_array()
                .into_iter()
                .flatten()
        })
        .filter_map(|e| e["reasoningEffort"].as_str().map(String::from))
        .collect();
    let index = efforts.iter().position(|s| *s == effort()).unwrap_or(0);
    let reasoning_label =
        effort_label(efforts.get(index).map(String::as_str).unwrap_or("")).to_string();
    let last = efforts.len().saturating_sub(1);
    let config_index = list.len().max(1);
    let options: Vec<_> = list
        .iter()
        .enumerate()
        .map(|(i, m)| {
            (
                i,
                m["model"].as_str().unwrap_or_default().to_string(),
                m["displayName"].as_str().unwrap_or("Model").to_string(),
            )
        })
        .collect();
    rsx! {
        div { id: "model-selector-mount",
            onmouseenter: move |_| status.hint.set(Some("Choose a model and reasoning level".into())),
            onmouseleave: move |_| status.hint.set(None),
            Select::<String> { class: "model-select", open, value: Some(selection.into()),
                on_open_change: move |shown| {open.set(Some(shown));if shown {let _=document::eval(r#"
                    const trigger=document.querySelector('#model-selector-mount button'),root=document.getElementById('model-selector-mount');
                    const rect=trigger.getBoundingClientRect(),width=Math.min(240,window.innerWidth-24);
                    root.style.setProperty('--selector-left',Math.max(12,Math.min(rect.right-width,window.innerWidth-width-12))+'px');
                    root.style.setProperty('--selector-bottom',(window.innerHeight-rect.top+8)+'px');
                    window.dadoChat?.closeActions();
                "#);}},
                on_value_change: move |value:Option<String>| {open.set(Some(false));
                    if value.as_deref()==Some("settings") {let _=document::eval("window.dadoSidebar?.openModelSettings();");}
                    else if let Some(value)=value.and_then(|s|s.strip_prefix("model:").map(String::from)) {
                        let _=document::eval(&format!("window.dadoChat?.selectModel({});",json!(value)));
                    }
                },
                SelectTrigger { class: "model-select-trigger", aria_label: "Select model and reasoning",
                    span { "{label}" }
                    svg { view_box: "0 0 24 24", "aria-hidden": "true", path {d:"m7 10 5 5 5-5"} }
                }
                SelectList { class: "model-select-list", aria_label: "Model selector",
                    SelectGroup {
                        SelectGroupLabel {class:"model-select-label","Models"}
                        if list.is_empty() {SelectOption::<String> {class:"model-select-option",index:0usize,value:"none".to_string(),disabled:true,"No Models"}}
                        for (i,slug,name) in options {
                            SelectOption::<String> {key:"{slug}",class:"model-select-option",index:i,value:format!("model:{slug}"),"{name}"}
                        }
                    }
                    if open()==Some(true) && !efforts.is_empty() {
                        div {class:"model-reasoning",role:"group",aria_label:"Reasoning",
                            div {class:"model-select-label","Reasoning"}
                            div {class:"reasoning-value","{reasoning_label}"}
                            input {r#type:"range",min:"0",max:"{last}",step:"1",value:"{index}",disabled:last==0,aria_label:"Reasoning level",aria_valuetext:reasoning_label,
                                onfocus:move |_|open.set(Some(true)),
                                onkeydown:move |event|{if event.key()!=Key::Escape && event.key()!=Key::Tab {event.stop_propagation();}},
                                oninput:move |event|{if let Some(value)=event.value().parse::<usize>().ok().and_then(|i|efforts.get(i)){effort.set(value.clone());let _=document::eval(&format!("document.dispatchEvent(new CustomEvent('dado-settings-change',{{detail:{{reasoningEffort:{}}}}}));",json!(value)));}},
                            }
                        }
                    }
                    SelectGroup {
                        SelectGroupLabel {class:"model-select-label","Configure"}
                        SelectOption::<String> {class:"model-select-option",index:config_index,value:"settings".to_string(),"Model Settings"}
                    }
                }
            }
        }
    }
}
