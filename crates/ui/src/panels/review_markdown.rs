use crate::gpui_component::{
    StyledExt,
    highlighter::LanguageRegistry,
    text::{TextView, TextViewState, TextViewStyle},
};
use gpui::{AnyElement, Entity, IntoElement, div, prelude::*, px, rems};

use crate::visual::{Tone, color, tone_colors};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReviewSuggestionContext {
    pub(crate) original_lines: Vec<ReviewSuggestionOriginalLine>,
    pub(crate) replacement_start_line: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReviewSuggestionOriginalLine {
    pub(crate) line_number: u32,
    pub(crate) text: String,
}

#[derive(Debug, Eq, PartialEq)]
struct ReviewSuggestionDisplayLine {
    line_number: Option<u32>,
    marker: &'static str,
    text: String,
    tone: Tone,
}

pub(crate) fn render_review_markdown_body(id: impl Into<String>, body: &str) -> AnyElement {
    render_review_markdown_body_with_context(id, body, None)
}

pub(crate) fn render_review_markdown_body_with_context(
    id: impl Into<String>,
    body: &str,
    suggestion_context: Option<&ReviewSuggestionContext>,
) -> AnyElement {
    let id = id.into();
    if !review_markdown_has_suggestion(body) {
        return render_review_markdown_text(id, review_markdown_body(body)).into_any_element();
    }

    div()
        .w_full()
        .min_w_0()
        .flex()
        .flex_col()
        .gap_2()
        .children(
            review_markdown_sections(body)
                .into_iter()
                .enumerate()
                .map(|(index, section)| match section {
                    ReviewMarkdownSection::Markdown(markdown) => render_review_markdown_text(
                        format!("{id}-markdown-{index}"),
                        review_markdown_body(&markdown),
                    )
                    .into_any_element(),
                    ReviewMarkdownSection::Suggestion(suggestion) => render_review_suggestion(
                        format!("{id}-suggestion-{index}"),
                        suggestion,
                        suggestion_context.cloned(),
                    )
                    .into_any_element(),
                }),
        )
        .into_any_element()
}

fn render_review_markdown_text(id: String, body: String) -> impl IntoElement {
    TextView::markdown(id, body)
        .style(review_markdown_style())
        .selectable(true)
        .min_w_0()
        .flex_none()
}

fn render_review_suggestion(
    id: String,
    suggestion: String,
    context: Option<ReviewSuggestionContext>,
) -> impl IntoElement {
    let show_line_numbers = context.is_some();
    let lines = review_suggestion_display_lines(&suggestion, context.as_ref());

    div()
        .id(id)
        .w_full()
        .min_w_0()
        .overflow_hidden()
        .rounded_xs()
        .border_1()
        .border_color(color::border_strong())
        .child(
            div()
                .w_full()
                .px_2()
                .py_1()
                .border_b_1()
                .border_color(color::border_strong())
                .bg(color::elevated_background())
                .text_xs()
                .font_medium()
                .text_color(color::text_muted())
                .child("Suggested change"),
        )
        .child(
            div()
                .w_full()
                .min_w_0()
                .flex()
                .flex_col()
                .children(lines.into_iter().map(move |line| {
                    let line_colors = tone_colors(line.tone);
                    div()
                        .w_full()
                        .min_w_0()
                        .flex()
                        .items_stretch()
                        .bg(line_colors.background)
                        .font_family("Lilex")
                        .text_size(px(12.0))
                        .line_height(px(20.0))
                        .text_color(color::text_primary())
                        .when(show_line_numbers, |element| {
                            element.child(
                                div()
                                    .w_10()
                                    .flex_none()
                                    .flex()
                                    .items_center()
                                    .justify_end()
                                    .pr_2()
                                    .bg(line_colors.border)
                                    .child(
                                        line.line_number.map_or_else(String::new, |line_number| {
                                            line_number.to_string()
                                        }),
                                    ),
                            )
                        })
                        .child(
                            div()
                                .w_6()
                                .flex_none()
                                .flex()
                                .items_center()
                                .justify_center()
                                .bg(line_colors.border)
                                .text_color(line_colors.text)
                                .child(line.marker),
                        )
                        .child(
                            div()
                                .min_w_0()
                                .flex_1()
                                .px_2()
                                .py_1()
                                .whitespace_normal()
                                .child(
                                    if line.text.is_empty() {
                                        " "
                                    } else {
                                        &line.text
                                    }
                                    .to_string(),
                                ),
                        )
                })),
        )
}

fn review_suggestion_display_lines(
    suggestion: &str,
    context: Option<&ReviewSuggestionContext>,
) -> Vec<ReviewSuggestionDisplayLine> {
    let original_line_count = context.map_or(0, |context| context.original_lines.len());
    let suggestion_line_count = suggestion.split('\n').count();
    let mut lines = Vec::with_capacity(original_line_count + suggestion_line_count);

    if let Some(context) = context {
        lines.extend(
            context
                .original_lines
                .iter()
                .map(|line| ReviewSuggestionDisplayLine {
                    line_number: Some(line.line_number),
                    marker: "-",
                    text: line.text.clone(),
                    tone: Tone::Danger,
                }),
        );
    }

    lines.extend(suggestion.split('\n').enumerate().map(|(index, text)| {
        let line_number = context.and_then(|context| {
            u32::try_from(index)
                .ok()
                .and_then(|index| context.replacement_start_line.checked_add(index))
        });
        ReviewSuggestionDisplayLine {
            line_number,
            marker: "+",
            text: text.to_string(),
            tone: Tone::Success,
        }
    }));

    lines
}

pub(crate) fn render_review_markdown_state(state: &Entity<TextViewState>) -> impl IntoElement {
    TextView::new(state)
        .style(review_markdown_style())
        .selectable(true)
        .min_w_0()
        .flex_none()
}

fn review_markdown_style() -> TextViewStyle {
    TextViewStyle::default()
        .paragraph_gap(rems(0.5))
        .heading_font_size(|level, _| match level {
            1 => px(14.0),
            2 => px(13.5),
            _ => px(13.0),
        })
}

fn markdown_language_registered(language: &str) -> bool {
    let language = language
        .trim()
        .trim_start_matches('.')
        .strip_prefix("language-")
        .unwrap_or(language);

    LanguageRegistry::singleton().language(language).is_some()
}

mod preparation;
pub(crate) use preparation::review_markdown_has_suggestion;
use preparation::{ReviewMarkdownSection, review_markdown_sections};

pub(crate) fn review_markdown_body(body: &str) -> String {
    preparation::review_markdown_body(body, markdown_language_registered)
}
pub(crate) fn overview_markdown_blocks(body: &str) -> Vec<String> {
    preparation::overview_markdown_blocks(body, markdown_language_registered)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn builds_removed_and_added_suggestion_diff_lines() {
        let context = ReviewSuggestionContext {
            original_lines: vec![ReviewSuggestionOriginalLine {
                line_number: 65,
                text: "await expectEventually { vm.lastRetriable == false }".to_string(),
            }],
            replacement_start_line: 65,
        };

        assert_eq!(
            review_suggestion_display_lines(
                "await expectEventually { mockPresenter.presentedErrors.count == 1 }\n#expect(vm.lastRetriable == false)",
                Some(&context),
            ),
            vec![
                ReviewSuggestionDisplayLine {
                    line_number: Some(65),
                    marker: "-",
                    text: "await expectEventually { vm.lastRetriable == false }".to_string(),
                    tone: Tone::Danger,
                },
                ReviewSuggestionDisplayLine {
                    line_number: Some(65),
                    marker: "+",
                    text: "await expectEventually { mockPresenter.presentedErrors.count == 1 }"
                        .to_string(),
                    tone: Tone::Success,
                },
                ReviewSuggestionDisplayLine {
                    line_number: Some(66),
                    marker: "+",
                    text: "#expect(vm.lastRetriable == false)".to_string(),
                    tone: Tone::Success,
                },
            ]
        );
    }
}
