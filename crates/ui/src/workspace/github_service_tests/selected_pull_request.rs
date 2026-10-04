use std::sync::Arc;

use gpui::TestAppContext;
use harbor_domain::{
    CheckConclusion, CheckStatus, ChecksSummary, MergeState, ReviewDecision, ReviewThreadState,
};
use harbor_logs::parse_workflow_log;

use crate::{
    actions::PanelTab,
    test_fixtures::{check_run, patched_diff_file, pull_request, review_thread, workflow_run},
    workspace::github_service::test_support::FakeGitHubApi,
};

use super::init_workspace_service_test;

#[gpui::test]
async fn selected_metadata_refresh_does_not_refetch_files(cx: &mut TestAppContext) {
    let api = Arc::new(FakeGitHubApi::default());
    let mut updated_pull_request = pull_request();
    updated_pull_request.title = "Updated title".to_string();
    api.push_pull_request_detail(Ok(updated_pull_request.clone()));
    let (view_entity, cx) = init_workspace_service_test(cx, api.clone());

    view_entity.update(cx, |view, cx| {
        view.pull_requests = vec![pull_request()];
        view.selection_state.reset_pull_request_index();
        view.refresh_selected_pull_request_metadata_only(cx);
    });
    cx.run_until_parked();

    assert_eq!(api.calls(), vec!["get_pull_request"]);
    view_entity.read_with(cx, |view, _| {
        assert_eq!(view.pull_requests[0].title, "Updated title");
        assert!(view.detail_state.files().is_empty());
    });
}

#[gpui::test]
async fn full_refresh_finishes_commits_required_by_overview(cx: &mut TestAppContext) {
    let api = Arc::new(FakeGitHubApi::default());
    let (view_entity, cx) = init_workspace_service_test(cx, api.clone());

    view_entity.update(cx, |view, cx| {
        view.pull_requests = vec![pull_request()];
        view.selection_state.reset_pull_request_index();
        view.active_tab = PanelTab::Overview;
        view.refresh_selected_pull_request(cx);
    });
    cx.run_until_parked();

    assert!(
        api.calls()
            .contains(&"list_pull_request_commits".to_string())
    );
    view_entity.read_with(cx, |view, _| {
        assert!(view.detail_state.commits_finished());
    });
}

#[gpui::test]
async fn commit_diff_files_receive_syntax_highlights(cx: &mut TestAppContext) {
    let api = Arc::new(FakeGitHubApi::default());
    let mut file = patched_diff_file();
    file.patch = Some(
        "@@ -1,3 +1,3 @@\n fn main() {\n-let old_value = true;\n+let new_value = false;\n }\n"
            .to_string(),
    );
    api.push_files(Ok(vec![file]));
    let (view_entity, cx) = init_workspace_service_test(cx, api.clone());

    view_entity.update(cx, |view, cx| {
        view.pull_requests = vec![pull_request()];
        view.selection_state.reset_pull_request_index();
        view.select_commit("commit-sha".to_string(), cx);
    });
    cx.run_until_parked();

    assert_eq!(api.calls(), vec!["list_commit_files"]);
    view_entity.read_with(cx, |view, _| {
        let diff = view.detail_state.diffs()[0]
            .as_ref()
            .expect("commit patch should be parsed");
        assert!(
            diff.hunks
                .iter()
                .flat_map(|hunk| &hunk.lines)
                .any(|line| !line.syntax_highlights.is_empty())
        );
    });
}

#[gpui::test]
async fn commit_diff_syntax_highlighting_completes_for_all_files(cx: &mut TestAppContext) {
    let api = Arc::new(FakeGitHubApi::default());
    let files = (0..8)
        .map(|index| {
            let mut file = patched_diff_file();
            file.path = format!("src/file_{index}.rs");
            file.patch = Some(format!(
                "@@ -1,3 +1,3 @@\n fn main() {{\n-let old_value_{index} = true;\n+let new_value_{index} = false;\n }}\n"
            ));
            file
        })
        .collect();
    api.push_files(Ok(files));
    let (view_entity, cx) = init_workspace_service_test(cx, api);

    view_entity.update(cx, |view, cx| {
        view.pull_requests = vec![pull_request()];
        view.selection_state.reset_pull_request_index();
        view.select_commit("commit-sha".to_string(), cx);
    });
    cx.run_until_parked();

    view_entity.read_with(cx, |view, _| {
        assert!(view.detail_state.diffs().iter().all(|diff| {
            diff.as_ref().is_some_and(|diff| {
                diff.hunks
                    .iter()
                    .flat_map(|hunk| &hunk.lines)
                    .any(|line| !line.syntax_highlights.is_empty())
            })
        }));
    });
}

