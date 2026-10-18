use super::*;
use crate::{ChecksSummary, PullRequestMergeCapabilities, test_fixtures::pull_request};

#[test]
fn missing_checks_and_review_do_not_add_a_row_signal() {
    let mut pr = pull_request();
    pr.checks_summary = ChecksSummary::default();

    assert_eq!(pull_request_row_signal(&pr), None);
}

#[test]
fn conflict_takes_priority_over_review_feedback() {
    let mut pr = pull_request();
    pr.merge_state = Some(MergeState::Dirty);
    pr.checks_summary.failed = 1;
    pr.review_decision = Some(ReviewDecision::ChangesRequested);
    pr.unresolved_threads = 2;

    assert_eq!(
        pull_request_row_signal(&pr),
        Some(PullRequestRowSignal::MergeConflict)
    );

    pr.merge_state = Some(MergeState::Clean);
    assert_eq!(
        pull_request_row_signal(&pr),
        Some(PullRequestRowSignal::ChangesRequested)
    );
}

#[test]
fn changes_requested_without_threads_still_has_review_signal() {
    let mut pr = pull_request();
    pr.review_decision = Some(ReviewDecision::ChangesRequested);

    assert_eq!(
        pull_request_row_signal(&pr),
        Some(PullRequestRowSignal::ChangesRequested)
    );
}

#[test]
fn approval_and_threads_do_not_add_row_signals() {
    let mut pr = pull_request();
    pr.review_decision = Some(ReviewDecision::Approved);

    assert_eq!(pull_request_row_signal(&pr), None);

    pr.unresolved_threads = 2;
    assert_eq!(pull_request_row_signal(&pr), None);
    assert_eq!(
        pull_request_row_rail_tone(&pr),
        PullRequestRowRailTone::Neutral
    );
}

#[test]
fn queue_status_remains_available_without_adding_row_signals() {
    let mut pr = pull_request();
    pr.review_decision = Some(ReviewDecision::Approved);
    pr.merge_capabilities = PullRequestMergeCapabilities {
        queue_state: MergeQueueState::Enabled,
        auto_merge_enabled: true,
        ..Default::default()
    };

    assert_eq!(
        pull_request_merge_queue_status(&pr),
        Some(PullRequestMergeQueueStatus::Waiting)
    );
    assert_eq!(pull_request_row_signal(&pr), None);

    pr.merge_capabilities.queue_state = MergeQueueState::Queued;
    assert_eq!(
        pull_request_merge_queue_status(&pr),
        Some(PullRequestMergeQueueStatus::Queued)
    );
    assert_eq!(pull_request_row_signal(&pr), None);

    pr.state = PullRequestState::Closed;
    assert_eq!(pull_request_merge_queue_status(&pr), None);
}

#[test]
fn review_required_stays_visible_alongside_ci() {
    let mut pr = pull_request();
    pr.review_decision = Some(ReviewDecision::ReviewRequired);

    for (failed, pending, expected_rail) in [
        (0, 0, PullRequestRowRailTone::Warning),
        (0, 1, PullRequestRowRailTone::Warning),
        (1, 0, PullRequestRowRailTone::Danger),
    ] {
        pr.checks_summary.failed = failed;
        pr.checks_summary.pending = pending;
        assert_eq!(
            pull_request_row_signal(&pr),
            Some(PullRequestRowSignal::ReviewNeeded)
        );
        assert_eq!(pull_request_row_rail_tone(&pr), expected_rail);
    }
}

#[test]
fn approval_only_gets_a_green_rail_when_ready_to_merge() {
    let mut pr = pull_request();
    pr.review_decision = Some(ReviewDecision::Approved);
    assert_eq!(
        pull_request_row_rail_tone(&pr),
        PullRequestRowRailTone::Success
    );

    for merge_state in [
        None,
        Some(MergeState::Unknown),
        Some(MergeState::Blocked),
        Some(MergeState::Behind),
    ] {
        pr.merge_state = merge_state;
        assert_eq!(
            pull_request_row_rail_tone(&pr),
            PullRequestRowRailTone::Neutral
        );
    }
}

