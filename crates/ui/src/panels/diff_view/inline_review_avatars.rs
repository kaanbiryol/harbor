use gpui::{IntoElement, div, prelude::*, px, rgb};
use gpui_component::{Sizable, StyledExt, avatar::Avatar};
use harbor_domain::ReviewComment;

pub(super) fn render_review_comment_avatar(comment: &ReviewComment) -> impl IntoElement {
    let avatar = if let Some(avatar_url) = review_comment_avatar_url(comment) {
        Avatar::new()
            .src(avatar_url)
            .with_size(px(20.0))
            .into_any_element()
    } else {
        render_review_comment_fallback_avatar(comment).into_any_element()
    };

    div().mt(px(1.0)).flex_none().child(avatar)
}

fn render_review_comment_fallback_avatar(comment: &ReviewComment) -> impl IntoElement {
    div()
        .size(px(20.0))
        .flex()
        .items_center()
        .justify_center()
        .flex_none()
        .rounded_full()
        .border_1()
        .border_color(rgb(0x3b255f))
        .bg(rgb(0x1d1430))
        .text_size(px(11.0))
        .line_height(px(20.0))
        .font_semibold()
        .text_color(rgb(0xa78bfa))
        .child(review_comment_avatar_initial(&comment.author))
}

pub(crate) fn review_comment_avatar_url(comment: &ReviewComment) -> Option<String> {
    comment
        .author_avatar_url
        .clone()
        .or_else(|| github_avatar_url_for_login(&comment.author))
}

pub(crate) fn review_comment_avatar_initial(author: &str) -> String {
    author
        .trim()
        .chars()
        .find(|character| character.is_alphanumeric())
        .map(|character| character.to_uppercase().collect())
        .unwrap_or_else(|| "?".to_string())
}

pub(crate) fn github_avatar_url_for_login(login: &str) -> Option<String> {
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

#[cfg(test)]
mod tests {
    use crate::test_fixtures::review_comment;

    use super::*;

    #[test]
    fn resolves_review_comment_avatar_urls() {
        let mut comment = review_comment();

        assert_eq!(
            review_comment_avatar_url(&comment).as_deref(),
            Some("https://github.com/octocat.png?size=48")
        );

        comment.author_avatar_url =
            Some("https://avatars.githubusercontent.com/u/1?v=4".to_string());
        assert_eq!(
            review_comment_avatar_url(&comment).as_deref(),
            Some("https://avatars.githubusercontent.com/u/1?v=4")
        );

        assert_eq!(github_avatar_url_for_login("ghost"), None);
        assert_eq!(github_avatar_url_for_login("bad login"), None);
    }

    #[test]
    fn uses_single_centered_fallback_initial() {
        assert_eq!(review_comment_avatar_initial("you"), "Y");
        assert_eq!(review_comment_avatar_initial(" octocat"), "O");
        assert_eq!(review_comment_avatar_initial(""), "?");
    }
}
