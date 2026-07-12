use std::collections::HashMap;

use chrono::Utc;
use harbor_sync::{ActivityState, SyncDecision, SyncPolicy, SyncReason, SyncState, SyncTarget};

pub(crate) struct SyncRuntimeState {
    activity_state: ActivityState,
    sync_policy: SyncPolicy,
    sync_states: HashMap<SyncTarget, SyncState>,
    did_focus: bool,
}

impl SyncRuntimeState {
    pub(crate) fn new(activity_state: ActivityState, sync_policy: SyncPolicy) -> Self {
        Self {
            activity_state,
            sync_policy,
            sync_states: HashMap::new(),
            did_focus: false,
        }
    }

    pub(crate) fn set_activity(&mut self, activity_state: ActivityState) {
        self.activity_state = activity_state;
    }

    pub(crate) fn activity_state(&self) -> ActivityState {
        self.activity_state
    }

    pub(crate) fn is_background(&self) -> bool {
        self.activity_state == ActivityState::Background
    }

    pub(crate) fn did_focus(&self) -> bool {
        self.did_focus
    }

    pub(crate) fn mark_focused_once(&mut self) {
        self.did_focus = true;
    }

    pub(crate) fn mark_attempt(&mut self, target: SyncTarget) {
        self.sync_states
            .entry(target)
            .or_default()
            .mark_attempt(Utc::now());
    }

    pub(crate) fn mark_success(&mut self, target: SyncTarget) {
        self.sync_states
            .entry(target)
            .or_default()
            .mark_success(Utc::now());
    }

    pub(crate) fn mark_failure(&mut self, target: SyncTarget) {
        self.sync_states.entry(target).or_default().mark_failure();
    }

    pub(crate) fn mark_stale(&mut self, target: SyncTarget) {
        self.sync_states.entry(target).or_default().mark_stale();
    }

    pub(crate) fn decision(&self, target: SyncTarget, reason: SyncReason) -> SyncDecision {
        let empty_state = SyncState::default();
        let state = self.sync_states.get(&target).unwrap_or(&empty_state);

        self.sync_policy
            .decision(reason, self.activity_state, state, Utc::now())
    }

    #[cfg(test)]
    pub(crate) fn sync_state(&self, target: SyncTarget) -> Option<&SyncState> {
        self.sync_states.get(&target)
    }

    #[cfg(test)]
    pub(crate) fn set_sync_state(&mut self, target: SyncTarget, state: SyncState) {
        self.sync_states.insert(target, state);
    }
}
