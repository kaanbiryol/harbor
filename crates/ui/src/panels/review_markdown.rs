use std::{cell::RefCell, ops::Range};

use gpui::{
    AnyElement, FontStyle, FontWeight, HighlightStyle, InteractiveText, IntoElement, SharedString,
    StrikethroughStyle, StyledText, UnderlineStyle, div, prelude::*, px,
};
use gpui_component::highlighter::LanguageRegistry;
use markdown::{ParseOptions, mdast};

use crate::visual::{color, font};

#[derive(Clone, Copy)]
struct MarkdownFence {
    marker: u8,
    length: usize,
}

const REVIEW_MARKDOWN_BLOCK_CACHE_LIMIT: usize = 256;

thread_local! {
    static REVIEW_MARKDOWN_BLOCK_CACHE: RefCell<Vec<ReviewMarkdownCacheEntry>> = const { RefCell::new(Vec::new()) };
}

#[derive(Clone)]
struct ReviewMarkdownCacheEntry {
    markdown: String,
    blocks: Vec<ReviewMarkdownBlock>,
}

pub(crate) fn render_review_markdown_body(id: impl Into<String>, body: &str) -> impl IntoElement {
    let id = id.into();
    let blocks = cached_review_markdown_blocks(review_markdown_body(body));

    div()
        .id(id.clone())
        .min_w_0()
        .flex_none()
        .flex()
        .flex_col()
        .gap_2()
        .children(
            blocks
                .into_iter()
                .enumerate()
                .map(|(index, block)| render_review_markdown_block(&id, index, block)),
        )
}

pub(crate) fn review_markdown_body(body: &str) -> String {
    let body = body.trim();

    if body.is_empty() {
        "empty comment".to_string()
    } else {
        normalize_review_markdown(body)
    }
}

fn cached_review_markdown_blocks(markdown: String) -> Vec<ReviewMarkdownBlock> {
    REVIEW_MARKDOWN_BLOCK_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();

        if let Some(index) = cache
            .iter()
            .position(|entry| entry.markdown.as_str() == markdown.as_str())
        {
            let entry = cache.remove(index);
            let blocks = entry.blocks.clone();
            cache.push(entry);
            return blocks;
        }

        let blocks = parse_review_markdown_blocks(&markdown);
        cache.push(ReviewMarkdownCacheEntry {
            markdown,
            blocks: blocks.clone(),
        });
        if cache.len() > REVIEW_MARKDOWN_BLOCK_CACHE_LIMIT {
            let overflow = cache.len() - REVIEW_MARKDOWN_BLOCK_CACHE_LIMIT;
            cache.drain(0..overflow);
        }

        blocks
    })
}

#[derive(Clone, Debug, Default, PartialEq)]
struct ReviewInlineText {
    text: String,
    highlights: Vec<(Range<usize>, HighlightStyle)>,
    font_family_overrides: Vec<(Range<usize>, SharedString)>,
    links: Vec<(Range<usize>, String)>,
}

#[derive(Clone, Debug, PartialEq)]
struct ReviewListItem {
    marker: String,
    text: ReviewInlineText,
}

#[derive(Clone, Debug, PartialEq)]
enum ReviewMarkdownBlock {
    Heading {
        level: usize,
        text: ReviewInlineText,
    },
    Paragraph(ReviewInlineText),
    List(Vec<ReviewListItem>),
    Quote(Vec<ReviewInlineText>),
    Table(Vec<Vec<ReviewInlineText>>),
    Code(Vec<String>),
    Rule,
}

fn parse_review_markdown_blocks(markdown: &str) -> Vec<ReviewMarkdownBlock> {
    let root = match markdown::to_mdast(markdown, &ParseOptions::gfm()) {
        Ok(mdast::Node::Root(root)) => root,
        Ok(node) => {
            return vec![ReviewMarkdownBlock::Paragraph(inline_from_nodes(&[node]))];
        }
        Err(error) => {
            tracing::warn!(error = %error, "failed to parse review markdown as gfm");
            return vec![ReviewMarkdownBlock::Paragraph(plain_inline_text(markdown))];
        }
    };

    let blocks = blocks_from_nodes(&root.children);

    if blocks.is_empty() {
        vec![ReviewMarkdownBlock::Paragraph(plain_inline_text(" "))]
    } else {
        blocks
    }
}

