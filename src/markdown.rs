use dioxus::prelude::*;
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag};
use serde_json::json;
use std::path::{Path, PathBuf};

fn safe_link(url: &str) -> bool {
    ["https://", "http://", "mailto:"]
        .iter()
        .any(|scheme| url.to_ascii_lowercase().starts_with(scheme))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SourceRange {
    start: usize,
    end: usize,
}

fn decode_path(path: &str) -> Option<String> {
    let bytes = path.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let high = *bytes.get(index + 1)?;
            let low = *bytes.get(index + 2)?;
            let digit = |value: u8| match value {
                b'0'..=b'9' => Some(value - b'0'),
                b'a'..=b'f' => Some(value - b'a' + 10),
                b'A'..=b'F' => Some(value - b'A' + 10),
                _ => None,
            };
            decoded.push(digit(high)? * 16 + digit(low)?);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).ok()
}

fn parse_source_link(url: &str) -> Option<(String, Option<SourceRange>)> {
    let destination = url.strip_prefix("dado-file:")?;
    let (encoded_path, fragment) = destination.split_once('#').unwrap_or((destination, ""));
    let path = decode_path(encoded_path)?;
    if path.is_empty() || path.starts_with('/') || path.contains('\\') || path.contains('?') {
        return None;
    }
    if Path::new(&path).components().any(|component| {
        matches!(
            component,
            std::path::Component::Prefix(_)
                | std::path::Component::RootDir
                | std::path::Component::ParentDir
        )
    }) {
        return None;
    }
    let range = if fragment.is_empty() {
        None
    } else {
        let lines = fragment.strip_prefix('L')?;
        let (start, end) = lines
            .split_once("-L")
            .or_else(|| lines.split_once('-'))
            .map_or((lines, lines), |(start, end)| (start, end));
        let start = start.parse::<usize>().ok()?;
        let end = end.parse::<usize>().ok()?;
        if start == 0 || end < start {
            return None;
        }
        Some(SourceRange { start, end })
    };
    Some((path, range))
}

fn resolve_source(root: &Path, path: &str) -> Result<PathBuf, String> {
    crate::project_model::source_path(
        root,
        &crate::project_model::Source {
            path: path.to_string(),
            kind: "file".into(),
            symbol: None,
            start: None,
            end: None,
        },
    )
}

// Render parser events as Dioxus nodes; raw HTML remains literal text.
fn nodes<'a>(
    events: &mut impl Iterator<Item = Event<'a>>,
    root: Option<PathBuf>,
    mut workbench: crate::Workbench,
    mut status: crate::Status,
) -> Element {
    let mut children = Vec::new();
    while let Some(event) = events.next() {
        let child = match event {
            Event::End(_) => break,
            Event::Start(tag) => {
                let content = nodes(events, root.clone(), workbench, status);
                match tag {
                    Tag::Paragraph => rsx! { p { {content} } },
                    Tag::Heading { level, .. } => match level {
                        HeadingLevel::H1 => rsx! { h1 {{content}} },
                        HeadingLevel::H2 => rsx! { h2 {{content}} },
                        HeadingLevel::H3 => rsx! { h3 {{content}} },
                        HeadingLevel::H4 => rsx! { h4 {{content}} },
                        HeadingLevel::H5 => rsx! { h5 {{content}} },
                        HeadingLevel::H6 => rsx! { h6 {{content}} },
                    },
                    Tag::Strong => rsx! { strong {{content}} },
                    Tag::Emphasis => rsx! { em {{content}} },
                    Tag::Strikethrough => rsx! { del {{content}} },
                    Tag::BlockQuote(_) => rsx! { blockquote {{content}} },
                    Tag::CodeBlock(_) => rsx! { pre { code {{content}} } },
                    Tag::List(Some(start)) => rsx! { ol { start:start as i64, {content} } },
                    Tag::List(None) => rsx! { ul {{content}} },
                    Tag::Item => rsx! { li {{content}} },
                    Tag::Table(_) => rsx! { div {class:"markdown-table",table {{content}}} },
                    Tag::TableHead => rsx! { thead {tr {{content}}} },
                    Tag::TableRow => rsx! { tr {{content}} },
                    Tag::TableCell => rsx! { td {{content}} },
                    Tag::Link { dest_url, .. } if safe_link(&dest_url) => {
                        let url = dest_url.to_string();
                        rsx! { a {href:url.clone(),onclick:move |e|{e.prevent_default();let _=open::that(&url);},{content}} }
                    }
                    Tag::Link { dest_url, .. }
                        if let Some((relative, range)) = parse_source_link(&dest_url)
                            && let Some(root) = root.as_deref()
                            && let Ok(path) = resolve_source(root, &relative) =>
                    {
                        let range = range.map(|range| json!({"start":range.start,"end":range.end}));
                        let label = relative;
                        rsx! {
                            a {
                                href: "#",
                                class: "chat-source-link",
                                aria_label: if let Some(range) = range.as_ref() {
                                    format!("Open {} lines {}–{} in editor", label, range["start"], range["end"])
                                } else {
                                    format!("Open {} in editor", label)
                                },
                                onclick: move |event| {
                                    event.prevent_default();
                                    workbench.shown.set(true);
                                    workbench.selected_file.set(Some(path.clone()));
                                    let _ = document::eval(&format!(
                                        "window.dadoWorkbench?.openFile({},{});",
                                        json!(path),
                                        range.clone().unwrap_or(serde_json::Value::Null)
                                    ));
                                    status.hint.set(Some(format!("Opening {label}")));
                                },
                                {content}
                            }
                        }
                    }
                    _ => content,
                }
            }
            Event::Code(text) => rsx! {code {"{text}"}},
            Event::Text(text) | Event::Html(text) | Event::InlineHtml(text) => rsx! {"{text}"},
            Event::SoftBreak => rsx! {"\n"},
            Event::HardBreak => rsx! {br {}},
            Event::Rule => rsx! {hr {}},
            Event::TaskListMarker(checked) => {
                rsx! {span {class:"markdown-task","aria-label":if checked {"Completed"}else{"Incomplete"},{if checked {"☑ "}else{"☐ "}}}}
            }
            _ => continue,
        };
        children.push(child);
    }
    rsx! {for child in children {{child}}}
}

