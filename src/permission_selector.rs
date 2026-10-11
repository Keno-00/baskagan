use crate::Status;
use dioxus::prelude::*;
use dioxus_primitives::select::{
    Select, SelectGroup, SelectGroupLabel, SelectList, SelectOption, SelectTrigger,
};
use serde_json::{Value, json};

#[component]
pub fn PermissionSelector() -> Element {
    let mut open = use_signal(|| Some(false));
    let mut selection = use_signal(|| Some("inspect".to_string()));
    let mut available = use_signal(|| false);
    let mut status = use_context::<Status>();
    use_future(move || async move {
        let mut bridge = document::eval(
            r#"
            window.dadoPermissionSelector={close(){dioxus.send({action:'close'});},restore(value){dioxus.send({action:'restore',value});}};
            document.addEventListener('pointerdown',event=>{if(!event.target.closest('#permission-selector-mount'))dioxus.send({action:'close'});});
            document.addEventListener('focusin',event=>{const row=event.target.closest('#permission-selector-mount [role="option"]');if(row)dioxus.send({action:'hint',text:row.querySelector('[data-help]')?.dataset.help});});
            window.addEventListener('resize',()=>dioxus.send({action:'close'}));
            window.mountPermissionSelector?.();
        "#,
        );
        while let Ok(message) = bridge.recv::<Value>().await {
            if message["action"] == "hint" {
                status.hint.set(message["text"].as_str().map(String::from));
            }
            if message["action"] == "close" {
                open.set(Some(false));
            }
            if message["action"] == "restore" {
                let mode = message["value"]["mode"]
                    .as_str()
                    .unwrap_or("chat")
                    .to_string();
                if selection().as_deref() != Some(&mode) {
                    selection.set(Some(mode));
                    open.set(Some(false));
                }
                available.set(message["value"]["available"] == true);
            }
        }
    });
    let mode = selection().unwrap_or_else(|| "chat".into());
    let full = mode == "full";
    let label = match mode.as_str() {
        "full" => "Full access",
        "approve" => "On approval",
        "inspect" => "Inspect",
        _ => "Chat",
    };
    let help = match mode.as_str() {
        "full" => {
            "Full access · Live edits and commands with your user permissions, without approval · Changes apply next turn"
        }
        "approve" => {
            "On approval · Each command needs your approval and starts in a working copy · Edits require review/apply/save"
        }
        "inspect" => "Inspect · Read project files and propose edits; commands disabled",
        _ => "Chat · Supplied context only; project tools disabled",
    };
    rsx! {
        div {id:"permission-selector-mount",onmouseenter:move |_|status.hint.set(Some(help.into())),onmouseleave:move |_|status.hint.set(None),
            Select::<String> {open,value:Some(selection.into()),
                on_open_change:move |shown| {open.set(Some(shown));if shown {let _=document::eval(r#"
                    const root=document.getElementById('permission-selector-mount'),rect=root.querySelector('button').getBoundingClientRect(),width=Math.min(240,innerWidth-24);
                    root.style.setProperty('--selector-left',Math.max(12,Math.min(rect.left,innerWidth-width-12))+'px');
                    root.style.setProperty('--selector-bottom',(innerHeight-rect.top+8)+'px');
                    window.dadoChat?.closeActions();window.dadoModelSelector?.close();
                "#);}},
                on_value_change:move |value:Option<String>| {open.set(Some(false));if let Some(value)=value {let _=document::eval(&format!("document.dispatchEvent(new CustomEvent('dado-settings-change',{{detail:{{agentMode:{}}}}}));",json!(value)));}},
                SelectTrigger {class:if full {"permission-select-trigger full-access"}else{"permission-select-trigger"},aria_label:"Select permissions",
                    svg {view_box:"0 0 24 24","aria-hidden":"true",path {d:if full {"M12 3 2 21h20zM12 9v5M12 17v1"}else{"M12 3 4 6v6c0 5 8 9 8 9s8-4 8-9V6z"}}}
                    span {"{label}"}
                    svg {view_box:"0 0 24 24","aria-hidden":"true",path {d:"m7 10 5 5 5-5"}}
                }
                SelectList {class:"model-select-list permission-select-list",aria_label:"Permissions",
                    SelectGroup {
                        SelectGroupLabel {class:"model-select-label","Permissions"}
                        for (index,value,title,description) in [(0usize,"chat","Chat","Supplied context only; no project tools"),(1,"inspect","Inspect","Read files and propose edits; no commands"),(2,"approve","On approval","Ask before each command; edits remain reviewable proposals"),(3,"full","Full access","Live file edits and commands without approval; unrestricted user access")] {
                            SelectOption::<String> {key:"{value}",class:"model-select-option",index,value:value.to_string(),disabled:value!="chat" && !available(),
                                div {class:"permission-option-content","data-help":description,onmouseenter:move |_|status.hint.set(Some(description.into())),onmouseleave:move |_|status.hint.set(None),
                                    svg {view_box:"0 0 24 24","aria-hidden":"true",path {d:if value=="full" {"M12 3 2 21h20zM12 9v5M12 17v1"}else{"M12 3 4 6v6c0 5 8 9 8 9s8-4 8-9V6z"}}}
                                    span {"{title}"}
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
