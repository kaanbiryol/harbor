use harbor_domain::{CheckConclusion, CheckStatus, ReviewDecision};

use super::*;
use crate::test_fixtures::{check_run, pull_request};

#[test]
fn summarizes_check_runs() {
    let check_runs = vec![
        check_run(CheckStatus::Completed, Some(CheckConclusion::Success)),
        check_run(CheckStatus::Completed, Some(CheckConclusion::Failure)),
        check_run(CheckStatus::Completed, Some(CheckConclusion::Skipped)),
        check_run(CheckStatus::InProgress, None),
    ];

    let summary = checks_summary_from_runs(&check_runs);

    assert_eq!(summary.total, 4);
    assert_eq!(summary.passed, 1);
    assert_eq!(summary.failed, 1);
    assert_eq!(summary.skipped, 1);
    assert_eq!(summary.pending, 1);
}

#[test]
fn shows_missing_checks_without_no_review_noise() {
    let mut pr = pull_request();
    pr.checks_summary = ChecksSummary::default();

    let signals = visible_pull_request_row_signals(&pr);

    assert!(signals.is_empty());
}

#[test]
fn prioritizes_action_signals_without_draft_text() {
    let mut pr = pull_request();
    pr.is_draft = true;
    pr.checks_summary = ChecksSummary {
        total: 4,
        passed: 2,
        failed: 1,
        pending: 1,
        skipped: 0,
    };
    pr.review_decision = Some(ReviewDecision::ChangesRequested);
    pr.unresolved_threads = 2;

    let signals = visible_pull_request_row_signals(&pr);

    assert_eq!(
        signal_summary(&signals),
        vec![
            (PullRequestRowSignalKind::ChecksFailed, None),
            (
                PullRequestRowSignalKind::ReviewChangesRequested,
                Some("changes".to_string())
            ),
            (
                PullRequestRowSignalKind::UnresolvedThreads,
                Some("2".to_string())
            )
        ]
    );
}

#[test]
fn shows_unresolved_threads_without_no_review_noise() {
    let mut pr = pull_request();
    pr.unresolved_threads = 2;

    let signals = visible_pull_request_row_signals(&pr);

    assert_eq!(
        signal_summary(&signals),
        vec![(
            PullRequestRowSignalKind::UnresolvedThreads,
            Some("2".to_string())
        )]
    );
}

#[test]
fn shows_conflict_as_the_only_row_signal() {
    let mut pr = pull_request();
    pr.merge_state = Some(MergeState::Dirty);
    pr.review_decision = Some(ReviewDecision::Approved);
    pr.unresolved_threads = 2;

    let signals = visible_pull_request_row_signals(&pr);

    assert_eq!(
        signal_summary(&signals),
        vec![(
            PullRequestRowSignalKind::Conflict,
            Some("conflict".to_string())
        )]
    );
}

#[test]
fn ready_pull_request_suppresses_redundant_approved_signal() {
    let mut pr = pull_request();
    pr.review_decision = Some(ReviewDecision::Approved);

    let signals = visible_pull_request_row_signals(&pr);

    assert_eq!(
        signal_summary(&signals),
        vec![(PullRequestRowSignalKind::ChecksPassed, None)]
    );
}

#[test]
fn colors_review_posture_on_row_rail() {
    let mut pr = pull_request();
    pr.review_decision = Some(ReviewDecision::Approved);
    assert_eq!(
        pull_request_row_rail_tone(&pr),
        PullRequestRowRailTone::Success
    );

    pr.review_decision = Some(ReviewDecision::ChangesRequested);
    assert_eq!(
        pull_request_row_rail_tone(&pr),
        PullRequestRowRailTone::Danger
    );

    pr.review_decision = None;
    pr.unresolved_threads = 1;
    assert_eq!(
        pull_request_row_rail_tone(&pr),
        PullRequestRowRailTone::Warning
    );
}

#[test]
fn row_rail_prioritizes_blockers_over_approval() {
    let mut pr = pull_request();
    pr.review_decision = Some(ReviewDecision::Approved);
    pr.checks_summary.failed = 1;

    assert_eq!(
        pull_request_row_rail_tone(&pr),
        PullRequestRowRailTone::Danger
    );

    pr.checks_summary.failed = 0;
    pr.checks_summary.pending = 1;

    assert_eq!(
        pull_request_row_rail_tone(&pr),
        PullRequestRowRailTone::Warning
    );
}

#[test]
fn closed_pull_request_is_not_ready_to_merge() {
    let mut pr = pull_request();
    pr.state = PullRequestState::Closed;

    assert!(!is_ready_to_merge(&pr));
    assert_eq!(
        pull_request_row_rail_tone(&pr),
        PullRequestRowRailTone::Neutral
    );
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

fn signal_summary(
    signals: &[PullRequestRowSignal],
) -> Vec<(PullRequestRowSignalKind, Option<String>)> {
    signals
        .iter()
        .map(|signal| (signal.kind, signal.label.clone()))
        .collect()
}