#[gpui::test]
async fn ignores_stale_pull_request_detail_results_after_selection_changes(
    cx: &mut TestAppContext,
) {
    let api = Arc::new(FakeGitHubApi::default());
    let first_pull_request = pull_request();
    let mut stale_detail = first_pull_request.clone();
    stale_detail.title = "Stale detail".to_string();
    let mut second_pull_request = pull_request();
    second_pull_request.number = 8;
    second_pull_request.title = "Selected detail".to_string();
    second_pull_request.head_sha = "def456".to_string();
    api.push_pull_request_detail(Ok(stale_detail));
    let (view_entity, cx) = init_workspace_service_test(cx, api);

    view_entity.update(cx, |view, cx| {
        view.pull_requests = vec![first_pull_request, second_pull_request.clone()];
        view.selection_state.reset_pull_request_index();
        let generation_before = view.review_data_generation();
        view.refresh_selected_pull_request(cx);
        assert!(view.review_data_generation() > generation_before);
        view.selection_state.set_pull_request_index(1);
    });
    cx.run_until_parked();

    view_entity.read_with(cx, |view, _| {
        assert_eq!(view.selected_pull_request_index(), 1);
        assert_eq!(view.pull_requests[1].title, "Selected detail");
        assert!(view.detail_state.files().is_empty());
        assert!(view.review_state.review_threads().is_empty());
    });
}

#[gpui::test]
async fn selected_metadata_replace_preserves_cached_row_signals(cx: &mut TestAppContext) {
    let api = Arc::new(FakeGitHubApi::default());
    let mut row_pull_request = pull_request();
    row_pull_request.review_decision = Some(ReviewDecision::Approved);
    row_pull_request.merge_state = Some(MergeState::Clean);
    row_pull_request.checks_summary = ChecksSummary {
        total: 3,
        passed: 3,
        failed: 0,
        pending: 0,
        skipped: 0,
    };
    row_pull_request.unresolved_threads = 2;
    let mut metadata = row_pull_request.clone();
    metadata.title = "REST detail".to_string();
    metadata.review_decision = None;
    metadata.merge_state = Some(MergeState::Unknown);
    metadata.checks_summary = ChecksSummary::default();
    metadata.unresolved_threads = 0;
    metadata.created_at = None;
    let (view_entity, cx) = init_workspace_service_test(cx, api);

    view_entity.update(cx, |view, _| {
        view.pull_requests = vec![row_pull_request.clone()];
        view.selection_state.reset_pull_request_index();
        view.replace_selected_pull_request_preserving_row_fields(metadata);
    });

    view_entity.read_with(cx, |view, _| {
        let selected = &view.pull_requests[0];
        assert_eq!(selected.title, "REST detail");
        assert_eq!(selected.review_decision, Some(ReviewDecision::Approved));
        assert_eq!(selected.merge_state, Some(MergeState::Clean));
        assert_eq!(selected.checks_summary, row_pull_request.checks_summary);
        assert_eq!(selected.unresolved_threads, 2);
        assert_eq!(selected.created_at, row_pull_request.created_at);
    });
}

