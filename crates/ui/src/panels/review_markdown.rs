use gpui::{IntoElement, prelude::*, px, rems};
use gpui_component::{
    highlighter::LanguageRegistry,
    text::{TextView, TextViewStyle},
};

#[derive(Clone, Copy)]
struct MarkdownFence {
    marker: u8,
    length: usize,
}

pub(crate) fn render_review_markdown_body(id: impl Into<String>, body: &str) -> impl IntoElement {
    TextView::markdown(id.into(), review_markdown_body(body))
        .style(review_markdown_style())
        .selectable(true)
        .min_w_0()
        .flex_none()
}

pub(crate) fn review_markdown_body(body: &str) -> String {
    let body = body.trim();

    if body.is_empty() {
        "empty comment".to_string()
    } else {
        normalize_review_markdown(body)
    }
}

fn review_markdown_style() -> TextViewStyle {
    TextViewStyle::default()
        .paragraph_gap(rems(0.5))
        .heading_font_size(|level, _| match level {
            1 => px(15.0),
            2 => px(14.0),
            _ => px(13.0),
        })
}

fn normalize_review_markdown(markdown: &str) -> String {
    let mut normalized = String::with_capacity(markdown.len());
    let mut fence = None;

    for line in markdown.split_inclusive('\n') {
        let (line, newline) = line
            .strip_suffix('\n')
            .map_or((line, ""), |line| (line, "\n"));
        normalized.push_str(&normalize_review_markdown_line(line, &mut fence));
        normalized.push_str(newline);
    }

    normalized
}

fn normalize_review_markdown_line(line: &str, fence: &mut Option<MarkdownFence>) -> String {
    if let Some(active_fence) = fence {
        if closes_markdown_fence(line, *active_fence) {
            *fence = None;
        }
        return line.to_string();
    }

    if let Some((marker, length, info)) = markdown_fence_opening(line) {
        *fence = Some(MarkdownFence { marker, length });
        return normalize_markdown_fence_language(line, length, info);
    }

    normalize_inline_html(line)
}

fn markdown_fence_opening(line: &str) -> Option<(u8, usize, &str)> {
    let trimmed = line.trim_start();
    let marker = trimmed.as_bytes().first().copied()?;
    if marker != b'`' && marker != b'~' {
        return None;
    }

    let length = trimmed
        .as_bytes()
        .iter()
        .take_while(|character| **character == marker)
        .count();
    if length < 3 {
        return None;
    }

    Some((marker, length, &trimmed[length..]))
}

fn closes_markdown_fence(line: &str, fence: MarkdownFence) -> bool {
    let trimmed = line.trim_start();
    let length = trimmed
        .as_bytes()
        .iter()
        .take_while(|character| **character == fence.marker)
        .count();

    length >= fence.length && trimmed[length..].trim().is_empty()
}

fn normalize_markdown_fence_language(line: &str, length: usize, info: &str) -> String {
    let Some(language) = info.split_whitespace().next() else {
        return line.to_string();
    };
    if language.trim().is_empty() || markdown_language_registered(language) {
        return line.to_string();
    }

    let leading_whitespace = line.len() - line.trim_start().len();
    format!("{}text", &line[..leading_whitespace + length])
}

fn markdown_language_registered(language: &str) -> bool {
    let language = language
        .trim()
        .trim_start_matches('.')
        .strip_prefix("language-")
        .unwrap_or(language);

    LanguageRegistry::singleton().language(language).is_some()
}

fn normalize_inline_html(line: &str) -> String {
    strip_known_inline_html_tags(&rewrite_anchor_tags(line))
}