fn blocks_from_nodes(nodes: &[mdast::Node]) -> Vec<ReviewMarkdownBlock> {
    let mut blocks = Vec::new();

    for node in nodes {
        push_block_from_node(node, &mut blocks);
    }

    blocks
}

fn push_block_from_node(node: &mdast::Node, blocks: &mut Vec<ReviewMarkdownBlock>) {
    match node {
        mdast::Node::Root(root) => blocks.extend(blocks_from_nodes(&root.children)),
        mdast::Node::Paragraph(paragraph) => {
            blocks.push(ReviewMarkdownBlock::Paragraph(inline_from_nodes(
                &paragraph.children,
            )));
        }
        mdast::Node::Heading(heading) => {
            blocks.push(ReviewMarkdownBlock::Heading {
                level: heading.depth as usize,
                text: inline_from_nodes(&heading.children),
            });
        }
        mdast::Node::List(list) => {
            let items = list
                .children
                .iter()
                .enumerate()
                .filter_map(|(index, child)| match child {
                    mdast::Node::ListItem(item) => Some(ReviewListItem {
                        marker: list_item_marker(list, item, index),
                        text: inline_from_flow_nodes(&item.children),
                    }),
                    _ => None,
                })
                .collect::<Vec<_>>();

            if !items.is_empty() {
                blocks.push(ReviewMarkdownBlock::List(items));
            }
        }
        mdast::Node::Blockquote(blockquote) => {
            let lines = blocks_from_nodes(&blockquote.children)
                .into_iter()
                .flat_map(quote_lines_from_block)
                .collect::<Vec<_>>();

            if !lines.is_empty() {
                blocks.push(ReviewMarkdownBlock::Quote(lines));
            }
        }
        mdast::Node::Table(table) => {
            let rows = table
                .children
                .iter()
                .filter_map(|child| match child {
                    mdast::Node::TableRow(row) => Some(
                        row.children
                            .iter()
                            .filter_map(|cell| match cell {
                                mdast::Node::TableCell(cell) => {
                                    Some(inline_from_nodes(&cell.children))
                                }
                                _ => None,
                            })
                            .collect::<Vec<_>>(),
                    ),
                    _ => None,
                })
                .filter(|row| !row.is_empty())
                .collect::<Vec<_>>();

            if !rows.is_empty() {
                blocks.push(ReviewMarkdownBlock::Table(rows));
            }
        }
        mdast::Node::Code(code) => {
            blocks.push(ReviewMarkdownBlock::Code(code_block_lines(&code.value)))
        }
        mdast::Node::Math(math) => {
            blocks.push(ReviewMarkdownBlock::Code(code_block_lines(&math.value)))
        }
        mdast::Node::ThematicBreak(_) => blocks.push(ReviewMarkdownBlock::Rule),
        mdast::Node::FootnoteDefinition(definition) => {
            let mut text = plain_inline_text(&format!("[{}]: ", definition.identifier));
            append_inline_text(&mut text, inline_from_flow_nodes(&definition.children));
            blocks.push(ReviewMarkdownBlock::Paragraph(text));
        }
        mdast::Node::Definition(_) => {}
        mdast::Node::Html(html) => {
            let value = stripped_html_text(&html.value);
            if !value.trim().is_empty() {
                blocks.push(ReviewMarkdownBlock::Paragraph(plain_inline_text(&value)));
            }
        }
        _ => {
            let text = inline_from_nodes(std::slice::from_ref(node));
            if !text.text.trim().is_empty() {
                blocks.push(ReviewMarkdownBlock::Paragraph(text));
            }
        }
    }
}

