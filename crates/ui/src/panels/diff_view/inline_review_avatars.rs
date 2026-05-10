use gpui::{IntoElement, div, prelude::*, px};
use gpui_component::{Sizable, avatar::Avatar};
use harbor_domain::ReviewComment;

pub(super) fn render_review_comment_avatar(comment: &ReviewComment) -> impl IntoElement {
    let avatar = Avatar::new()
        .name(comment.author.clone())
        .with_size(px(20.0));
    let avatar = if let Some(avatar_url) = review_comment_avatar_url(comment) {
        avatar.src(avatar_url)
    } else {
        avatar
    };

    div().mt(px(1.0)).flex_none().child(avatar)
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
}