#[component]
pub fn Markdown(text: String, root: Option<PathBuf>) -> Element {
    let workbench = use_context::<crate::Workbench>();
    let status = use_context::<crate::Status>();
    nodes(
        &mut Parser::new_ext(
            &text,
            Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS,
        ),
        root,
        workbench,
        status,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn links_only_open_web_and_mail_destinations() {
        for url in [
            "https://example.com",
            "http://localhost",
            "mailto:hello@example.com",
        ] {
            assert!(safe_link(url));
        }
        for url in [
            "javascript:alert(1)",
            "file:///C:/secret",
            "data:text/html,hello",
            "./relative",
        ] {
            assert!(!safe_link(url));
        }
    }

    #[test]
    fn parses_project_file_links_and_inclusive_ranges() {
        assert_eq!(
            parse_source_link("dado-file:src/ui_controls.rs#L12-L24"),
            Some((
                "src/ui_controls.rs".into(),
                Some(SourceRange { start: 12, end: 24 })
            ))
        );
        assert_eq!(
            parse_source_link("dado-file:src/ui%20controls.rs#L3"),
            Some((
                "src/ui controls.rs".into(),
                Some(SourceRange { start: 3, end: 3 })
            ))
        );
        assert_eq!(
            parse_source_link("dado-file:src/main.rs"),
            Some(("src/main.rs".into(), None))
        );
    }

    #[test]
    fn rejects_malformed_or_unsafe_project_file_links() {
        for url in [
            "dado-file:../outside.rs#L1-L2",
            "dado-file:/absolute.rs",
            "dado-file:C:/outside.rs",
            "dado-file:src\\main.rs",
            "dado-file:src/main.rs#L0-L2",
            "dado-file:src/main.rs#L5-L2",
            "dado-file:src/main.rs#Lx",
            "dado-file:src/main.rs#invalid",
            "dado-file:%GG",
        ] {
            assert!(parse_source_link(url).is_none(), "accepted {url}");
        }
    }

    #[test]
    fn project_file_resolution_rejects_escape_and_generated_storage() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("src")).unwrap();
        std::fs::write(root.path().join("src/main.rs"), "fn main() {}\n").unwrap();
        std::fs::create_dir_all(root.path().join(".dado")).unwrap();
        std::fs::write(root.path().join(".dado/private.json"), "{}").unwrap();
        assert!(resolve_source(root.path(), "src/main.rs").is_ok());
        assert!(resolve_source(root.path(), "../outside.rs").is_err());
        assert!(resolve_source(root.path(), ".dado/private.json").is_err());
    }
}
