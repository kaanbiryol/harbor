use std::time::Duration;

use chrono::Utc;
use gpui::Context;
use harbor_sync::{
    SyncDecision, SyncReason, SyncSignals, SyncState, SyncTarget,
    checks_have_running_or_pending_work,
};

use crate::{
    actions::PanelTab,
    workspace::{AppView, PullRequestInboxCacheKey},
};

const IDLE_SYNC_LOOP_DELAY: Duration = Duration::from_secs(60);

impl AppView {
    pub(crate) fn ensure_sync_loop(&mut self, cx: &mut Context<Self>) {
        if self.tasks.sync_task.is_some() {
            return;
        }

        self.tasks.sync_task = Some(cx.spawn(async move |this, cx| {
            loop {
                let delay = match this.update(cx, |view, _| view.next_sync_delay()) {
                    Ok(delay) => delay,
                    Err(error) => {
                        tracing::warn!(%error, "failed to read next sync delay");
                        break;
                    }
                };

                cx.background_executor().timer(delay).await;

                if let Err(error) = this.update(cx, |view, cx| {
                    view.run_scheduled_active_inbox_sync(cx);
                    view.run_scheduled_selected_pull_request_sync(cx);
                }) {
                    tracing::warn!(%error, "failed to run scheduled sync");
                    break;
                }
            }
        }));
    }

    pub(crate) fn mark_sync_attempt(&mut self, target: SyncTarget) {
        self.sync_runtime
            .sync_states
            .entry(target)
            .or_default()
            .mark_attempt(Utc::now());
    }

    pub(crate) fn mark_sync_success(&mut self, target: SyncTarget) {
        self.sync_runtime
            .sync_states
            .entry(target)
            .or_default()
            .mark_success(Utc::now());
    }

    pub(crate) fn mark_sync_failure(&mut self, target: SyncTarget) {
        self.sync_runtime
            .sync_states
            .entry(target)
            .or_default()
            .mark_failure();
    }

    pub(crate) fn mark_active_inbox_stale(&mut self) {
        self.sync_runtime
            .sync_states
            .entry(self.pull_request_inbox.mode.active_sync_target())
            .or_default()
            .mark_stale();
    }

    fn run_scheduled_active_inbox_sync(&mut self, cx: &mut Context<Self>) {
        let Some(repository) = self.repository_state.configured_repo.clone() else {
            return;
        };
        if self.is_loading_prs {
            return;
        }

        let decision = self.active_inbox_sync_decision(SyncReason::Scheduled);
        if matches!(decision, SyncDecision::RunNow) {
            let key =
                PullRequestInboxCacheKey::new(repository.clone(), self.pull_request_inbox.mode);
            if self.pull_request_inbox.mode.active_sync_target() == SyncTarget::ActiveInbox {
                tracing::info!(
                    repository = %repository.full_name(),
                    mode = self.pull_request_inbox.mode.key(),
                    activity_state = ?self.sync_runtime.activity_state,
                    "github graphql source: scheduled active inbox refresh"
                );
            }
            self.spawn_pull_request_inbox_refresh(
                repository,
                self.pull_request_inbox.mode,
                key,
                false,
                cx,
            );
        }
    }

    fn next_sync_delay(&self) -> Duration {
        if self.repository_state.configured_repo.is_none() {
            return IDLE_SYNC_LOOP_DELAY;
        }

        let active_inbox_delay =
            self.sync_decision_delay(self.active_inbox_sync_decision(SyncReason::Scheduled));
        let selected_delay = self.next_selected_pull_request_sync_delay();

        active_inbox_delay.min(selected_delay)
    }

    fn sync_decision_delay(&self, decision: SyncDecision) -> Duration {
        match decision {
            SyncDecision::RunNow => Duration::from_secs(1),
            SyncDecision::Wait(delay) | SyncDecision::Backoff(delay) => delay,
            SyncDecision::SkipInFlight => Duration::from_secs(5),
        }
    }

    fn active_inbox_sync_decision(&self, reason: SyncReason) -> SyncDecision {
        self.sync_decision(self.pull_request_inbox.mode.active_sync_target(), reason)
    }

