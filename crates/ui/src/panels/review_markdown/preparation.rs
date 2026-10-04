use markdown::{ParseOptions, mdast::Node};
use std::ops::Range;

#[derive(Clone, Copy)]
struct MarkdownFence {
    marker: u8,
    length: usize,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) enum ReviewMarkdownSection {
    Markdown(String),
    Suggestion(String),
}

#[derive(Clone, Copy)]
struct MarkdownFenceBlock {
    start: usize,
    content_start: usize,
    content_end: usize,
    end: usize,
    is_suggestion: bool,
}

pub(crate) fn review_markdown_body(body: &str, language_registered: fn(&str) -> bool) -> String {
    let body = body.trim();

    if body.is_empty() {
        "empty comment".to_string()
    } else {
        normalize_review_markdown(body, language_registered)
    }
}

pub(crate) fn overview_markdown_body(body: &str, language_registered: fn(&str) -> bool) -> String {
    let body = review_markdown_body(body, language_registered);
    let mut normalized = String::with_capacity(body.len());
    let mut fence = None;

    for line in body.split_inclusive('\n') {
        let (line, newline) = line
            .strip_suffix('\n')
            .map_or((line, ""), |line| (line, "\n"));

        if let Some(active_fence) = fence {
            if closes_markdown_fence(line, active_fence) {
                fence = None;
            }
            normalized.push_str(line);
        } else if let Some((marker, length, _)) = markdown_fence_opening(line) {
            fence = Some(MarkdownFence { marker, length });
            normalized.push_str(line);
        } else {
            normalized.extend(line.chars().filter(|character| *character != '`'));
        }
        normalized.push_str(newline);
    }

    normalized
}

pub(crate) fn overview_markdown_blocks(
    body: &str,
    language_registered: fn(&str) -> bool,
) -> Vec<String> {
    let source = overview_markdown_body(body, language_registered);
    let root = match markdown::to_mdast(&source, &ParseOptions::gfm()) {
        Ok(Node::Root(root)) => root,
        Ok(_) => return vec![source],
        Err(error) => {
            tracing::warn!(%error, "failed to split pull request description markdown");
            return vec![source];
        }
    };
    let definitions = root
        .children
        .iter()
        .filter(|node| matches!(node, Node::Definition(_) | Node::FootnoteDefinition(_)))
        .filter_map(|node| markdown_node_source(&source, node))
        .collect::<Vec<_>>()
        .join("\n\n");
    let mut blocks = root
        .children
        .iter()
        .filter(|node| !matches!(node, Node::Definition(_) | Node::FootnoteDefinition(_)))
        .filter_map(|node| markdown_node_source(&source, node))
        .map(str::to_string)
        .collect::<Vec<_>>();

    if !definitions.is_empty() {
        for block in &mut blocks {
            block.push_str("\n\n");
            block.push_str(&definitions);
        }
    }

    if blocks.is_empty() && !source.is_empty() {
        blocks.push(source);
    }
    blocks
}

fn markdown_node_source<'a>(source: &'a str, node: &Node) -> Option<&'a str> {
    let position = node.position()?;
    let range = position.start.offset..position.end.offset;
    if range.start > range.end
        || range.end > source.len()
        || !source.is_char_boundary(range.start)
        || !source.is_char_boundary(range.end)
    {
        return None;
    }

    Some(source[range].trim())
}

pub(crate) fn review_markdown_has_suggestion(markdown: &str) -> bool {
    let mut search_start = 0;

    while let Some(fence) = next_markdown_fence_block(markdown, search_start) {
        if fence.is_suggestion {
            return true;
        }
        search_start = fence.end;
    }

    false
}

pub(super) fn review_markdown_sections(markdown: &str) -> Vec<ReviewMarkdownSection> {
    let markdown = markdown.trim();
    let mut sections = Vec::new();
    let mut markdown_start = 0;
    let mut search_start = 0;

    while let Some(fence) = next_markdown_fence_block(markdown, search_start) {
        if fence.is_suggestion {
            push_markdown_section(&mut sections, &markdown[markdown_start..fence.start]);
            let suggestion = &markdown[fence.content_start..fence.content_end];
            let suggestion = suggestion.strip_suffix('\n').unwrap_or(suggestion);
            let suggestion = suggestion.strip_suffix('\r').unwrap_or(suggestion);
            sections.push(ReviewMarkdownSection::Suggestion(suggestion.to_string()));
            markdown_start = fence.end;
        }
        search_start = fence.end;
    }

    push_markdown_section(&mut sections, &markdown[markdown_start..]);
    sections
}

fn push_markdown_section(sections: &mut Vec<ReviewMarkdownSection>, markdown: &str) {
    if !markdown.trim().is_empty() {
        sections.push(ReviewMarkdownSection::Markdown(markdown.to_string()));
    }
}

