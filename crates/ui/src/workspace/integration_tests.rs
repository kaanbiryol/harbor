use harbor_domain::PullRequest;
use harbor_github::GitHubError;

use crate::{
    test_fixtures::patched_diff_file,
    workspace::test_support::{FakeGitHubApi, init_workspace_service_test},
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

fn assert_calls_unordered(mut actual: Vec<String>, expected: Vec<&str>) {
    let mut expected = expected.into_iter().map(str::to_string).collect::<Vec<_>>();
    actual.sort();
    expected.sort();
    assert_eq!(actual, expected);
}