    fn sync_decision(&self, target: SyncTarget, reason: SyncReason) -> SyncDecision {
        let empty_state = SyncState::default();
        let state = self
            .sync_runtime
            .sync_states
            .get(&target)
            .unwrap_or(&empty_state);

        self.sync_runtime.sync_policy.decision(
            target,
            reason,
            self.sync_runtime.activity_state,
            state,
            self.sync_signals(),
            Utc::now(),
        )
    }

    fn sync_signals(&self) -> SyncSignals {
        SyncSignals {
            has_running_or_pending_checks: self.pull_requests.iter().any(|pull_request| {
                checks_have_running_or_pending_work(pull_request.checks_summary)
            }),
            has_running_workflows: harbor_sync::workflow_runs_have_running_work(
                &self.detail_state.workflow_runs,
            ),
            selected_pr_visible: self.selected_pull_request().is_some(),
        }
    }

    pub(crate) fn active_inbox_focus_catch_up_due(&self) -> bool {
        matches!(
            self.active_inbox_sync_decision(SyncReason::FocusGained),
            SyncDecision::RunNow
        )
    }

    fn run_scheduled_selected_pull_request_sync(&mut self, cx: &mut Context<Self>) {
        if self.sync_runtime.activity_state == harbor_sync::ActivityState::Background
            || self.selected_pull_request().is_none()
            || self.detail_state.detail_loading.details
            || self.detail_state.detail_loading.files
            || self.detail_state.detail_loading.checks
            || self.detail_state.detail_loading.workflows
            || self.detail_state.detail_loading.reviews
        {
            return;
        }

        if self.sync_decision(
            SyncTarget::SelectedPullRequestMetadata,
            SyncReason::Scheduled,
        ) == SyncDecision::RunNow
        {
            self.mark_sync_attempt(SyncTarget::SelectedPullRequestMetadata);
            self.refresh_selected_pull_request_metadata_only(cx);
            return;
        }

        let target = match self.active_tab {
            PanelTab::Review => Some(SyncTarget::SelectedPullRequestReviews),
            PanelTab::Checks => Some(SyncTarget::SelectedPullRequestChecks),
            PanelTab::Actions | PanelTab::Logs => Some(SyncTarget::SelectedPullRequestWorkflows),
            PanelTab::Diff => None,
        };

        let Some(target) = target else {
            return;
        };

        if self.sync_decision(target, SyncReason::Scheduled) != SyncDecision::RunNow {
            return;
        }

        self.mark_sync_attempt(target);
        match target {
            SyncTarget::SelectedPullRequestReviews => {
                self.detail_state.detail_loaded.reviews = false;
            }
            SyncTarget::SelectedPullRequestChecks => {
                self.detail_state.detail_loaded.checks = false;
            }
            SyncTarget::SelectedPullRequestWorkflows => {
                self.detail_state.detail_loaded.workflows = false;
            }
            SyncTarget::ActiveInbox | SyncTarget::SelectedPullRequestMetadata => {}
            SyncTarget::ActiveInboxLight | SyncTarget::ActiveInboxEnrichment => {}
        }
        self.load_active_panel_data_if_needed(cx);
    }

    fn next_selected_pull_request_sync_delay(&self) -> Duration {
        if self.sync_runtime.activity_state == harbor_sync::ActivityState::Background
            || self.selected_pull_request().is_none()
        {
            return IDLE_SYNC_LOOP_DELAY;
        }

        let mut delay = self.sync_decision_delay(self.sync_decision(
            SyncTarget::SelectedPullRequestMetadata,
            SyncReason::Scheduled,
        ));

        let active_panel_target = match self.active_tab {
            PanelTab::Review => Some(SyncTarget::SelectedPullRequestReviews),
            PanelTab::Checks => Some(SyncTarget::SelectedPullRequestChecks),
            PanelTab::Actions | PanelTab::Logs => Some(SyncTarget::SelectedPullRequestWorkflows),
            PanelTab::Diff => None,
        };

        if let Some(target) = active_panel_target {
            delay = delay
                .min(self.sync_decision_delay(self.sync_decision(target, SyncReason::Scheduled)));
        }

        delay
    }
}
