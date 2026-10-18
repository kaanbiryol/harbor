use std::sync::Arc;

use gpui::TestAppContext;

use crate::{
    test_fixtures::{patched_diff_file, pull_request, workflow_run},
    workspace::{
        GitHubAuthSource, GitHubAuthStatus, PullRequestInboxCacheKey, test_support::FakeGitHubApi,
    },
};
use harbor_github::GitHubAuthApi;

use super::init_workspace_service_test;

#[test]
fn fake_github_api_records_auth_calls() {
    let api = FakeGitHubApi::default();

    api.configure_token("oauth-token".to_string())
        .expect("fake oauth token auth should succeed");
    api.configure_gh_cli()
        .expect("fake github cli auth should succeed");
    api.clear_auth().expect("fake auth clear should succeed");

    assert_eq!(
        api.calls(),
        vec!["configure_token", "configure_gh_cli", "clear_auth",]
    );
}

#[gpui::test]
async fn signed_out_state_clears_visible_github_content(cx: &mut TestAppContext) {
    let api = Arc::new(FakeGitHubApi::default());
    let pull_request = pull_request();
    let (view_entity, cx) = init_workspace_service_test(cx, api);

    view_entity.update(cx, |view, _| {
        view.auth_status = GitHubAuthStatus::SignedOut;
        view.repository_state
            .select_repository(pull_request.repo.clone());
        view.pull_requests = vec![pull_request];
        view.detail_state
            .replace_diff_files(vec![patched_diff_file()], Vec::new());
        view.detail_state
            .replace_workflow_runs(vec![workflow_run()]);
        view.review_state
            .set_current_user_login(Some("octocat".to_string()));
        view.cache_current_pull_request_detail_snapshot();
        view.cache_current_pull_request_inbox_snapshot();
        let detail_key = view.selected_pull_request_detail_key().unwrap();
        let inbox_key = PullRequestInboxCacheKey::new(
            view.pull_requests[0].repo.clone(),
            view.pull_request_inbox.mode(),
        );
        assert!(view.detail_state.snapshot(&detail_key).is_some());
        assert!(view.pull_request_inbox.snapshot(&inbox_key).is_some());
        view.pull_request_inbox.start_loading();

        view.show_github_sign_in_required();

        assert!(view.pull_requests.is_empty());
        assert!(view.current_repository().is_none());
        assert!(view.detail_state.files().is_empty());
        assert!(view.detail_state.workflow_runs().is_empty());
        assert_eq!(view.review_state.current_user_login(), None);
        assert!(!view.pull_request_inbox.is_loading());
        assert!(view.detail_state.snapshot(&detail_key).is_none());
        assert!(view.pull_request_inbox.snapshot(&inbox_key).is_none());
        assert_eq!(
            view.status,
            "Choose a GitHub sign-in method to load repositories"
        );
    });
}

#[gpui::test]
async fn delayed_account_response_cannot_replace_new_session(cx: &mut TestAppContext) {
    let api = Arc::new(FakeGitHubApi::default());
    let old_account = api.pending_current_user();
    let new_account = api.pending_current_user();
    let (view_entity, cx) = init_workspace_service_test(cx, api);
    view_entity.update(cx, |view, cx| {
        view.finish_authenticated_github_sign_in(GitHubAuthSource::GhCli, cx);
    });
    cx.run_until_parked();
    view_entity.update(cx, |view, cx| {
        view.finish_authenticated_github_sign_in(GitHubAuthSource::OAuth, cx);
    });
    cx.run_until_parked();
    new_account.send(Ok("new-account".into())).unwrap();
    cx.run_until_parked();
    old_account.send(Ok("old-account".into())).unwrap();
    cx.run_until_parked();
    view_entity.read_with(cx, |view, _| {
        assert_eq!(view.cache_account.as_deref(), Some("new-account"));
        assert_eq!(view.review_state.current_user_login(), Some("new-account"));
        assert!(matches!(&view.auth_status, GitHubAuthStatus::SignedIn { login: Some(login), .. } if login == "new-account"));
    });
}