fn quote_lines_from_block(block: ReviewMarkdownBlock) -> Vec<ReviewInlineText> {
    match block {
        ReviewMarkdownBlock::Heading { text, .. } | ReviewMarkdownBlock::Paragraph(text) => {
            vec![text]
        }
        ReviewMarkdownBlock::List(items) => items
            .into_iter()
            .map(|item| {
                let mut text = plain_inline_text(&format!("{} ", item.marker));
                append_inline_text(&mut text, item.text);
                text
            })
            .collect(),
        ReviewMarkdownBlock::Quote(lines) => lines,
        ReviewMarkdownBlock::Table(rows) => rows
            .into_iter()
            .map(|row| join_inline_texts(row, " | "))
            .collect(),
        ReviewMarkdownBlock::Code(lines) => vec![plain_inline_text(&lines.join("\n"))],
        ReviewMarkdownBlock::Rule => vec![plain_inline_text("---")],
    }
}

fn list_item_marker(list: &mdast::List, item: &mdast::ListItem, index: usize) -> String {
    if let Some(checked) = item.checked {
        if checked {
            "[x]".to_string()
        } else {
            "[ ]".to_string()
        }
    } else if list.ordered {
        format!("{}.", list.start.unwrap_or(1) + index as u32)
    } else {
        "-".to_string()
    }
}

fn code_block_lines(value: &str) -> Vec<String> {
    let lines = value.lines().map(str::to_string).collect::<Vec<_>>();

    if lines.is_empty() {
        vec![String::new()]
    } else {
        lines
    }
}

fn inline_from_flow_nodes(nodes: &[mdast::Node]) -> ReviewInlineText {
    let mut inline_text = ReviewInlineText::default();

    for (index, node) in nodes.iter().enumerate() {
        if index > 0 && !inline_text.text.ends_with('\n') {
            push_inline_segment(&mut inline_text, " ", InlineStyle::default());
        }

        match node {
            mdast::Node::Paragraph(paragraph) => {
                push_inline_nodes(
                    &mut inline_text,
                    &paragraph.children,
                    InlineStyle::default(),
                );
            }
            mdast::Node::Heading(heading) => {
                push_inline_nodes(&mut inline_text, &heading.children, InlineStyle::default());
            }
            mdast::Node::List(list) => {
                for (item_index, child) in list.children.iter().enumerate() {
                    if let mdast::Node::ListItem(item) = child {
                        if !inline_text.text.is_empty() {
                            push_inline_segment(&mut inline_text, " ", InlineStyle::default());
                        }
                        push_inline_segment(
                            &mut inline_text,
                            &format!("{} ", list_item_marker(list, item, item_index)),
                            InlineStyle::default(),
                        );
                        append_inline_text(
                            &mut inline_text,
                            inline_from_flow_nodes(&item.children),
                        );
                    }
                }
            }
            mdast::Node::Code(code) => {
                let style = InlineStyle {
                    code: true,
                    ..Default::default()
                };
                push_inline_segment(&mut inline_text, &code.value, style);
            }
            mdast::Node::Math(math) => {
                let style = InlineStyle {
                    code: true,
                    ..Default::default()
                };
                push_inline_segment(&mut inline_text, &math.value, style);
            }
            mdast::Node::Blockquote(blockquote) => {
                append_inline_text(
                    &mut inline_text,
                    inline_from_flow_nodes(&blockquote.children),
                );
            }
            mdast::Node::Table(table) => {
                let row_text = table
                    .children
                    .iter()
                    .filter_map(|child| match child {
                        mdast::Node::TableRow(row) => Some(
                            row.children
                                .iter()
                                .filter_map(|cell| match cell {
                                    mdast::Node::TableCell(cell) => {
                                        Some(inline_from_nodes(&cell.children))
                                    }
                                    _ => None,
                                })
                                .collect::<Vec<_>>(),
                        ),
                        _ => None,
                    })
                    .map(|row| join_inline_texts(row, " | "))
                    .collect::<Vec<_>>();
                append_inline_text(&mut inline_text, join_inline_texts(row_text, " "));
            }
            mdast::Node::ThematicBreak(_) => {
                push_inline_segment(&mut inline_text, "---", InlineStyle::default());
            }
            mdast::Node::Html(html) => {
                push_inline_segment(
                    &mut inline_text,
                    &stripped_html_text(&html.value),
                    InlineStyle::default(),
                );
            }
            _ => push_inline_node(&mut inline_text, node, InlineStyle::default()),
        }
    }

    if inline_text.text.is_empty() {
        inline_text.text.push(' ');
    }

    inline_text
}

