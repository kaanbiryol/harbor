use gpui::{Context, IntoElement, div, prelude::*, px};
use harbor_domain::PullRequest;

use crate::{
    visual::{color, layout},
    workspace::AppView,
};

impl AppView {
    pub(super) fn render_details(
        &self,
        pr: Option<&PullRequest>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let Some(pr) = pr else {
            return div()
                .w(px(layout::PULL_REQUEST_DETAILS_WIDTH))
                .flex()
                .flex_col()
                .min_h_0()
                .border_1()
                .border_color(color::border())
                .bg(color::panel_background())
                .overflow_hidden()
                .p_3()
                .text_sm()
                .text_color(color::text_muted())
                .child("Select a pull request to see details")
                .into_any_element();
        };

        div()
            .w(px(layout::PULL_REQUEST_DETAILS_WIDTH))
            .flex()
            .flex_col()
            .min_h_0()
            .border_1()
            .border_color(color::border())
            .bg(color::panel_background())
            .overflow_hidden()
            .child(self.render_pull_request_details_header(pr, cx))
            .child(self.render_pull_request_overview(pr))
            .child(self.render_changed_files_header(cx))
            .child(self.render_changed_files_body(cx))
            .into_any_element()
    }
}