fn rewrite_anchor_tags(line: &str) -> String {
    let mut rest = line;
    let mut rewritten = String::with_capacity(line.len());

    while let Some(open_index) = find_ascii_case_insensitive(rest, "<a") {
        rewritten.push_str(&rest[..open_index]);
        let after_open = &rest[open_index..];
        let Some(open_end) = after_open.find('>') else {
            rewritten.push_str(after_open);
            return rewritten;
        };
        let tag = &after_open[..=open_end];
        if !is_opening_anchor_tag(tag) {
            rewritten.push_str(tag);
            rest = &after_open[open_end + 1..];
            continue;
        }
        let Some(close_index) = find_ascii_case_insensitive(&after_open[open_end + 1..], "</a>")
        else {
            rest = &after_open[open_end + 1..];
            continue;
        };
        let content_start = open_end + 1;
        let content_end = content_start + close_index;
        let content = &after_open[content_start..content_end];

        if let Some(href) = anchor_href(tag) {
            rewritten.push('[');
            rewritten.push_str(&escape_markdown_link_text(content));
            rewritten.push_str("](");
            rewritten.push_str(&href);
            rewritten.push(')');
        } else {
            rewritten.push_str(content);
        }

        rest = &after_open[content_end + "</a>".len()..];
    }

    rewritten.push_str(rest);
    rewritten
}

fn is_opening_anchor_tag(tag: &str) -> bool {
    let tag = tag.as_bytes();
    tag.len() >= 3
        && tag[0] == b'<'
        && tag[1].eq_ignore_ascii_case(&b'a')
        && tag[2].is_ascii_whitespace()
}

fn anchor_href(tag: &str) -> Option<String> {
    let href_index = find_ascii_case_insensitive(tag, "href")?;
    let after_href = tag[href_index + "href".len()..].trim_start();
    let after_equals = after_href.strip_prefix('=')?.trim_start();
    let quote = after_equals.as_bytes().first().copied()?;
    if quote != b'\'' && quote != b'"' {
        return None;
    }

    let href = &after_equals[1..after_equals[1..].find(quote as char)? + 1];
    Some(href.to_string())
}

fn escape_markdown_link_text(text: &str) -> String {
    text.replace('[', "\\[").replace(']', "\\]")
}

fn strip_known_inline_html_tags(line: &str) -> String {
    let mut rest = line;
    let mut stripped = String::with_capacity(line.len());

    while let Some(open_index) = rest.find('<') {
        stripped.push_str(&rest[..open_index]);
        let after_open = &rest[open_index..];
        let Some(close_index) = after_open.find('>') else {
            stripped.push_str(after_open);
            return stripped;
        };
        let tag = &after_open[..=close_index];
        if !is_known_inline_html_tag(tag) {
            stripped.push_str(tag);
        }
        rest = &after_open[close_index + 1..];
    }

    stripped.push_str(rest);
    stripped
}

fn is_known_inline_html_tag(tag: &str) -> bool {
    let tag = tag
        .trim_start_matches('<')
        .trim_end_matches('>')
        .trim()
        .trim_start_matches('/')
        .trim_start();
    let name = tag
        .split(|character: char| character.is_ascii_whitespace() || character == '/')
        .next()
        .unwrap_or_default();

    ["a", "sub", "sup"]
        .iter()
        .any(|known| name.eq_ignore_ascii_case(known))
}

fn find_ascii_case_insensitive(haystack: &str, needle: &str) -> Option<usize> {
    haystack
        .as_bytes()
        .windows(needle.len())
        .position(|window| window.eq_ignore_ascii_case(needle.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_common_review_markdown() {
        assert_eq!(
            review_markdown_body("**bold**\n\n- list item"),
            "**bold**\n\n- list item"
        );
        assert_eq!(review_markdown_body(" \n\t "), "empty comment");
    }

    #[test]
    fn normalizes_unregistered_code_fence_languages() {
        assert_eq!(
            review_markdown_body("```suggestion\nlet value = 1;\n```\n\n```mermaid\ngraph TD\n```"),
            "```text\nlet value = 1;\n```\n\n```text\ngraph TD\n```"
        );
    }

    #[test]
    fn keeps_registered_code_fence_languages() {
        assert_eq!(
            review_markdown_body("```rust\nlet value = 1;\n```"),
            "```rust\nlet value = 1;\n```"
        );
    }

    #[test]
    fn normalizes_common_inline_html() {
        assert_eq!(
            review_markdown_body(
                "<a href=\"https://example.com/rule\">rule</a> <sub>small print</sub>"
            ),
            "[rule](https://example.com/rule) small print"
        );
    }

    #[test]
    fn leaves_unknown_html_tags_for_text_view() {
        assert_eq!(
            review_markdown_body("<details><summary>note</summary></details>"),
            "<details><summary>note</summary></details>"
        );
    }
}
