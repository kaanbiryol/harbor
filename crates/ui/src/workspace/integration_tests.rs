use std::sync::Arc;

use crate::gpui_component::{Root, Theme, ThemeMode};
use gpui::{
    AppContext, Context, Entity, IntoElement, Render, TestAppContext, VisualTestContext, Window,
    div,
};
use harbor_domain::PullRequest;
use harbor_github::GitHubError;

use crate::{
    test_fixtures::patched_diff_file,
    workspace::{AppView, test_support::FakeGitHubApi},
};

mod action_results;
mod auth;
mod cache;
mod file_viewed_state;
mod focus;
mod inbox;
mod loading;
mod review;
mod search;
mod selected_pull_request;

fn init_workspace_service_test(
    cx: &mut TestAppContext,
    api: Arc<FakeGitHubApi>,
) -> (Entity<AppView>, &mut VisualTestContext) {
    cx.update(|cx| {
        crate::gpui_component::init(cx);
        Theme::change(ThemeMode::Dark, None, cx);
    });

    let mut view_entity = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| AppView::new_with_github_api(api, window, cx));
        view_entity = Some(view);
        Root::new(cx.new(|_| EmptyHarness), window, cx)
    });

    (
        view_entity.expect("workspace service test AppView should be created"),
        cx,
    )
}

fn enqueue_successful_detail_load(api: &FakeGitHubApi, pull_request: &PullRequest) {
    api.push_pull_request_detail(Ok(pull_request.clone()));
    api.push_files(Ok(vec![patched_diff_file()]));
    api.push_check_runs(Ok(Vec::new()));
    api.push_workflow_runs(Ok(Vec::new()));
    api.push_current_user(Ok("octocat".to_string()));
    api.push_reviews(Ok(Vec::new()));
    api.push_pull_request_comments(Ok(Vec::new()));
    api.push_review_threads(Ok(Vec::new()));
}

fn github_error(message: &str) -> GitHubError {
    GitHubError::Transport(message.to_string())
}

struct EmptyHarness;

impl Render for EmptyHarness {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

fn assert_calls_unordered(mut actual: Vec<String>, expected: Vec<&str>) {
    let mut expected = expected.into_iter().map(str::to_string).collect::<Vec<_>>();
    actual.sort();
    expected.sort();
    assert_eq!(actual, expected);
}
