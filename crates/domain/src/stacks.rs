use serde::{Deserialize, Serialize};

use crate::{
    MergeMethod, MergeQueueState, PullRequest, PullRequestState,
    pull_request_signals::merge_blocker,
};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PullRequestStackMembership {
    pub number: u64,
    pub position: usize,
    pub size: usize,
    pub base_ref: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PullRequestStack {
    pub number: u64,
    pub base_ref: String,
    /// Ordered from the bottom (nearest the base branch) to the top.
    pub pull_requests: Vec<PullRequest>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StackMergeAction {
    Enqueue,
    Merge(MergeMethod),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StackMergeOutcome {
    Merged,
    Enqueued,
}

impl PullRequestStack {
    pub fn top_open_pull_request(&self) -> Option<&PullRequest> {
        self.pull_requests
            .iter()
            .rev()
            .find(|pull_request| pull_request.state == PullRequestState::Open)
    }

    pub fn open_count(&self) -> usize {
        self.pull_requests
            .iter()
            .filter(|pull_request| pull_request.state == PullRequestState::Open)
            .count()
    }

    pub fn uses_merge_queue(&self) -> bool {
        self.pull_requests
            .iter()
            .find(|pull_request| pull_request.state == PullRequestState::Open)
            .is_some_and(|pull_request| {
                matches!(
                    pull_request.merge_capabilities.queue_state,
                    MergeQueueState::Enabled | MergeQueueState::Queued
                )
            })
    }

    pub fn merge_blocker(&self) -> Option<String> {
        let Some(top) = self.top_open_pull_request() else {
            return Some("This stack has no open pull requests".into());
        };
        if top.merge_capabilities.queue_state == MergeQueueState::Queued {
            return Some(format!("PR #{} is already in the merge queue", top.number));
        }
        for pull_request in &self.pull_requests {
            if pull_request.state == PullRequestState::Merged
                || pull_request.merge_capabilities.queue_state == MergeQueueState::Queued
            {
                continue;
            }
            if let Some(blocker) = merge_blocker(pull_request) {
                return Some(blocker);
            }
            if pull_request.merge_capabilities.queue_state == MergeQueueState::Unknown {
                return Some("Loading stack merge queue settings from GitHub".into());
            }
            if !pull_request.merge_capabilities.viewer_can_queue {
                return Some("You do not have permission to merge this stack".into());
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MergeState, test_fixtures::pull_request};

    fn stack() -> PullRequestStack {
        let mut bottom = pull_request();
        bottom.merge_capabilities.queue_state = MergeQueueState::Enabled;
        bottom.merge_capabilities.viewer_can_queue = true;
        let mut top = bottom.clone();
        top.number = 8;
        top.merge_capabilities.queue_state = MergeQueueState::Disabled;
        PullRequestStack {
            number: 9,
            base_ref: "main".into(),
            pull_requests: vec![bottom, top],
        }
    }

    #[test]
    fn uses_the_trunk_queue_and_targets_the_top_open_pull_request() {
        let mut stack = stack();
        assert!(stack.uses_merge_queue());
        assert_eq!(stack.top_open_pull_request().unwrap().number, 8);
        assert_eq!(stack.open_count(), 2);
        assert_eq!(stack.merge_blocker(), None);
        stack.pull_requests[1].state = PullRequestState::Merged;
        assert_eq!(stack.top_open_pull_request().unwrap().number, 7);
        assert_eq!(stack.open_count(), 1);
    }

    #[test]
    fn blocks_the_whole_stack_when_a_lower_layer_is_not_ready() {
        let mut stack = stack();
        stack.pull_requests[0].merge_state = Some(MergeState::Dirty);
        assert!(
            stack
                .merge_blocker()
                .unwrap()
                .contains("#7 has merge conflicts")
        );
        stack.pull_requests[0].state = PullRequestState::Merged;
        assert_eq!(stack.merge_blocker(), None);
        stack.pull_requests[1].is_draft = true;
        assert!(stack.merge_blocker().unwrap().contains("draft"));
    }

    #[test]
    fn requires_known_permissions_and_stops_duplicate_enqueues() {
        let mut stack = stack();
        stack.pull_requests[0].merge_capabilities.queue_state = MergeQueueState::Queued;
        assert_eq!(stack.merge_blocker(), None);
        stack.pull_requests[1].merge_capabilities.queue_state = MergeQueueState::Queued;
        assert!(
            stack
                .merge_blocker()
                .unwrap()
                .contains("already in the merge queue")
        );
        stack.pull_requests[1].merge_capabilities.queue_state = MergeQueueState::Disabled;
        stack.pull_requests[0].merge_capabilities.queue_state = MergeQueueState::Unknown;
        assert!(stack.merge_blocker().unwrap().contains("Loading"));
        stack.pull_requests[0].merge_capabilities.queue_state = MergeQueueState::Enabled;
        stack.pull_requests[1].merge_capabilities.viewer_can_queue = false;
        assert!(stack.merge_blocker().unwrap().contains("permission"));
        stack.pull_requests[1].state = PullRequestState::Closed;
        assert!(stack.merge_blocker().unwrap().contains("not open"));
    }
}