fn inline_from_nodes(nodes: &[mdast::Node]) -> ReviewInlineText {
    let mut inline_text = ReviewInlineText::default();
    push_inline_nodes(&mut inline_text, nodes, InlineStyle::default());

    if inline_text.text.is_empty() {
        inline_text.text.push(' ');
    }

    inline_text
}

fn push_inline_nodes(
    inline_text: &mut ReviewInlineText,
    nodes: &[mdast::Node],
    style: InlineStyle,
) {
    for node in nodes {
        push_inline_node(inline_text, node, style.clone());
    }
}

fn push_inline_node(inline_text: &mut ReviewInlineText, node: &mdast::Node, style: InlineStyle) {
    match node {
        mdast::Node::Text(text) => push_inline_segment(inline_text, &text.value, style),
        mdast::Node::Break(_) => push_inline_segment(inline_text, "\n", style),
        mdast::Node::InlineCode(code) => {
            let mut style = style;
            style.code = true;
            push_inline_segment(inline_text, &code.value, style);
        }
        mdast::Node::InlineMath(math) => {
            let mut style = style;
            style.code = true;
            push_inline_segment(inline_text, &math.value, style);
        }
        mdast::Node::Strong(strong) => {
            let mut style = style;
            style.bold = true;
            push_inline_nodes(inline_text, &strong.children, style);
        }
        mdast::Node::Emphasis(emphasis) => {
            let mut style = style;
            style.italic = true;
            push_inline_nodes(inline_text, &emphasis.children, style);
        }
        mdast::Node::Delete(delete) => {
            let mut style = style;
            style.strikethrough = true;
            push_inline_nodes(inline_text, &delete.children, style);
        }
        mdast::Node::Link(link) => {
            let mut style = style;
            style.link = clickable_link_url(&link.url);
            push_inline_nodes(inline_text, &link.children, style);
        }
        mdast::Node::LinkReference(link) => {
            let mut style = style;
            style.link = None;
            push_inline_nodes(inline_text, &link.children, style);
        }
        mdast::Node::Image(image) => {
            let label = if image.alt.is_empty() {
                image.url.as_str()
            } else {
                image.alt.as_str()
            };
            push_inline_segment(inline_text, label, style);
        }
        mdast::Node::ImageReference(image) => {
            let label = if image.alt.is_empty() {
                image.identifier.as_str()
            } else {
                image.alt.as_str()
            };
            push_inline_segment(inline_text, label, style);
        }
        mdast::Node::FootnoteReference(reference) => {
            let mut style = style;
            style.italic = true;
            push_inline_segment(inline_text, &format!("[{}]", reference.identifier), style);
        }
        mdast::Node::Html(html) => {
            push_inline_segment(inline_text, &stripped_html_text(&html.value), style);
        }
        mdast::Node::MdxTextExpression(expression) => {
            push_inline_segment(inline_text, &expression.value, style);
        }
        mdast::Node::MdxJsxTextElement(element) => {
            push_inline_nodes(inline_text, &element.children, style);
        }
        _ => {
            if let Some(children) = node.children() {
                push_inline_nodes(inline_text, children, style);
            } else {
                let text = node.to_string();
                if !text.is_empty() {
                    push_inline_segment(inline_text, &text, style);
                }
            }
        }
    }
}

