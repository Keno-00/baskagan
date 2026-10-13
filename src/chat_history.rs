use crate::ui_controls::{Icon, UiButton};
use dioxus::prelude::*;
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::PathBuf;

#[derive(Clone, Default, Deserialize, PartialEq)]
#[serde(default)]
struct Layout {
    shown: bool,
    index: usize,
}
#[derive(Clone, Default, Deserialize, PartialEq)]
#[serde(default)]
struct History {
    chat: String,
    follow: bool,
    revision: u64,
    messages: Vec<Message>,
}
#[derive(Clone, Default, Deserialize, PartialEq)]
#[serde(default)]
struct Message {
    id: String,
    role: String,
    mode: String,
    text: String,
    time: String,
    clock: String,
    status: String,
    response: String,
    attachments: Vec<Reference>,
    skills: Vec<Reference>,
    comments: Vec<Value>,
    tools: Vec<Tool>,
    changes: Option<Changes>,
    approvals: Vec<Approval>,
}
#[derive(Clone, Default, Deserialize, PartialEq)]
#[serde(default)]
struct Reference {
    name: String,
    path: String,
    scope: String,
    revision: String,
}
#[derive(Clone, Default, Deserialize, PartialEq)]
#[serde(default)]
struct Tool {
    id: String,
    tool: String,
    status: String,
    output: String,
}
#[derive(Clone, Default, Deserialize, PartialEq)]
#[serde(default)]
struct Changes {
    count: usize,
    status: String,
    applying: bool,
}
#[derive(Clone, Default, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
struct Approval {
    approval_id: String,
    request_id: String,
    command: String,
    reason: String,
    cwd: String,
    timeout_seconds: u64,
    submitted: bool,
}
#[derive(Clone, Default, Deserialize, PartialEq)]
#[serde(default)]
struct Review {
    message: String,
    state: String,
    busy: bool,
    issue: Option<String>,
    changes: Vec<FileChange>,
}
#[derive(Clone, Default, Deserialize, PartialEq)]
#[serde(default)]
struct FileChange {
    path: String,
    operation: String,
    conflict: bool,
    lines: Vec<DiffLine>,
}
#[derive(Clone, Default, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
struct DiffLine {
    kind: String,
    text: String,
    old_line: Option<u64>,
    new_line: Option<u64>,
}
fn action(action: &str, id: &str, value: Value) {
    let _ = document::eval(&format!(
        "window.dadoChat?.historyAction({},{},{});",
        json!(action),
        json!(id),
        value
    ));
}
#[component]
pub fn ChatHistory(root: Option<PathBuf>) -> Element {
    let mut history = use_signal(History::default);
    let mut review = use_signal(|| None::<Review>);
    let mut layout = use_signal(Layout::default);
    let mut was_choosing = use_signal(|| false);
    use_future(move || async move {
        let mut bridge = document::eval(
            "window.dadoHistory={restore:state=>dioxus.send({kind:'history',state}),review:state=>dioxus.send({kind:'review',state}),layout:(shown,index)=>dioxus.send({kind:'layout',state:{shown,index}})};window.dadoChat?.historyReady();",
        );
        while let Ok(message) = bridge.recv::<Value>().await {
            if message["kind"] == "history" {
                if let Ok(next) = serde_json::from_value(message["state"].clone())
                    && *history.peek() != next
                {
                    history.set(next);
                }
            } else if message["kind"] == "review"
                && let Ok(next) = serde_json::from_value(message["state"].clone())
            {
                review.set(next);
            } else if message["kind"] == "layout"
                && let Ok(next) = serde_json::from_value(message["state"].clone())
            {
                layout.set(next);
            }
        }
    });
    use_effect(move || {
        let state = history.read();
        let chat = state.chat.clone();
        let follow = state.follow;
        let revision = state.revision;
        let _ = document::eval(&format!(
            "requestAnimationFrame(()=>requestAnimationFrame(()=>window.dadoChat?.historyRendered({},{},{})));",
            json!(chat),
            follow,
            revision
        ));
    });
    use_effect(move || {
        let shown = layout.read().shown;
        if shown || *was_choosing.peek() {
            let selector = if shown {
                ".chat-layouts button"
            } else {
                ".chat-dots"
            };
            let _ = document::eval(&format!(
                "requestAnimationFrame(()=>document.querySelector({})?.focus());",
                json!(selector)
            ));
        }
        was_choosing.set(shown);
    });
    let state = history.read().clone();
    let choosing = layout.read().shown;
    let layout_index = layout.read().index;
    rsx! {
        style { {include_str!("chat_history.css")} }
        div {class:if choosing {"chat-divider choosing"}else{"chat-divider"},
            div {hidden:choosing, UiButton {label:"Change chat layout",class:"chat-dots",icon:"layout",onpress:move |_|action("layout-open","",Value::Null)}}
            div {class:"chat-layouts",hidden:!choosing,
                for (index,label,top) in [(0,"Systems larger",15),(1,"Balanced layout",10),(2,"Chat larger",5)] {
                    button {r#type:"button",aria_label:label,aria_pressed:if index==layout_index {"true"}else{"false"},onclick:move |_|action("layout",&index.to_string(),Value::Null),
                        svg {view_box:"0 0 28 28","aria-hidden":"true",
                            rect {x:3,y:3,width:22,height:top,rx:2,fill:"var(--ui-primary)"}
                            rect {x:3,y:top+5,width:22,height:20-top,rx:2,fill:"var(--ui-secondary)"}
                        }
                    }
                }
            }
        }
        div {class:"chat-history",role:"log",aria_label:"Messages",aria_live:"polite",
            div {class:"chat-messages",
                for message in state.messages { MessageView {key:"{state.chat}:{message.id}",root:root.clone(),review:review.read().clone().filter(|r|r.message==message.id),message} }
            }
        }

    }
}
#[component]
fn MessageView(message: Message, root: Option<PathBuf>, review: Option<Review>) -> Element {
    let assistant = message.role == "assistant";
    let class = if assistant {
        "chat-assistant-message"
    } else {
        "chat-user-message"
    };
    let streaming = if message.status == "streaming" {
        " streaming"
    } else {
        ""
    };
    let review_id = message.id.clone();
    let apply_id = message.id.clone();
    let applied = message
        .changes
        .as_ref()
        .is_some_and(|c| c.status == "applied_to_editor");
    let applying = message.changes.as_ref().is_some_and(|c| c.applying);
    let idea_count = message
        .comments
        .iter()
        .filter(|item| item["source"] == "idea")
        .count();
    let annotation_count = message.comments.len() - idea_count;
    let count = message.changes.as_ref().map_or(0, |c| c.count);
    rsx! {
        article {class:"{class}{streaming}","data-message":message.id,aria_label:if assistant {"Assistant message"} else {"Your message"},
            if annotation_count>0 && idea_count>0 {div {class:"message-context-summaries",SentContext {message:message.id.clone(),count:annotation_count} SentContext {message:message.id.clone(),count:idea_count,idea:true}}} else if annotation_count>0 {SentContext {message:message.id.clone(),count:annotation_count}} else if idea_count>0 {SentContext {message:message.id.clone(),count:idea_count,idea:true}}
            if assistant || !message.text.is_empty() || !message.skills.is_empty() || !message.attachments.is_empty() {
                div {class:if assistant {"chat-assistant-bubble"}else{"chat-user-bubble"},
                    if assistant || !message.text.is_empty() {div {class:if assistant {"chat-message-text chat-markdown"}else{"chat-message-text"},if assistant {crate::markdown::Markdown {text:message.text.clone(),root:root.clone()}}else {"{message.text}"}}}
                    if !message.skills.is_empty() || !message.attachments.is_empty() {
                        div {class:"chat-message-files",
                            for reference in message.skills { ReferenceChip {key:"skill:{reference.name}",reference,skill:true} }
                            for reference in message.attachments { ReferenceChip {key:"file:{reference.path}",reference,skill:false} }
                        }
                    }
                }
            }
            time {datetime:message.time,"{message.clock}"}
            if assistant {
                div {class:"chat-agent-actions",
                    if !message.response.is_empty() {span {class:"chat-response-state","{message.response}"}}
                    if count>0 {UiButton {label:format!("Review proposed changes ({count})"),icon:"edit",class:"chat-review-changes",text:count.to_string(),disabled:message.status=="streaming",onpress:move |_|action("review",&review_id,Value::Null)}}
                    if count>0 && message.mode!="full" {UiButton {label:"Apply to editor",icon:"check",class:"chat-compact-action",text:(if applying {"Applying…"}else if applied {"Applied"}else{"Apply to editor"}).to_string(),disabled:message.status=="streaming" || applying || applied,onpress:move |_|action("apply",&apply_id,Value::Null)}}
                }
            }
            if let Some(review)=review { ReviewPanel {review} }
            if !message.tools.is_empty() {
                details {class:"chat-tool-activity",
                    summary {Icon {name:"terminal"} span {"Activity"} span {class:"activity-count","{message.tools.len()}"} Icon {name:"chevron"}}
                    div {class:"activity-tools",
                        for tool in message.tools {
                            details {key:"{tool.id}","data-call":tool.id,
                                summary {span {{tool.tool.trim_start_matches("sde_").replace('_', " ")}} span {class:"tool-status","{tool.status}"}}
                                pre {"{tool.output}"}
                            }
                        }
                    }
                }
            }
            if !message.approvals.is_empty() {div {class:"chat-approvals",
                for approval in message.approvals { ApprovalCard {key:"{approval.approval_id}",approval} }
            }}
        }
    }
}
#[component]
fn ReferenceChip(reference: Reference, skill: bool) -> Element {
    let mut status = use_context::<crate::Status>();
    let hint = if skill {
        format!("Skill · {} · {}", reference.scope, reference.revision)
    } else {
        reference.path
    };
    rsx! {span {class:"chat-file",onmouseenter:move |_|status.hint.set(Some(hint.clone())),onmouseleave:move |_|status.hint.set(None),Icon {name:if skill {"skill"}else{"editor"}} span {"{reference.name}"}}}
}
#[component]
fn ApprovalCard(approval: Approval) -> Element {
    let mut status = use_context::<crate::Status>();
    let cwd = approval.cwd.clone();
    rsx! {
        section {class:"chat-command-approval",aria_label:"Command approval",
            div {class:"command-approval-heading",Icon {name:"terminal"} span {"Run command?"}}
            pre {"{approval.command}"}
            if !approval.reason.is_empty() {div {class:"command-approval-reason","{approval.reason}"}}
            div {class:"command-approval-scope",onmouseenter:move |_|status.hint.set(Some(cwd.clone())),onmouseleave:move |_|status.hint.set(None),"Working copy · {approval.timeout_seconds}s · Your user permissions"}
            div {class:"command-approval-actions",
                for (decision,label,icon) in [("decline","Decline","close"),("accept","Run once","check")] {
                    {let id=approval.approval_id.clone();let request=approval.request_id.clone();rsx! {
                        UiButton {label,icon,class:"chat-compact-action",text:label.to_string(),disabled:approval.submitted,onpress:move |_|action("approval",&request,json!({"id":id,"decision":decision}))}
                    }}
                }
            }
        }
    }
}
#[component]
fn ReviewPanel(review: Review) -> Element {
    let mut status = use_context::<crate::Status>();
    rsx! {
        section {class:"chat-inline-review",role:"region",aria_label:"Review proposed changes",
            onkeydown:move |event|{if event.key()==Key::Escape{event.stop_propagation();action("review-close","",Value::Null);}},
            header {strong {"Changes"} span {class:"review-count","{review.changes.len()}"} UiButton {label:"Close change review",icon:"close",class:"chat-review-close",onpress:move |_|action("review-close","",Value::Null)}}
            div {class:"change-review-files",
                for file in review.changes {
                    {let adds=file.lines.iter().filter(|line|line.kind=="add").count();let removes=file.lines.iter().filter(|line|line.kind=="remove").count();rsx! {
                        details {key:"{file.path}",class:"review-file",open:true,
                            summary {class:"change-review-name",Icon {name:"editor"} span {class:"review-path","{file.path}"} span {class:"review-line-count","+{adds} −{removes}"}
                                if file.operation=="create" {span {"New"}}
                                if file.operation=="delete" {span {"Delete"}}
                                if file.conflict {span {class:"change-review-conflict","Changed on disk"}}
                            }
                            pre {class:"change-review-diff",
                                for line in file.lines {
                                    div {class:"diff-{line.kind}",span {"{line.old_line.map(|n|n.to_string()).unwrap_or_default()}"} span {"{line.new_line.map(|n|n.to_string()).unwrap_or_default()}"} span {{format!("{} {}",if line.kind=="add" {"+"}else if line.kind=="remove" {"−"}else{" "},line.text)}}}
                                }
                            }
                        }
                    }}
                }
            }
            footer {
                if let Some(issue)=review.issue {
                    span {class:"review-issue",tabindex:0,aria_label:issue.clone(),onfocus:{let issue=issue.clone();move |_|status.hint.set(Some(issue.clone()))},onblur:move |_|status.hint.set(None),onmouseenter:move |_|status.hint.set(Some(issue.clone())),onmouseleave:move |_|status.hint.set(None),"Review required"}
                }
            }
        }
    }
}

#[component]
fn SentContext(message: String, count: usize, #[props(default)] idea: bool) -> Element {
    let text = if idea {
        format!("{count} Ideas/Things")
    } else {
        format!("{count} annotation{}", if count == 1 { "" } else { "s" })
    };
    let label = if idea {
        format!("View ideas ({count})")
    } else {
        format!("View comments ({count})")
    };
    let icon = if idea { "idea" } else { "comment" };
    rsx! {div {class:if idea {"annotation-summary-anchor sent-ideas"}else{"annotation-summary-anchor sent-annotations"},
        UiButton {label,icon,text:Some(text),class:"sent-context-badge",onpress:move |_|action("sent-context",&message,json!(idea))}
    }}
}