#[gpui::test]
async fn selecting_uncached_pull_request_clears_previous_detail_state(cx: &mut TestAppContext) {
    let api = Arc::new(FakeGitHubApi::default());
    let first_pull_request = pull_request();
    let mut second_pull_request = pull_request();
    second_pull_request.number = 8;
    second_pull_request.head_sha = "def456".to_string();
    let (view_entity, cx) = init_workspace_service_test(cx, api);

    view_entity.update(cx, |view, cx| {
        view.pull_requests = vec![first_pull_request, second_pull_request];
        view.selection_state.reset_pull_request_index();
        view.detail_state
            .replace_diff_files(vec![patched_diff_file()], vec![None]);
        view.detail_state.replace_check_runs(vec![check_run(
            CheckStatus::Completed,
            Some(CheckConclusion::Success),
        )]);
        view.detail_state
            .replace_workflow_runs(vec![workflow_run()]);
        view.review_state
            .replace_loaded_review_threads(vec![review_thread(ReviewThreadState::Unresolved)]);
        view.detail_state.log_chunk = Some(Arc::new(parse_workflow_log(42, "build")));
        view.changed_files_state
            .collapsed_file_tree_folders
            .insert("src".to_string());
        view.checks_state
            .expanded_groups
            .insert("build".to_string());
        view.changed_files_state
            .expanded_diff_file_paths
            .insert("src/lib.rs".to_string());
        view.changed_files_state
            .collapsed_diff_file_paths
            .insert("src/other.rs".to_string());
        view.changed_files_state
            .reviewed_file_paths
            .insert("src/lib.rs".to_string());
        view.changed_files_state
            .excluded_file_type_filters
            .insert("rs".to_string());
        view.changed_files_state.show_files_owned_by_current_user = true;
        view.changed_files_state
            .owned_file_paths
            .insert("src/lib.rs".to_string());
        view.active_tab = PanelTab::Review;

        view.select_pull_request(1, cx);

        assert_eq!(view.selected_pull_request_index(), 1);
        assert_eq!(view.active_tab, PanelTab::Overview);
        assert!(view.detail_state.files().is_empty());
        assert!(view.detail_state.check_runs().is_empty());
        assert!(view.detail_state.workflow_runs().is_empty());
        assert!(view.review_state.review_threads().is_empty());
        assert!(view.detail_state.log_chunk.is_none());
        assert!(
            view.changed_files_state
                .collapsed_file_tree_folders
                .is_empty()
        );
        assert!(view.checks_state.expanded_groups.is_empty());
        assert!(view.changed_files_state.expanded_diff_file_paths.is_empty());
        assert!(
            view.changed_files_state
                .collapsed_diff_file_paths
                .is_empty()
        );
        assert!(view.changed_files_state.reviewed_file_paths.is_empty());
        assert!(
            view.changed_files_state
                .excluded_file_type_filters
                .is_empty()
        );
        assert!(!view.changed_files_state.show_files_owned_by_current_user);
        assert!(view.changed_files_state.owned_file_paths.is_empty());
        assert!(view.diff_list_items.is_empty());
    });
}

#[gpui::test]
async fn selecting_cached_pull_request_restores_detail_without_refetch(cx: &mut TestAppContext) {
    let api = Arc::new(FakeGitHubApi::default());
    let first_pull_request = pull_request();
    let mut second_pull_request = pull_request();
    second_pull_request.number = 8;
    second_pull_request.head_sha = "def456".to_string();
    let (view_entity, cx) = init_workspace_service_test(cx, api.clone());

    view_entity.update(cx, |view, cx| {
        view.pull_requests = vec![first_pull_request, second_pull_request];
        view.selection_state.reset_pull_request_index();
        view.detail_state
            .replace_diff_files(vec![patched_diff_file()], vec![None]);
        view.detail_state.replace_check_runs(vec![check_run(
            CheckStatus::Completed,
            Some(CheckConclusion::Success),
        )]);
        view.detail_state
            .replace_workflow_runs(vec![workflow_run()]);
        view.review_state
            .replace_loaded_review_threads(vec![review_thread(ReviewThreadState::Unresolved)]);
        view.detail_state.apply_details_success();
        view.detail_state.apply_files_success();
        view.detail_state.apply_checks_success();
        view.detail_state.apply_workflows_success();
        view.review_state.apply_reviews_success();
        view.selection_state.set_diff_position(0, 3);
        view.active_tab = PanelTab::Review;
        view.cache_current_pull_request_detail_snapshot();

        view.selection_state.set_pull_request_index(1);
        view.clear_selected_pull_request_detail_state();
        view.select_pull_request(0, cx);

        assert_eq!(view.selected_pull_request_index(), 0);
        assert_eq!(view.active_tab, PanelTab::Review);
        assert_eq!(view.active_hunk_index(), 3);
        assert_eq!(view.detail_state.files().len(), 1);
        assert_eq!(view.detail_state.check_runs().len(), 1);
        assert_eq!(view.detail_state.workflow_runs().len(), 1);
        assert_eq!(view.review_state.review_threads().len(), 1);
        assert_eq!(view.status, "Showing cached PR #7 details");
    });
    cx.run_until_parked();

    assert!(api.calls().is_empty());
}

