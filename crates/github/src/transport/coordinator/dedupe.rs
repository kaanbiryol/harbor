use std::sync::{Arc, Condvar, Mutex};

use serde_json::Value;

use crate::{GitHubError, Result};

use super::{GhCliRequestCoordinator, GitHubRequestKind};

#[derive(Default)]
pub(super) struct InFlightJsonRequest {
    result: Mutex<Option<Result<Value>>>,
    completed: Condvar,
}

pub(super) enum JsonDedupeRole {
    Leader(Arc<InFlightJsonRequest>),
    Follower(Arc<InFlightJsonRequest>),
    Disabled,
}

impl GhCliRequestCoordinator {
    pub(super) fn json_dedupe_role(
        &self,
        kind: GitHubRequestKind,
        read_key: Option<&str>,
    ) -> JsonDedupeRole {
        if kind != GitHubRequestKind::Read {
            return JsonDedupeRole::Disabled;
        }

        let Some(read_key) = read_key else {
            return JsonDedupeRole::Disabled;
        };

        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(in_flight) = state.in_flight_json_reads.get(read_key) {
            return JsonDedupeRole::Follower(in_flight.clone());
        }

        let in_flight = Arc::new(InFlightJsonRequest::default());
        state
            .in_flight_json_reads
            .insert(read_key.to_string(), in_flight.clone());

        JsonDedupeRole::Leader(in_flight)
    }

    pub(super) fn remove_in_flight_json_read(
        &self,
        read_key: &str,
        in_flight: &Arc<InFlightJsonRequest>,
    ) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        if state
            .in_flight_json_reads
            .get(read_key)
            .is_some_and(|current| Arc::ptr_eq(current, in_flight))
        {
            state.in_flight_json_reads.remove(read_key);
        }
    }
}

impl InFlightJsonRequest {
    pub(super) fn wait(&self) -> Result<Value> {
        let mut result = self
            .result
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        while result.is_none() {
            result = self
                .completed
                .wait(result)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
        }

        result.as_ref().cloned().unwrap_or_else(|| {
            Err(GitHubError::Transport(
                "in-flight request lost its result".into(),
            ))
        })
    }

    pub(super) fn complete(&self, result: Result<Value>) {
        *self
            .result
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(result);
        self.completed.notify_all();
    }
}
