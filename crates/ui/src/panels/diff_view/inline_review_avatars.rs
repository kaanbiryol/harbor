use gpui::{AnyElement, IntoElement, div, img, prelude::*, px, rgb};
use gpui_component::StyledExt;
use harbor_domain::ReviewComment;

pub(super) fn render_review_comment_avatar(comment: &ReviewComment) -> impl IntoElement {
    let initial = author_initial(&comment.author);
    let avatar = div()
        .mt(px(1.0))
        .w(px(20.0))
        .h(px(20.0))
        .flex_none()
        .rounded_xs()
        .border_1()
        .border_color(rgb(0x334155))
        .bg(rgb(0x1d2734))
        .flex()
        .items_center()
        .justify_center()
        .text_xs()
        .font_medium()
        .text_color(rgb(0xcbd5e1));

    if let Some(avatar_url) = review_comment_avatar_url(comment) {
        let loading_initial = initial.clone();
        let fallback_initial = initial.clone();
        avatar
            .overflow_hidden()
            .child(
                img(avatar_url)
                    .w(px(20.0))
                    .h(px(20.0))
                    .with_loading(move || render_review_comment_avatar_initial(&loading_initial))
                    .with_fallback(move || render_review_comment_avatar_initial(&fallback_initial)),
            )
            .into_any_element()
    } else {
        avatar.child(initial).into_any_element()
    }
}

fn render_review_comment_avatar_initial(initial: &str) -> AnyElement {
    div()
        .w(px(20.0))
        .h(px(20.0))
        .flex()
        .items_center()
        .justify_center()
        .text_xs()
        .font_medium()
        .text_color(rgb(0xcbd5e1))
        .child(initial.to_string())
        .into_any_element()
}

pub(crate) fn review_comment_avatar_url(comment: &ReviewComment) -> Option<String> {
    comment
        .author_avatar_url
        .clone()
        .or_else(|| github_avatar_url_for_login(&comment.author))
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

fn author_initial(author: &str) -> String {
    author
        .chars()
        .find(|character| character.is_alphanumeric())
        .map(|character| character.to_uppercase().to_string())
        .unwrap_or_else(|| "?".to_string())
}

#[cfg(test)]
mod tests {
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

    fn review_comment() -> ReviewComment {
        ReviewComment {
            id: "comment".to_string(),
            author: "octocat".to_string(),
            author_avatar_url: None,
            body: "Looks good".to_string(),
            created_at: chrono::DateTime::parse_from_rfc3339("2026-05-01T10:00:00Z")
                .expect("valid test timestamp")
                .with_timezone(&chrono::Utc),
            updated_at: None,
            position: None,
            viewer_did_author: false,
            viewer_can_update: false,
            viewer_can_delete: false,
            viewer_can_react: true,
            reactions: Vec::new(),
        }
    }
}