#[gpui::test]
async fn skipped_jobs_do_not_request_logs(cx: &mut TestAppContext) {
    let api = Arc::new(FakeGitHubApi::default());
    api.push_workflow_jobs(Ok(vec![harbor_domain::WorkflowJob {
        id: 1,
        name: "Skipped".into(),
        status: harbor_domain::WorkflowStatus::Completed,
        conclusion: Some(harbor_domain::WorkflowConclusion::Skipped),
        steps: vec![],
    }]));
    let (view_entity, cx) = init_workspace_service_test(cx, api.clone());
    view_entity.update(cx, |view, cx| {
        view.pull_requests = vec![pull_request()];
        view.selection_state.reset_pull_request_index();
        view.detail_state
            .replace_workflow_runs(vec![workflow_run()]);
        view.load_selected_workflow_logs(cx);
    });
    cx.run_until_parked();
    assert_eq!(api.calls(), vec!["list_workflow_jobs_for_run"]);
    view_entity.read_with(cx, |view, cx| {
        assert!(
            !view
                .workflow_logs
                .as_ref()
                .unwrap()
                .read(cx)
                .state
                .is_loading()
        );
        assert!(
            view.workflow_logs
                .as_ref()
                .unwrap()
                .read(cx)
                .state
                .error()
                .is_none()
        );
        assert!(
            view.workflow_logs
                .as_ref()
                .unwrap()
                .read(cx)
                .state
                .chunk()
                .unwrap()
                .lines
                .is_empty()
        );
    });
}

#[gpui::test]
async fn switching_accounts_drops_log_loading_and_rejects_its_result(cx: &mut TestAppContext) {
    let api = Arc::new(FakeGitHubApi::default());
    api.push_workflow_jobs(Ok(Vec::new()));
    let old_logs = api.pending_workflow_logs();
    let (view, cx) = init_workspace_service_test(cx, api);
    view.update(cx, |view, cx| {
        view.pull_requests = vec![pull_request()];
        view.detail_state
            .replace_workflow_runs(vec![workflow_run()]);
        view.load_selected_workflow_logs(cx);
    });
    cx.run_until_parked();
    view.update(cx, |view, _| view.clear_authenticated_github_content());
    cx.run_until_parked();
    assert!(old_logs.send(Ok("old account output".into())).is_err());
    view.read_with(cx, |view, _| {
        assert!(view.workflow_logs.is_none());
        assert!(view.detail_state.log_chunk.is_none());
    });
}

#[gpui::test]
async fn failed_job_opens_its_failed_step(cx: &mut TestAppContext) {
    let api = Arc::new(FakeGitHubApi::default());
    api.push_workflow_jobs(Ok(vec![harbor_domain::WorkflowJob {
        id: 1,
        name: "Test".into(),
        status: harbor_domain::WorkflowStatus::Completed,
        conclusion: Some(harbor_domain::WorkflowConclusion::Failure),
        steps: vec![harbor_domain::WorkflowStep {
            number: 2,
            name: "Run tests".into(),
            status: harbor_domain::WorkflowStatus::Completed,
            conclusion: Some(harbor_domain::WorkflowConclusion::Failure),
            started_at: None,
            completed_at: None,
        }],
    }]));
    api.push_workflow_logs(Ok(
        "Test\tSetup\tok\nTest\tRun tests\t::error::failed".into()
    ));
    let (view_entity, cx) = init_workspace_service_test(cx, api.clone());
    view_entity.update(cx, |view, cx| {
        view.pull_requests = vec![pull_request()];
        view.selection_state.reset_pull_request_index();
        view.detail_state
            .replace_workflow_runs(vec![workflow_run()]);
        view.load_selected_workflow_logs(cx);
    });
    cx.run_until_parked();
    view_entity.read_with(cx, |view, cx| {
        assert_eq!(
            view.workflow_logs
                .as_ref()
                .unwrap()
                .read(cx)
                .state
                .selected_section,
            Some(("Test".into(), Some("Run tests".into())))
        );
        assert_eq!(
            *view
                .workflow_logs
                .as_ref()
                .unwrap()
                .read(cx)
                .state
                .visible_lines,
            vec![1]
        );
    });
}

