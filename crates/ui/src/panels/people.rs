use crate::gpui_component::{Sizable, StyledExt, avatar::Avatar, tooltip::Tooltip};
use gpui::{AnyElement, IntoElement, Rgba, div, prelude::*, px};
use harbor_domain::ReviewComment;

use crate::{
    date_time::{full_time_label_with_edit, natural_time_label_with_edit},
    visual::color,
};

pub(crate) fn avatar_url(login: &str) -> Option<String> {
    let login = login.trim();

    if login.is_empty()
        || login.eq_ignore_ascii_case("ghost")
        || login.eq_ignore_ascii_case("you")
        || login.chars().any(char::is_whitespace)
    {
        None
    } else {
        Some(format!("https://github.com/{login}.png?size=48"))
    }
}

fn profile_url(login: &str) -> String {
    format!("https://github.com/{login}")
}

fn avatar_initial(label: &str) -> String {
    label
        .trim()
        .chars()
        .find(|character| character.is_alphanumeric())
        .map(|character| character.to_uppercase().collect())
        .unwrap_or_else(|| "?".to_string())
}

pub(crate) fn render_avatar(login: &str, avatar_url: Option<&str>, size: f32) -> AnyElement {
    if let Some(avatar_url) = avatar_url
        .map(str::to_string)
        .or_else(|| self::avatar_url(login))
    {
        return Avatar::new()
            .rounded_full()
            .src(avatar_url)
            .name(login.to_string())
            .with_size(px(size))
            .into_any_element();
    }

    render_initial_avatar(login, size).into_any_element()
}

pub(crate) fn render_initial_avatar(label: &str, size: f32) -> impl IntoElement {
    div()
        .size(px(size))
        .flex()
        .items_center()
        .justify_center()
        .flex_none()
        .rounded_full()
        .border_1()
        .border_color(color::border_strong())
        .bg(color::row_selected_subtle())
        .text_size(px((size * 0.52).max(9.0)))
        .line_height(px(size))
        .font_semibold()
        .text_color(color::accent())
        .child(avatar_initial(label))
}

pub(crate) fn render_author_link(id: String, author: String, text_color: Rgba) -> impl IntoElement {
    let profile_url = profile_url(&author);

    div()
        .id(id)
        .font_medium()
        .text_color(text_color)
        .cursor_pointer()
        .hover(|element| element.text_color(color::accent_hover()))
        .on_click(move |_, _, cx| {
            cx.open_url(&profile_url);
        })
        .child(author)
}

pub(crate) fn review_comment_time_label(comment: &ReviewComment) -> String {
    natural_time_label_with_edit(comment.created_at, comment.updated_at)
}

pub(crate) fn review_comment_time_tooltip(comment: &ReviewComment) -> String {
    full_time_label_with_edit(comment.created_at, comment.updated_at)
}

pub(crate) fn render_time_metadata(
    id: String,
    label: String,
    tooltip: Option<String>,
    text_color: Rgba,
) -> impl IntoElement {
    div()
        .id(id)
        .text_xs()
        .text_color(text_color)
        .when_some(tooltip, |element, tooltip| {
            element.tooltip(move |window, cx| Tooltip::new(tooltip.clone()).build(window, cx))
        })
        .child(label)
}

#[cfg(test)]
mod tests {
    use super::{avatar_initial, avatar_url, profile_url};

    #[test]
    fn builds_avatar_urls_for_github_users() {
        assert_eq!(
            avatar_url("octocat").as_deref(),
            Some("https://github.com/octocat.png?size=48")
        );
        assert_eq!(avatar_url("ghost"), None);
        assert_eq!(avatar_url("you"), None);
        assert_eq!(avatar_url("bad login"), None);
        assert_eq!(avatar_url(""), None);
    }

    #[test]
    fn builds_profile_urls() {
        assert_eq!(profile_url("octocat"), "https://github.com/octocat");
    }

    #[test]
    fn derives_avatar_initials() {
        assert_eq!(avatar_initial("octocat"), "O");
        assert_eq!(avatar_initial(" team-reviewers"), "T");
        assert_eq!(avatar_initial(""), "?");
    }
}