fn next_markdown_fence_block(
    markdown: &str,
    mut search_start: usize,
) -> Option<MarkdownFenceBlock> {
    while search_start < markdown.len() {
        let (line, next_line_start) = markdown_line(markdown, search_start);
        let Some((marker, length, info)) = markdown_fence_opening(line) else {
            search_start = next_line_start;
            continue;
        };
        let fence = MarkdownFence { marker, length };
        let mut close_start = next_line_start;

        while close_start < markdown.len() {
            let (close_line, close_end) = markdown_line(markdown, close_start);
            if closes_markdown_fence(close_line, fence) {
                return Some(MarkdownFenceBlock {
                    start: search_start,
                    content_start: next_line_start,
                    content_end: close_start,
                    end: close_end,
                    is_suggestion: info.trim() == "suggestion",
                });
            }
            close_start = close_end;
        }

        return None;
    }

    None
}

fn markdown_line(markdown: &str, start: usize) -> (&str, usize) {
    let remaining = &markdown[start..];
    if let Some(newline) = remaining.find('\n') {
        (&remaining[..newline], start + newline + 1)
    } else {
        (remaining, markdown.len())
    }
}

fn normalize_review_markdown(markdown: &str, language_registered: fn(&str) -> bool) -> String {
    let block_html_ranges = block_html_ranges(markdown);
    if block_html_ranges.is_empty() {
        return normalize_review_markdown_segment(markdown, language_registered);
    }

    let mut normalized = String::with_capacity(markdown.len());
    let mut segment_start = 0;

    for range in block_html_ranges {
        normalized.push_str(&normalize_review_markdown_segment(
            &markdown[segment_start..range.start],
            language_registered,
        ));
        normalized.push_str(&markdown[range.clone()]);
        segment_start = range.end;
    }
    normalized.push_str(&normalize_review_markdown_segment(
        &markdown[segment_start..],
        language_registered,
    ));
    normalized
}

fn normalize_review_markdown_segment(
    markdown: &str,
    language_registered: fn(&str) -> bool,
) -> String {
    let mut normalized = String::with_capacity(markdown.len());
    let mut fence = None;

    for line in markdown.split_inclusive('\n') {
        let (line, newline) = line
            .strip_suffix('\n')
            .map_or((line, ""), |line| (line, "\n"));
        normalized.push_str(&normalize_review_markdown_line(
            line,
            &mut fence,
            language_registered,
        ));
        normalized.push_str(newline);
    }

    normalized
}

fn block_html_ranges(markdown: &str) -> Vec<Range<usize>> {
    if !markdown.contains('<') {
        return Vec::new();
    }

    let root = match markdown::to_mdast(markdown, &ParseOptions::gfm()) {
        Ok(Node::Root(root)) => root,
        Ok(_) => return Vec::new(),
        Err(error) => {
            tracing::warn!(%error, "failed to parse review markdown before preserving html blocks");
            return Vec::new();
        }
    };
    let mut ranges = Vec::new();

    for node in root.children {
        let Node::Html(html) = node else {
            continue;
        };
        let Some(position) = html.position else {
            continue;
        };
        let range = position.start.offset..position.end.offset;
        if range.start <= range.end
            && range.end <= markdown.len()
            && markdown.is_char_boundary(range.start)
            && markdown.is_char_boundary(range.end)
        {
            ranges.push(range);
        }
    }

    ranges
}