fn render_review_markdown_block(
    id_prefix: &str,
    block_index: usize,
    block: ReviewMarkdownBlock,
) -> AnyElement {
    match block {
        ReviewMarkdownBlock::Heading { level, text } => {
            let heading =
                render_inline_text(inline_text_id(id_prefix, block_index, "heading"), text);
            heading
                .font_weight(FontWeight::SEMIBOLD)
                .text_size(match level {
                    1 => px(15.0),
                    2 => px(14.0),
                    _ => px(13.0),
                })
                .into_any_element()
        }
        ReviewMarkdownBlock::Paragraph(text) => {
            render_inline_text(inline_text_id(id_prefix, block_index, "paragraph"), text)
                .into_any_element()
        }
        ReviewMarkdownBlock::List(items) => div()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_1()
            .children(items.into_iter().enumerate().map(|(item_index, item)| {
                render_review_list_item(id_prefix, block_index, item_index, item)
            }))
            .into_any_element(),
        ReviewMarkdownBlock::Quote(lines) => div()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_1()
            .border_l_2()
            .border_color(color::border_strong())
            .pl_2()
            .text_color(color::text_muted())
            .children(lines.into_iter().enumerate().map(|(line_index, line)| {
                render_inline_text(
                    inline_text_id(id_prefix, block_index, &format!("quote-{line_index}")),
                    line,
                )
                .into_any_element()
            }))
            .into_any_element(),
        ReviewMarkdownBlock::Table(rows) => {
            render_table(id_prefix, block_index, rows).into_any_element()
        }
        ReviewMarkdownBlock::Code(lines) => render_code_block(lines).into_any_element(),
        ReviewMarkdownBlock::Rule => div()
            .min_w_0()
            .h(px(1.0))
            .bg(color::border_subtle())
            .into_any_element(),
    }
}

fn render_inline_text(id: String, inline_text: ReviewInlineText) -> gpui::Div {
    let ReviewInlineText {
        text,
        highlights,
        font_family_overrides,
        links,
    } = inline_text;
    let styled_text = StyledText::new(text)
        .with_highlights(highlights)
        .with_font_family_overrides(font_family_overrides);
    let text_element = if links.is_empty() {
        styled_text.into_any_element()
    } else {
        let ranges = links
            .iter()
            .map(|(range, _)| range.clone())
            .collect::<Vec<_>>();
        let urls = links.into_iter().map(|(_, url)| url).collect::<Vec<_>>();

        InteractiveText::new(id, styled_text)
            .on_click(ranges, move |index, _, cx| {
                if let Some(url) = urls.get(index) {
                    cx.open_url(url);
                }
            })
            .into_any_element()
    };

    div()
        .min_w_0()
        .flex_none()
        .whitespace_normal()
        .child(text_element)
}

fn render_review_list_item(
    id_prefix: &str,
    block_index: usize,
    item_index: usize,
    item: ReviewListItem,
) -> AnyElement {
    div()
        .min_w_0()
        .flex()
        .items_start()
        .gap_2()
        .child(
            div()
                .flex_none()
                .w(px(32.0))
                .text_right()
                .text_color(color::text_muted())
                .child(item.marker),
        )
        .child(
            render_inline_text(
                inline_text_id(id_prefix, block_index, &format!("list-{item_index}")),
                item.text,
            )
            .flex_1(),
        )
        .into_any_element()
}

fn render_table(
    id_prefix: &str,
    block_index: usize,
    rows: Vec<Vec<ReviewInlineText>>,
) -> impl IntoElement {
    div()
        .min_w_0()
        .flex_none()
        .overflow_hidden()
        .border_1()
        .border_color(color::border_subtle())
        .children(rows.into_iter().enumerate().map(|(row_index, row)| {
            div()
                .min_w_0()
                .flex()
                .when(row_index > 0, |element| {
                    element.border_t_1().border_color(color::border_subtle())
                })
                .children(row.into_iter().enumerate().map(move |(cell_index, cell)| {
                    render_table_cell(id_prefix, block_index, row_index, cell_index, cell)
                }))
        }))
}

fn render_table_cell(
    id_prefix: &str,
    block_index: usize,
    row_index: usize,
    cell_index: usize,
    cell: ReviewInlineText,
) -> AnyElement {
    div()
        .min_w_0()
        .flex_1()
        .px_2()
        .py_1()
        .child(render_inline_text(
            inline_text_id(
                id_prefix,
                block_index,
                &format!("table-{row_index}-{cell_index}"),
            ),
            cell,
        ))
        .into_any_element()
}

