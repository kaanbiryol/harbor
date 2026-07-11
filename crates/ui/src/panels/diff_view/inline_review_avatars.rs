use gpui::{IntoElement, div, prelude::*, px};
use gpui_component::{Sizable, StyledExt, avatar::Avatar};
use harbor_domain::ReviewComment;

use crate::{
    github::{avatar_initial, avatar_url},
    visual::color,
};

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
        .border_color(color::border_strong())
        .bg(color::row_selected_subtle())
        .text_size(px(11.0))
        .line_height(px(20.0))
        .font_semibold()
        .text_color(color::accent())
        .child(avatar_initial(&comment.author))
}

pub(crate) fn review_comment_avatar_url(comment: &ReviewComment) -> Option<String> {
    comment
        .author_avatar_url
        .clone()
        .or_else(|| avatar_url(&comment.author))
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
    }
}