#[test]
fn ci_failure_sets_a_red_rail_without_a_duplicate_blocker_icon() {
    let mut pr = pull_request();
    pr.review_decision = Some(ReviewDecision::Approved);
    pr.checks_summary.failed = 1;

    assert_eq!(
        pull_request_row_rail_tone(&pr),
        PullRequestRowRailTone::Danger
    );
    assert_eq!(pull_request_row_signal(&pr), None);

    pr.checks_summary.failed = 0;
    pr.checks_summary.pending = 1;

    assert_eq!(
        pull_request_row_rail_tone(&pr),
        PullRequestRowRailTone::Neutral
    );
    assert_eq!(pull_request_row_signal(&pr), None);
}

#[test]
fn drafts_and_finished_pull_requests_have_no_action_signal_or_colored_rail() {
    let mut pr = pull_request();
    pr.merge_state = Some(MergeState::Dirty);
    pr.review_decision = Some(ReviewDecision::ChangesRequested);
    pr.checks_summary.failed = 1;

    for (state, is_draft) in [
        (PullRequestState::Open, true),
        (PullRequestState::Closed, false),
        (PullRequestState::Merged, false),
    ] {
        pr.state = state;
        pr.is_draft = is_draft;
        assert!(!is_ready_to_merge(&pr));
        assert_eq!(pull_request_row_signal(&pr), None);
        assert_eq!(
            pull_request_row_rail_tone(&pr),
            PullRequestRowRailTone::Neutral
        );
    }
}

#[test]
fn allows_review_actions_for_open_pull_requests() {
    assert_eq!(review_action_blocker(&pull_request()), None);
}

#[test]
fn blocks_merge_until_pull_request_is_ready() {
    let mut pr = pull_request();
    pr.checks_summary.pending = 1;

    assert_eq!(
        merge_blocker(&pr).as_deref(),
        Some("PR #7 still has pending checks")
    );

    pr.checks_summary.pending = 0;
    pr.unresolved_threads = 2;

    assert_eq!(
        merge_blocker(&pr).as_deref(),
        Some("PR #7 still has 2 unresolved review threads")
    );
}

#[test]
fn allows_clean_pull_request_merge() {
    assert_eq!(merge_blocker(&pull_request()), None);
}

#[test]
fn merge_when_ready_allows_pending_requirements_for_queue() {
    let mut pr = pull_request();
    pr.checks_summary.pending = 1;
    pr.review_decision = Some(ReviewDecision::ReviewRequired);
    pr.merge_capabilities = PullRequestMergeCapabilities {
        queue_state: MergeQueueState::Enabled,
        viewer_can_queue: true,
        ..Default::default()
    };

    assert_eq!(merge_when_ready_blocker(&pr), None);

    pr.merge_capabilities.queue_state = MergeQueueState::Queued;
    assert_eq!(
        merge_when_ready_blocker(&pr).as_deref(),
        Some("PR #7 is already queued to merge")
    );

    pr.merge_capabilities.queue_state = MergeQueueState::Enabled;
    pr.merge_capabilities.viewer_can_queue = false;
    assert_eq!(
        merge_when_ready_blocker(&pr).as_deref(),
        Some("You do not have permission to queue PR #7")
    );
}

#[test]
fn admin_bypass_ignores_requirements_but_not_conflicts() {
    let mut pr = pull_request();
    pr.merge_state = Some(MergeState::Blocked);
    pr.checks_summary.failed = 1;
    pr.unresolved_threads = 2;
    pr.merge_capabilities.viewer_can_merge_as_admin = true;

    assert_eq!(merge_without_requirements_blocker(&pr), None);

    pr.merge_state = Some(MergeState::Dirty);
    assert_eq!(
        merge_without_requirements_blocker(&pr).as_deref(),
        Some("PR #7 has merge conflicts")
    );
}

#[test]
fn merge_plan_uses_direct_merge_when_queue_is_disabled() {
    let mut pr = pull_request();
    pr.merge_capabilities.queue_state = MergeQueueState::Disabled;

    let plan = merge_action_plan(&pr, false, false);

    assert_eq!(plan.control, MergeControl::Merge);
    assert!(!plan.disabled);
    assert_eq!(plan.tooltip, "Merge pull request");
    assert_eq!(plan.bypass_placement, None);

    pr.checks_summary.failed = 1;
    let plan = merge_action_plan(&pr, false, false);

    assert!(plan.disabled);
    assert_eq!(plan.tooltip, "PR #7 still has failing checks");
}