#[gpui::test]
async fn logs_load_checks_from_other_workflows_without_downloading_their_output(
    cx: &mut TestAppContext,
) {
    let api = Arc::new(FakeGitHubApi::default());
    let (view, cx) = init_workspace_service_test(cx, api.clone());
    let mut title_run = workflow_run();
    title_run.name = "PR Title".into();
    let mut build_run = title_run.clone();
    build_run.id = 84;
    build_run.name = "PR".into();
    let job = |id, name: &str| harbor_domain::WorkflowJob {
        id,
        name: name.to_string(),
        status: harbor_domain::WorkflowStatus::Completed,
        conclusion: Some(harbor_domain::WorkflowConclusion::Success),
        steps: Vec::new(),
    };
    api.push_workflow_jobs(Ok(vec![job(1, "PR Title")]));
    api.push_workflow_logs(Ok("PR Title\tCheck PR title\ttitle passed".into()));
    view.update(cx, |view, cx| {
        view.pull_requests = vec![pull_request()];
        view.detail_state
            .replace_workflow_runs(vec![title_run, build_run]);
        view.load_selected_workflow_logs(cx);
    });
    cx.run_until_parked();

    let other_jobs = vec![job(2, "Android"), job(3, "iOS")];
    api.push_workflow_jobs(Ok(other_jobs.clone()));
    view.update(cx, |view, cx| view.load_workflow_log_navigation(cx));
    cx.run_until_parked();
    view.read_with(cx, |view, cx| {
        let jobs = view
            .workflow_logs
            .as_ref()
            .unwrap()
            .read(cx)
            .state
            .navigation_jobs
            .get(&84)
            .unwrap()
            .as_ref()
            .unwrap();
        assert_eq!(jobs.as_ref(), &other_jobs);
        assert_eq!(
            view.workflow_logs
                .as_ref()
                .unwrap()
                .read(cx)
                .state
                .chunk()
                .unwrap()
                .run_id,
            42
        );
    });
    assert_eq!(
        api.calls()
            .iter()
            .filter(|call| *call == "workflow_run_log")
            .count(),
        1
    );

    api.push_workflow_jobs(Ok(other_jobs));
    api.push_workflow_logs(Ok(
        "Android\tBuild\tandroid output\niOS\tBuild\tios output".into()
    ));
    view.update(cx, |view, cx| {
        view.workflow_logs.as_ref().unwrap().update(cx, |logs, cx| {
            logs.select_workflow_log_section(84, Some(("Android".into(), None)), cx)
        })
    });
    cx.run_until_parked();
    view.read_with(cx, |view, cx| {
        let state = &view.workflow_logs.as_ref().unwrap().read(cx).state;
        assert_eq!(state.chunk().unwrap().run_id, 84);
        assert_eq!(state.selected_section, Some(("Android".into(), None)));
        assert_eq!(state.visible_lines.as_ref(), &[0]);
        assert_eq!(state.chunk().unwrap().lines[0].text, "android output");
    });
}

#[gpui::test]
async fn head_change_restarts_in_flight_content(cx: &mut TestAppContext) {
    check_head_change(cx, false);
}

#[gpui::test]
async fn head_change_discards_content_that_finished_before_metadata(cx: &mut TestAppContext) {
    check_head_change(cx, true);
}

fn check_head_change(cx: &mut TestAppContext, files_finish_first: bool) {
    let api = Arc::new(FakeGitHubApi::default());
    let metadata = api.pending_detail();
    let files = api.pending_files();
    let mut new_file = patched_diff_file();
    new_file.path = "new-head.rs".into();
    api.push_files(Ok(vec![new_file]));
    let (view, cx) = init_workspace_service_test(cx, api.clone());
    view.update(cx, |view, cx| {
        view.pull_requests = vec![pull_request()];
        view.refresh_selected_pull_request(cx);
    });
    cx.run_until_parked();

    let files = if files_finish_first {
        files
            .send(Ok(vec![patched_diff_file()]))
            .expect("deliver old files");
        cx.run_until_parked();
        view.read_with(cx, |view, _| assert!(!view.detail_state.files().is_empty()));
        None
    } else {
        Some(files)
    };
    let mut updated = pull_request();
    updated.head_sha = "new-head".into();
    metadata.send(Ok(updated)).expect("deliver changed head");
    cx.run_until_parked();
    if let Some(files) = files {
        assert!(files.send(Ok(vec![patched_diff_file()])).is_err());
    }

    view.read_with(cx, |view, _| {
        assert_eq!(view.selected_pull_request().unwrap().head_sha, "new-head");
        assert_eq!(view.detail_state.files()[0].path, "new-head.rs");
        assert!(!view.detail_state.is_any_loading());
        assert!(!view.review_state.reviews_loading());
    });
    assert_eq!(
        api.calls()
            .iter()
            .filter(|call| *call == "list_pull_request_files")
            .count(),
        2
    );
}