fn normalize_review_markdown_line(
    line: &str,
    fence: &mut Option<MarkdownFence>,
    language_registered: fn(&str) -> bool,
) -> String {
    if let Some(active_fence) = fence {
        if closes_markdown_fence(line, *active_fence) {
            *fence = None;
        }
        return line.to_string();
    }

    if let Some((marker, length, info)) = markdown_fence_opening(line) {
        *fence = Some(MarkdownFence { marker, length });
        return normalize_markdown_fence_language(line, length, info, language_registered);
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

fn normalize_markdown_fence_language(
    line: &str,
    length: usize,
    info: &str,
    language_registered: fn(&str) -> bool,
) -> String {
    let Some(language) = info.split_whitespace().next() else {
        return line.to_string();
    };
    if language.trim().is_empty() || language_registered(language) {
        return line.to_string();
    }

    let leading_whitespace = line.len() - line.trim_start().len();
    format!("{}text", &line[..leading_whitespace + length])
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
    fn review_markdown_body(body: &str) -> String {
        super::review_markdown_body(body, |language| language == "rust")
    }
    fn overview_markdown_body(body: &str) -> String {
        super::overview_markdown_body(body, |language| language == "rust")
    }
    fn overview_markdown_blocks(body: &str) -> Vec<String> {
        super::overview_markdown_blocks(body, |language| language == "rust")
    }

    #[test]
    fn preserves_common_review_markdown() {
        assert_eq!(
            review_markdown_body("**bold**\n\n- list item"),
            "**bold**\n\n- list item"
        );
        assert_eq!(review_markdown_body(" \n\t "), "empty comment");
    }

    #[test]
    fn removes_inline_code_decoration_from_overview_markdown() {
        assert_eq!(
            overview_markdown_body("- `cargo fmt --all`\n\n```rust\nlet value = 1;\n```"),
            "- cargo fmt --all\n\n```rust\nlet value = 1;\n```"
        );
    }

    #[test]
    fn splits_overview_markdown_at_top_level_block_boundaries() {
        let blocks = overview_markdown_blocks(
            "## Summary\n\nFirst paragraph.\n\n- First item\n- Second item\n\nFinal paragraph.",
        );

        assert_eq!(
            blocks,
            vec![
                "## Summary",
                "First paragraph.",
                "- First item\n- Second item",
                "Final paragraph."
            ]
        );
    }

    #[test]
    fn keeps_reference_definitions_available_to_each_overview_block() {
        let blocks = overview_markdown_blocks(
            "Read the [guide][docs].\n\nMore context.\n\n[docs]: https://example.com/docs",
        );

        assert_eq!(blocks.len(), 2);
        assert!(blocks[0].ends_with("[docs]: https://example.com/docs"));
        assert!(blocks[1].ends_with("[docs]: https://example.com/docs"));
    }

    #[test]
    fn normalizes_unregistered_code_fence_languages() {
        assert_eq!(
            review_markdown_body("```suggestion\nlet value = 1;\n```\n\n```mermaid\ngraph TD\n```"),
            "```text\nlet value = 1;\n```\n\n```text\ngraph TD\n```"
        );
    }

    #[test]
    fn separates_suggested_changes_from_surrounding_markdown() {
        assert!(review_markdown_has_suggestion(
            "Use the localized helper:\n\n```suggestion\nTextField(.placeholder(\"Search places\"), text: $query)\n```"
        ));
        assert_eq!(
            review_markdown_sections(
                "Use the localized helper:\n\n```suggestion\nTextField(.placeholder(\"Search places\"), text: $query)\n```\n\nLooks good."
            ),
            vec![
                ReviewMarkdownSection::Markdown("Use the localized helper:\n\n".to_string()),
                ReviewMarkdownSection::Suggestion(
                    "TextField(.placeholder(\"Search places\"), text: $query)".to_string()
                ),
                ReviewMarkdownSection::Markdown("\nLooks good.".to_string()),
            ]
        );
    }

    #[test]
    fn supports_multiple_and_tilde_suggestion_fences() {
        assert_eq!(
            review_markdown_sections(
                "```suggestion\nfirst\n```\n\nbetween\n\n~~~suggestion\nsecond\n~~~"
            ),
            vec![
                ReviewMarkdownSection::Suggestion("first".to_string()),
                ReviewMarkdownSection::Markdown("\nbetween\n\n".to_string()),
                ReviewMarkdownSection::Suggestion("second".to_string()),
            ]
        );
    }

    #[test]
    fn ignores_suggestion_markers_inside_other_code_fences() {
        let markdown = "```text\n```suggestion\nnot a suggestion\n```\n```";

        assert!(!review_markdown_has_suggestion(markdown));
        assert_eq!(
            review_markdown_sections(markdown),
            vec![ReviewMarkdownSection::Markdown(markdown.to_string())]
        );
    }

    #[test]
    fn leaves_unclosed_suggestion_fences_as_markdown() {
        let markdown = "before\n\n```suggestion\nunfinished";

        assert!(!review_markdown_has_suggestion(markdown));
        assert_eq!(
            review_markdown_sections(markdown),
            vec![ReviewMarkdownSection::Markdown(markdown.to_string())]
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
    fn preserves_linked_picture_html_with_image_dimensions() {
        let body = concat!(
            "<div><a href=\"https://cursor.com/agents/run\"><picture>",
            "<source media=\"(prefers-color-scheme: dark)\" ",
            "srcset=\"https://cursor.com/open-dark.png\">",
            "<source media=\"(prefers-color-scheme: light)\" ",
            "srcset=\"https://cursor.com/open-light.png\">",
            "<img alt=\"Open in Web\" width=\"114\" height=\"28\" ",
            "src=\"https://cursor.com/open-dark.png\"></picture></a>&nbsp;",
            "<a href=\"https://cursor.com/automations/run\"><picture>",
            "<img alt=\"View Automation\" width=\"141\" height=\"28\" ",
            "src=\"https://cursor.com/automation-dark.png\"></picture></a>&nbsp;</div>",
        );

        let normalized = review_markdown_body(body);

        assert_eq!(normalized, body);
        assert!(normalized.contains("width=\"114\" height=\"28\""));
        assert!(normalized.contains("width=\"141\" height=\"28\""));
    }

    #[test]
    fn leaves_unknown_html_tags_for_text_view() {
        assert_eq!(
            review_markdown_body("<details><summary>note</summary></details>"),
            "<details><summary>note</summary></details>"
        );
    }
}
