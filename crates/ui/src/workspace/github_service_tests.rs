use std::sync::Arc;

use gpui::{
    AppContext, Context, Entity, IntoElement, Render, TestAppContext, VisualTestContext, Window,
    div,
};
use gpui_component::{Root, Theme, ThemeMode};
use harbor_domain::PullRequest;
use harbor_github::GitHubError;

use crate::{
    test_fixtures::patched_diff_file,
    workspace::{AppView, github_service::test_support::FakeGitHubApi},
};

#[path = "github_service_tests/action_results.rs"]
mod action_results;
#[path = "github_service_tests/auth.rs"]
mod auth;
#[path = "github_service_tests/cache.rs"]
mod cache;
#[path = "github_service_tests/file_viewed_state.rs"]
mod file_viewed_state;
#[path = "github_service_tests/focus.rs"]
mod focus;
#[path = "github_service_tests/inbox.rs"]
mod inbox;
#[path = "github_service_tests/loading.rs"]
mod loading;
#[path = "github_service_tests/review.rs"]
mod review;
#[path = "github_service_tests/selected_pull_request.rs"]
mod selected_pull_request;

fn init_workspace_service_test(
    cx: &mut TestAppContext,
    api: Arc<FakeGitHubApi>,
) -> (Entity<AppView>, &mut VisualTestContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
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
