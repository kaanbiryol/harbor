use std::sync::Arc;

use gpui::TestAppContext;

use crate::{
    test_fixtures::{patched_diff_file, pull_request, workflow_run},
    workspace::{
        GitHubAuthSource, GitHubAuthStatus,
        github_service::{GitHubApi, test_support::FakeGitHubApi},
    },
};

use super::init_workspace_service_test;

#[test]
fn fake_github_api_records_auth_source_calls() {
    let api = FakeGitHubApi::default();

    api.configure_token("oauth-token".to_string(), GitHubAuthSource::OAuth)
        .expect("fake oauth token auth should succeed");
    api.configure_gh_cli()
        .expect("fake github cli auth should succeed");
    api.clear_auth().expect("fake auth clear should succeed");

    assert_eq!(
        api.calls(),
        vec!["configure_oauth_token", "configure_gh_cli", "clear_auth",]
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
        view.pull_request_inbox.start_loading();

        view.show_github_sign_in_required();

        assert!(view.pull_requests.is_empty());
        assert!(view.current_repository().is_none());
        assert!(view.detail_state.files().is_empty());
        assert!(view.detail_state.workflow_runs().is_empty());
        assert_eq!(view.review_state.current_user_login(), None);
        assert!(!view.pull_request_inbox.is_loading());
        assert_eq!(
            view.status,
            "Choose a GitHub sign-in method to load repositories"
        );
    });
}