fn render_code_block(lines: Vec<String>) -> impl IntoElement {
    div()
        .min_w_0()
        .flex_none()
        .overflow_hidden()
        .bg(color::input_background())
        .border_1()
        .border_color(color::border_subtle())
        .px_2()
        .py_2()
        .font_family(font::MONO)
        .text_size(px(11.0))
        .text_color(color::text_secondary())
        .children(lines.into_iter().map(|line| {
            div()
                .min_w_0()
                .whitespace_normal()
                .child(if line.is_empty() { " " } else { line.as_str() }.to_string())
        }))
}

fn inline_text_id(id_prefix: &str, block_index: usize, suffix: &str) -> String {
    format!("{id_prefix}-inline-{block_index}-{suffix}")
}

#[derive(Clone, Default)]
struct InlineStyle {
    bold: bool,
    italic: bool,
    strikethrough: bool,
    code: bool,
    link: Option<String>,
}

fn push_inline_segment(inline_text: &mut ReviewInlineText, text: &str, style: InlineStyle) {
    let start = inline_text.text.len();
    inline_text.text.push_str(text);
    let end = inline_text.text.len();

    if start == end {
        return;
    }

    let highlight = style.highlight();
    if highlight != HighlightStyle::default() {
        inline_text.highlights.push((start..end, highlight));
    }
    if style.code {
        inline_text
            .font_family_overrides
            .push((start..end, SharedString::from(font::MONO.to_string())));
    }
    if let Some(url) = style.link {
        inline_text.links.push((start..end, url));
    }
}

fn plain_inline_text(text: &str) -> ReviewInlineText {
    let mut inline_text = ReviewInlineText::default();
    push_inline_segment(&mut inline_text, text, InlineStyle::default());
    inline_text
}

fn append_inline_text(target: &mut ReviewInlineText, source: ReviewInlineText) {
    let offset = target.text.len();
    target.text.push_str(&source.text);
    target.highlights.extend(
        source
            .highlights
            .into_iter()
            .map(|(range, highlight)| (range.start + offset..range.end + offset, highlight)),
    );
    target.font_family_overrides.extend(
        source
            .font_family_overrides
            .into_iter()
            .map(|(range, family)| (range.start + offset..range.end + offset, family)),
    );
    target.links.extend(
        source
            .links
            .into_iter()
            .map(|(range, url)| (range.start + offset..range.end + offset, url)),
    );
}

fn join_inline_texts(texts: Vec<ReviewInlineText>, separator: &str) -> ReviewInlineText {
    let mut joined = ReviewInlineText::default();

    for (index, text) in texts.into_iter().enumerate() {
        if index > 0 {
            push_inline_segment(&mut joined, separator, InlineStyle::default());
        }
        append_inline_text(&mut joined, text);
    }

    joined
}

fn stripped_html_text(html: &str) -> String {
    if html.eq_ignore_ascii_case("<br>")
        || html.eq_ignore_ascii_case("<br/>")
        || html.eq_ignore_ascii_case("<br />")
    {
        "\n".to_string()
    } else {
        strip_known_inline_html_tags(html)
    }
}

impl InlineStyle {
    fn highlight(&self) -> HighlightStyle {
        HighlightStyle {
            font_weight: self.bold.then_some(FontWeight::BOLD),
            font_style: self.italic.then_some(FontStyle::Italic),
            underline: self.link.is_some().then_some(UnderlineStyle {
                thickness: px(1.0),
                ..Default::default()
            }),
            strikethrough: self.strikethrough.then_some(StrikethroughStyle {
                thickness: px(1.0),
                ..Default::default()
            }),
            ..Default::default()
        }
    }
}

