use std::time::Duration;

use chrono::{DateTime, Utc};
use harbor_domain::{ChecksSummary, WorkflowConclusion, WorkflowRun, WorkflowStatus};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActivityState {
    Focused,
    Background,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SyncTarget {
    ActiveInbox,
    ActiveInboxLight,
    ActiveInboxEnrichment,
    SelectedPullRequestMetadata,
    SelectedPullRequestReviews,
    SelectedPullRequestChecks,
    SelectedPullRequestWorkflows,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyncReason {
    Scheduled,
    Manual,
    Startup,
    RepositorySwitch,
    FocusGained,
    LocalMutation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyncDecision {
    RunNow,
    Wait(Duration),
    SkipInFlight,
    Backoff(Duration),
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SyncState {
    pub last_successful_fetch_at: Option<DateTime<Utc>>,
    pub last_attempt_at: Option<DateTime<Utc>>,
    pub failure_count: u32,
    pub stale: bool,
    pub in_flight: bool,
}

impl SyncState {
    pub fn mark_attempt(&mut self, now: DateTime<Utc>) {
        self.last_attempt_at = Some(now);
        self.in_flight = true;
    }

    pub fn mark_success(&mut self, now: DateTime<Utc>) {
        self.last_successful_fetch_at = Some(now);
        self.last_attempt_at = Some(now);
        self.failure_count = 0;
        self.stale = false;
        self.in_flight = false;
    }

    pub fn mark_failure(&mut self) {
        self.failure_count = self.failure_count.saturating_add(1);
        self.in_flight = false;
    }

    pub fn mark_stale(&mut self) {
        self.stale = true;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SyncSignals {
    pub has_running_or_pending_checks: bool,
    pub has_running_workflows: bool,
    pub selected_pr_visible: bool,
}

impl Default for SyncSignals {
    fn default() -> Self {
        Self {
            has_running_or_pending_checks: false,
            has_running_workflows: false,
            selected_pr_visible: true,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SyncPolicy {
    pub focused_interval: Duration,
    pub background_interval: Duration,
}

impl Default for SyncPolicy {
    fn default() -> Self {
        Self {
            focused_interval: Duration::from_secs(300),
            background_interval: Duration::from_secs(1_800),
        }
    }
}

impl SyncPolicy {
    pub fn decision(
        &self,
        _target: SyncTarget,
        reason: SyncReason,
        activity: ActivityState,
        state: &SyncState,
        _signals: SyncSignals,
        now: DateTime<Utc>,
    ) -> SyncDecision {
        if state.in_flight {
            return SyncDecision::SkipInFlight;
        }

        if matches!(
            reason,
            SyncReason::Manual
                | SyncReason::Startup
                | SyncReason::RepositorySwitch
                | SyncReason::LocalMutation
        ) || state.stale
        {
            return SyncDecision::RunNow;
        }

        if reason == SyncReason::FocusGained && self.is_due(state, self.focused_interval, now) {
            return SyncDecision::RunNow;
        }

        if let Some(backoff) = SyncBackoff::for_failure_count(state.failure_count)
            .and_then(|backoff| backoff.remaining_since(state.last_attempt_at, now))
        {
            return SyncDecision::Backoff(backoff);
        }

        let interval = self.interval(activity);

        match state.last_successful_fetch_at {
            None => SyncDecision::RunNow,
            Some(last_successful_fetch_at) => {
                let elapsed = now
                    .signed_duration_since(last_successful_fetch_at)
                    .to_std()
                    .unwrap_or_default();
                if elapsed >= interval {
                    SyncDecision::RunNow
                } else {
                    SyncDecision::Wait(interval - elapsed)
                }
            }
        }
    }

    pub fn interval(&self, activity: ActivityState) -> Duration {
        match activity {
            ActivityState::Focused => self.focused_interval,
            ActivityState::Background => self.background_interval,
        }
    }

    fn is_due(&self, state: &SyncState, interval: Duration, now: DateTime<Utc>) -> bool {
        state.last_successful_fetch_at.is_none_or(|last| {
            now.signed_duration_since(last).to_std().unwrap_or_default() >= interval
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SyncBackoff {
    delay: Duration,
}

impl SyncBackoff {
    pub fn for_failure_count(failure_count: u32) -> Option<Self> {
        if failure_count == 0 {
            return None;
        }

        let exponent = failure_count.saturating_sub(1).min(5);
        Some(Self {
            delay: Duration::from_secs(30 * 2_u64.pow(exponent)),
        })
    }

    pub fn remaining_since(
        self,
        last_attempt_at: Option<DateTime<Utc>>,
        now: DateTime<Utc>,
    ) -> Option<Duration> {
        let last_attempt_at = last_attempt_at?;
        let elapsed = now
            .signed_duration_since(last_attempt_at)
            .to_std()
            .unwrap_or_default();
        self.delay.checked_sub(elapsed)
    }
}

pub fn workflow_runs_have_running_work(workflow_runs: &[WorkflowRun]) -> bool {
    workflow_runs.iter().any(|run| {
        run.status != WorkflowStatus::Completed
            || matches!(
                run.conclusion,
                None | Some(WorkflowConclusion::ActionRequired)
            )
    })
}

pub fn checks_have_running_or_pending_work(summary: ChecksSummary) -> bool {
    summary.pending > 0
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, TimeZone, Utc};

    use super::*;

    #[test]
    fn focused_targets_refresh_every_five_minutes() {
        let policy = SyncPolicy::default();
        let now = time(10);
        let state = SyncState {
            last_successful_fetch_at: Some(time(6)),
            ..SyncState::default()
        };

        for target in [
            SyncTarget::ActiveInbox,
            SyncTarget::ActiveInboxLight,
            SyncTarget::ActiveInboxEnrichment,
            SyncTarget::SelectedPullRequestMetadata,
            SyncTarget::SelectedPullRequestReviews,
            SyncTarget::SelectedPullRequestChecks,
            SyncTarget::SelectedPullRequestWorkflows,
        ] {
            assert_eq!(
                policy.decision(
                    target,
                    SyncReason::Scheduled,
                    ActivityState::Focused,
                    &state,
                    SyncSignals::default(),
                    now,
                ),
                SyncDecision::Wait(Duration::from_secs(60))
            );

            assert_eq!(
                policy.decision(
                    target,
                    SyncReason::Scheduled,
                    ActivityState::Focused,
                    &SyncState {
                        last_successful_fetch_at: Some(time(5)),
                        ..SyncState::default()
                    },
                    SyncSignals::default(),
                    now,
                ),
                SyncDecision::RunNow
            );
        }
    }

    #[test]
    fn focus_catch_up_uses_focused_cadence_for_all_inbox_targets() {
        let policy = SyncPolicy::default();
        let now = time(10);

        for target in [SyncTarget::ActiveInbox, SyncTarget::ActiveInboxLight] {
            assert_eq!(
                policy.decision(
                    target,
                    SyncReason::FocusGained,
                    ActivityState::Focused,
                    &SyncState {
                        last_successful_fetch_at: Some(now - chrono::Duration::seconds(31)),
                        ..SyncState::default()
                    },
                    SyncSignals::default(),
                    now,
                ),
                SyncDecision::Wait(Duration::from_secs(269))
            );

            assert_eq!(
                policy.decision(
                    target,
                    SyncReason::FocusGained,
                    ActivityState::Focused,
                    &SyncState {
                        last_successful_fetch_at: Some(now - chrono::Duration::seconds(300)),
                        ..SyncState::default()
                    },
                    SyncSignals::default(),
                    now,
                ),
                SyncDecision::RunNow
            );
        }
    }

    #[test]
    fn background_targets_refresh_every_thirty_minutes() {
        let policy = SyncPolicy::default();
        let now = time(10);

        for target in [
            SyncTarget::ActiveInbox,
            SyncTarget::ActiveInboxLight,
            SyncTarget::ActiveInboxEnrichment,
            SyncTarget::SelectedPullRequestMetadata,
            SyncTarget::SelectedPullRequestReviews,
            SyncTarget::SelectedPullRequestChecks,
            SyncTarget::SelectedPullRequestWorkflows,
        ] {
            assert_eq!(
                policy.decision(
                    target,
                    SyncReason::Scheduled,
                    ActivityState::Background,
                    &SyncState {
                        last_successful_fetch_at: Some(now - chrono::Duration::seconds(1_740)),
                        ..SyncState::default()
                    },
                    SyncSignals::default(),
                    now,
                ),
                SyncDecision::Wait(Duration::from_secs(60))
            );

            assert_eq!(
                policy.decision(
                    target,
                    SyncReason::Scheduled,
                    ActivityState::Background,
                    &SyncState {
                        last_successful_fetch_at: Some(now - chrono::Duration::seconds(1_800)),
                        ..SyncState::default()
                    },
                    SyncSignals::default(),
                    now,
                ),
                SyncDecision::RunNow
            );
        }
    }

    #[test]
    fn running_checks_do_not_accelerate_refresh() {
        let policy = SyncPolicy::default();
        let now = time(10);

        assert_eq!(
            policy.decision(
                SyncTarget::SelectedPullRequestChecks,
                SyncReason::Scheduled,
                ActivityState::Focused,
                &SyncState {
                    last_successful_fetch_at: Some(now - chrono::Duration::seconds(44)),
                    ..SyncState::default()
                },
                SyncSignals {
                    has_running_or_pending_checks: true,
                    ..SyncSignals::default()
                },
                now,
            ),
            SyncDecision::Wait(Duration::from_secs(256))
        );

        assert_eq!(
            policy.decision(
                SyncTarget::SelectedPullRequestChecks,
                SyncReason::Scheduled,
                ActivityState::Focused,
                &SyncState {
                    last_successful_fetch_at: Some(now - chrono::Duration::seconds(300)),
                    ..SyncState::default()
                },
                SyncSignals {
                    has_running_or_pending_checks: true,
                    ..SyncSignals::default()
                },
                now,
            ),
            SyncDecision::RunNow
        );
    }

    #[test]
    fn selected_metadata_refreshes_every_five_minutes() {
        let policy = SyncPolicy::default();
        let now = time(10);

        assert_eq!(
            policy.decision(
                SyncTarget::SelectedPullRequestMetadata,
                SyncReason::Scheduled,
                ActivityState::Focused,
                &SyncState {
                    last_successful_fetch_at: Some(time(6)),
                    ..SyncState::default()
                },
                SyncSignals::default(),
                now,
            ),
            SyncDecision::Wait(Duration::from_secs(60))
        );

        assert_eq!(
            policy.decision(
                SyncTarget::SelectedPullRequestMetadata,
                SyncReason::Scheduled,
                ActivityState::Focused,
                &SyncState {
                    last_successful_fetch_at: Some(time(5)),
                    ..SyncState::default()
                },
                SyncSignals::default(),
                now,
            ),
            SyncDecision::RunNow
        );
    }

    #[test]
    fn manual_and_stale_sync_run_immediately() {
        let policy = SyncPolicy::default();
        let now = time(10);
        let fresh = SyncState {
            last_successful_fetch_at: Some(now),
            ..SyncState::default()
        };

        assert_eq!(
            policy.decision(
                SyncTarget::ActiveInbox,
                SyncReason::Manual,
                ActivityState::Focused,
                &fresh,
                SyncSignals::default(),
                now,
            ),
            SyncDecision::RunNow
        );

        assert_eq!(
            policy.decision(
                SyncTarget::ActiveInbox,
                SyncReason::Scheduled,
                ActivityState::Focused,
                &SyncState {
                    stale: true,
                    ..fresh
                },
                SyncSignals::default(),
                now,
            ),
            SyncDecision::RunNow
        );
    }

    #[test]
    fn failures_back_off_scheduled_work() {
        let policy = SyncPolicy::default();
        let now = time(10);
        let state = SyncState {
            last_successful_fetch_at: Some(time(1)),
            last_attempt_at: Some(now - chrono::Duration::seconds(10)),
            failure_count: 1,
            ..SyncState::default()
        };

        assert_eq!(
            policy.decision(
                SyncTarget::ActiveInbox,
                SyncReason::Scheduled,
                ActivityState::Focused,
                &state,
                SyncSignals::default(),
                now,
            ),
            SyncDecision::Backoff(Duration::from_secs(20))
        );
    }

    fn time(minute: i64) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 5, 1, 10, minute as u32, 0)
            .single()
            .expect("valid test time")
    }
}