#[test]
fn merge_plan_waits_for_queue_settings() {
    let plan = merge_action_plan(&pull_request(), false, false);

    assert_eq!(plan.control, MergeControl::Merge);
    assert!(plan.disabled);
    assert_eq!(plan.tooltip, "Loading merge queue settings from GitHub");
}

#[test]
fn merge_plan_queues_when_merge_queue_is_enabled() {
    let mut pr = pull_request();
    pr.merge_capabilities = PullRequestMergeCapabilities {
        queue_state: MergeQueueState::Enabled,
        viewer_can_queue: true,
        ..Default::default()
    };

    let plan = merge_action_plan(&pr, false, false);

    assert_eq!(plan.control, MergeControl::MergeWhenReady);
    assert_eq!(
        plan.tooltip,
        "Add pull request to the merge queue when requirements are met"
    );

    pr.merge_capabilities.auto_merge_enabled = true;
    assert_eq!(
        merge_action_plan(&pr, false, false).control,
        MergeControl::QueueStatus(PullRequestMergeQueueStatus::Waiting)
    );
}

#[test]
fn merge_plan_places_admin_bypass_in_menu_only_when_merge_is_actionable() {
    let mut pr = pull_request();
    pr.merge_capabilities = PullRequestMergeCapabilities {
        queue_state: MergeQueueState::Enabled,
        viewer_can_queue: true,
        viewer_can_merge_as_admin: true,
        ..Default::default()
    };

    assert_eq!(
        merge_action_plan(&pr, false, false).bypass_placement,
        Some(MergeBypassPlacement::MergeMenu)
    );
    assert_eq!(
        merge_action_plan(&pr, false, true).bypass_placement,
        Some(MergeBypassPlacement::Separate)
    );

    pr.merge_capabilities.queue_state = MergeQueueState::Queued;
    let plan = merge_action_plan(&pr, false, false);
    assert_eq!(
        plan.control,
        MergeControl::QueueStatus(PullRequestMergeQueueStatus::Queued)
    );
    assert_eq!(plan.bypass_placement, Some(MergeBypassPlacement::Separate));

    let plan = merge_action_plan(&pr, true, false);
    assert!(plan.bypass_enabled);
    assert_eq!(plan.control, MergeControl::Merge);
    assert_eq!(
        plan.tooltip,
        "Merge immediately without waiting for requirements"
    );
    assert_eq!(plan.bypass_placement, Some(MergeBypassPlacement::MergeMenu));
}

#[test]
fn merge_plan_ignores_bypass_request_without_admin_permission() {
    let mut pr = pull_request();
    pr.merge_capabilities.queue_state = MergeQueueState::Disabled;

    let plan = merge_action_plan(&pr, true, false);

    assert!(!plan.bypass_enabled);
    assert_eq!(plan.bypass_placement, None);
}

#[test]
fn classifies_overview_readiness_in_priority_order() {
    let mut pr = pull_request();
    pr.merge_state = Some(MergeState::Dirty);
    pr.checks_summary.failed = 1;
    pr.review_decision = Some(ReviewDecision::ChangesRequested);
    assert_eq!(pull_request_readiness(&pr), PullRequestReadiness::Conflicts);

    pr.merge_state = Some(MergeState::Clean);
    assert_eq!(
        pull_request_readiness(&pr),
        PullRequestReadiness::ChecksFailed
    );

    pr.checks_summary.failed = 0;
    pr.review_decision = Some(ReviewDecision::Approved);
    pr.unresolved_threads = 1;
    assert_eq!(
        pull_request_readiness(&pr),
        PullRequestReadiness::ConversationsOpen
    );
    assert_eq!(merge_readiness(&pr), MergeReadiness::ConversationsOpen);

    pr.unresolved_threads = 0;
    assert_eq!(pull_request_readiness(&pr), PullRequestReadiness::Ready);
    assert_eq!(merge_readiness(&pr), MergeReadiness::Ready);
}

#[test]
fn keeps_merge_action_eligibility_distinct_from_visual_approval_readiness() {
    let mut pr = pull_request();
    pr.review_decision = None;

    assert_eq!(merge_blocker(&pr), None);
    assert_eq!(merge_readiness(&pr), MergeReadiness::WaitingForApproval);
    assert_eq!(
        pull_request_readiness(&pr),
        PullRequestReadiness::ReviewRequired
    );
}