fn clickable_link_url(url: &str) -> Option<String> {
    let url = url.trim();
    let (scheme, _) = url.split_once(':')?;
    if ["https", "http", "mailto"]
        .iter()
        .any(|allowed_scheme| scheme.eq_ignore_ascii_case(allowed_scheme))
    {
        Some(url.to_string())
    } else {
        None
    }
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
    fn parses_common_review_markdown_blocks() {
        let blocks = parse_review_markdown_blocks(
            "# Title\n\nParagraph with **bold** and `code` [rule](https://example.com).\n\n- item one\n- item two\n\n> quoted\n\n```text\nlet value = 1;\n```",
        );

        assert_eq!(blocks.len(), 5);
        match &blocks[0] {
            ReviewMarkdownBlock::Heading { level, text } => {
                assert_eq!(*level, 1);
                assert_eq!(text.text, "Title");
            }
            block => panic!("expected heading, got {block:?}"),
        }
        match &blocks[1] {
            ReviewMarkdownBlock::Paragraph(text) => {
                assert_eq!(text.text, "Paragraph with bold and code rule.");
                assert_eq!(text.highlights.len(), 2);
                assert_eq!(text.font_family_overrides.len(), 1);
                assert_eq!(text.links.len(), 1);
                let (range, url) = &text.links[0];
                assert_eq!(&text.text[range.clone()], "rule");
                assert_eq!(url, "https://example.com");
            }
            block => panic!("expected paragraph, got {block:?}"),
        }
        match &blocks[2] {
            ReviewMarkdownBlock::List(items) => {
                assert_eq!(items.len(), 2);
                assert_eq!(items[0].marker, "-");
                assert_eq!(items[0].text.text, "item one");
                assert_eq!(items[1].text.text, "item two");
            }
            block => panic!("expected list, got {block:?}"),
        }
        assert!(
            matches!(&blocks[3], ReviewMarkdownBlock::Quote(lines) if lines[0].text == "quoted")
        );
        assert!(
            matches!(&blocks[4], ReviewMarkdownBlock::Code(lines) if lines == &vec!["let value = 1;".to_string()])
        );
    }

    #[test]
    fn parses_gfm_task_lists_tables_and_autolinks() {
        let blocks = parse_review_markdown_blocks(
            "* [x] done\n\n| rule | url |\n| --- | --- |\n| semgrep | https://example.com |\n\n~~stale~~",
        );

        assert_eq!(blocks.len(), 3);
        match &blocks[0] {
            ReviewMarkdownBlock::List(items) => {
                assert_eq!(items.len(), 1);
                assert_eq!(items[0].marker, "[x]");
                assert_eq!(items[0].text.text, "done");
            }
            block => panic!("expected task list, got {block:?}"),
        }
        match &blocks[1] {
            ReviewMarkdownBlock::Table(rows) => {
                assert_eq!(rows.len(), 2);
                assert_eq!(rows[0][0].text, "rule");
                assert_eq!(rows[1][1].text, "https://example.com");
                assert_eq!(rows[1][1].highlights.len(), 1);
                assert_eq!(rows[1][1].links.len(), 1);
                let (range, url) = &rows[1][1].links[0];
                assert_eq!(&rows[1][1].text[range.clone()], "https://example.com");
                assert_eq!(url, "https://example.com");
            }
            block => panic!("expected table, got {block:?}"),
        }
        match &blocks[2] {
            ReviewMarkdownBlock::Paragraph(text) => {
                assert_eq!(text.text, "stale");
                assert_eq!(text.highlights.len(), 1);
            }
            block => panic!("expected strikethrough paragraph, got {block:?}"),
        }
    }

    #[test]
    fn parses_rewritten_html_anchors_as_clickable_links() {
        let blocks = parse_review_markdown_blocks(&review_markdown_body(
            "<a href=\"https://semgrep.dev/findings/1\">View finding</a>",
        ));

        match &blocks[0] {
            ReviewMarkdownBlock::Paragraph(text) => {
                assert_eq!(text.text, "View finding");
                assert_eq!(text.links.len(), 1);
                let (range, url) = &text.links[0];
                assert_eq!(&text.text[range.clone()], "View finding");
                assert_eq!(url, "https://semgrep.dev/findings/1");
            }
            block => panic!("expected paragraph, got {block:?}"),
        }
    }
}
