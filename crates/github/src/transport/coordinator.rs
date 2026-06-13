use std::{
    collections::{HashMap, VecDeque},
    io::Write,
    process::{Output, Stdio},
    sync::{Arc, Condvar, Mutex},
    time::{Duration, Instant},
};

use http::StatusCode;
use serde_json::Value;

use crate::{
    ConditionalFetch, GitHubApiFamily, GitHubError, GitHubRateLimitStatus,
    GitHubRequestAttribution, Result,
};

use super::errors::{
    graphql_rate_limit_error, map_failed_status, map_http_failure, map_spawn_error,
};
use super::gh_command::gh_command;
use super::octocrab::{OctocrabRequestFuture, OctocrabResponse};
use super::response::{
    graphql_response_cost, json_value_from_body, parse_json_output, parse_response_metadata,
    response_metadata_from_headers,
};

mod dedupe;
mod rate_limit;

use dedupe::{InFlightJsonRequest, JsonDedupeRole};

const MAX_CONCURRENT_GITHUB_REQUESTS: usize = 4;
pub(super) const MUTATION_REQUEST_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum GitHubRequestKind {
    Read,
    Mutation,
}

#[derive(Default)]
pub(super) struct GhCliRequestCoordinator {
    state: Mutex<GhCliRequestCoordinatorState>,
    state_changed: Condvar,
}

#[derive(Default)]
pub(super) struct GhCliRequestCoordinatorState {
    active_requests: usize,
    mutation_active: bool,
    last_mutation_completed_at: Option<Instant>,
    in_flight_json_reads: HashMap<String, Arc<InFlightJsonRequest>>,
    latest_rate_limit: Option<GitHubRateLimitStatus>,
    latest_rate_limits: HashMap<String, GitHubRateLimitStatus>,
    latest_request_attribution: Option<GitHubRequestAttribution>,
    recent_request_attributions: VecDeque<GitHubRequestAttribution>,
}

#[cfg(test)]
impl GhCliRequestCoordinatorState {
    pub(super) fn with_last_mutation_completed_at(last_mutation_completed_at: Instant) -> Self {
        Self {
            last_mutation_completed_at: Some(last_mutation_completed_at),
            ..Default::default()
        }
    }
}

impl GhCliRequestCoordinator {
    pub(super) async fn run_json(
        self: &Arc<Self>,
        kind: GitHubRequestKind,
        family: GitHubApiFamily,
        operation_name: String,
        read_key: Option<String>,
        args: Vec<String>,
        input: Option<String>,
    ) -> Result<Value> {
        let coordinator = self.clone();

        smol::unblock(move || {
            coordinator.run_json_blocking(kind, family, operation_name, read_key, args, input)
        })
        .await
    }

    pub(super) async fn run_conditional_json(
        self: &Arc<Self>,
        kind: GitHubRequestKind,
        family: GitHubApiFamily,
        operation_name: String,
        read_key: Option<String>,
        args: Vec<String>,
        input: Option<String>,
    ) -> Result<ConditionalFetch<Value>> {
        let coordinator = self.clone();

        smol::unblock(move || {
            coordinator.run_conditional_json_blocking(
                kind,
                family,
                operation_name,
                read_key,
                args,
                input,
            )
        })
        .await
    }

    pub(super) async fn run_text(self: &Arc<Self>, args: Vec<String>) -> Result<String> {
        let coordinator = self.clone();

        smol::unblock(move || {
            let _request_guard = coordinator.acquire(GitHubRequestKind::Read);
            let output = gh_command().args(args).output().map_err(map_spawn_error)?;

            if !output.status.success() {
                return Err(map_failed_status(&output.stdout, &output.stderr));
            }

            String::from_utf8(output.stdout)
                .map_err(|error| GitHubError::Mapping(error.to_string()))
        })
        .await
    }

    pub(super) async fn run_octocrab_json<F>(
        self: &Arc<Self>,
        runtime: Arc<tokio::runtime::Runtime>,
        kind: GitHubRequestKind,
        family: GitHubApiFamily,
        operation_name: String,
        read_key: Option<String>,
        request: F,
    ) -> Result<Value>
    where
        F: FnOnce() -> OctocrabRequestFuture + Send + 'static,
    {
        let coordinator = self.clone();

        smol::unblock(move || {
            coordinator.run_octocrab_json_blocking(
                runtime,
                kind,
                family,
                operation_name,
                read_key,
                request,
            )
        })
        .await
    }

    pub(super) async fn run_octocrab_conditional_json<F>(
        self: &Arc<Self>,
        runtime: Arc<tokio::runtime::Runtime>,
        kind: GitHubRequestKind,
        family: GitHubApiFamily,
        operation_name: String,
        request: F,
    ) -> Result<ConditionalFetch<Value>>
    where
        F: FnOnce() -> OctocrabRequestFuture + Send + 'static,
    {
        let coordinator = self.clone();

        smol::unblock(move || {
            coordinator.run_octocrab_conditional_json_blocking(
                runtime,
                kind,
                family,
                operation_name,
                request,
            )
        })
        .await
    }

    pub(super) async fn run_octocrab_text<F, M>(
        self: &Arc<Self>,
        runtime: Arc<tokio::runtime::Runtime>,
        kind: GitHubRequestKind,
        family: GitHubApiFamily,
        operation_name: String,
        request: F,
        map_body: M,
    ) -> Result<String>
    where
        F: FnOnce() -> OctocrabRequestFuture + Send + 'static,
        M: FnOnce(&[u8]) -> Result<String> + Send + 'static,
    {
        let coordinator = self.clone();

        smol::unblock(move || {
            coordinator.run_octocrab_text_blocking(
                runtime,
                kind,
                family,
                operation_name,
                request,
                map_body,
            )
        })
        .await
    }

    fn run_json_blocking(
        &self,
        kind: GitHubRequestKind,
        family: GitHubApiFamily,
        operation_name: String,
        read_key: Option<String>,
        args: Vec<String>,
        input: Option<String>,
    ) -> Result<Value> {
        match self.json_dedupe_role(kind, read_key.as_deref()) {
            JsonDedupeRole::Follower(in_flight) => in_flight.wait(),
            JsonDedupeRole::Leader(in_flight) => {
                let result =
                    self.run_json_without_dedupe(kind, family, operation_name, args, input);
                in_flight.complete(result.clone());
                if let Some(read_key) = read_key {
                    self.remove_in_flight_json_read(&read_key, &in_flight);
                }
                result
            }
            JsonDedupeRole::Disabled => {
                self.run_json_without_dedupe(kind, family, operation_name, args, input)
            }
        }
    }

    fn run_octocrab_json_blocking<F>(
        &self,
        runtime: Arc<tokio::runtime::Runtime>,
        kind: GitHubRequestKind,
        family: GitHubApiFamily,
        operation_name: String,
        read_key: Option<String>,
        request: F,
    ) -> Result<Value>
    where
        F: FnOnce() -> OctocrabRequestFuture + Send + 'static,
    {
        match self.json_dedupe_role(kind, read_key.as_deref()) {
            JsonDedupeRole::Follower(in_flight) => in_flight.wait(),
            JsonDedupeRole::Leader(in_flight) => {
                let result = self.run_octocrab_json_without_dedupe(
                    runtime,
                    kind,
                    family,
                    operation_name,
                    request,
                );
                in_flight.complete(result.clone());
                if let Some(read_key) = read_key {
                    self.remove_in_flight_json_read(&read_key, &in_flight);
                }
                result
            }
            JsonDedupeRole::Disabled => self.run_octocrab_json_without_dedupe(
                runtime,
                kind,
                family,
                operation_name,
                request,
            ),
        }
    }

    fn run_octocrab_conditional_json_blocking<F>(
        &self,
        runtime: Arc<tokio::runtime::Runtime>,
        kind: GitHubRequestKind,
        family: GitHubApiFamily,
        operation_name: String,
        request: F,
    ) -> Result<ConditionalFetch<Value>>
    where
        F: FnOnce() -> OctocrabRequestFuture + Send + 'static,
    {
        let response =
            self.run_octocrab_response(runtime, kind, family, operation_name, request)?;
        let metadata = response_metadata_from_headers(response.status, &response.headers);

        if response.status == StatusCode::NOT_MODIFIED {
            return Ok(ConditionalFetch::NotModified {
                validator: metadata.validator,
            });
        }

        if !response.status.is_success() {
            return Err(map_http_failure(
                response.status,
                &response.headers,
                &response.body,
            ));
        }

        Ok(ConditionalFetch::Modified {
            value: json_value_from_body(&response.body)?,
            validator: metadata.validator,
        })
    }

    fn run_octocrab_text_blocking<F, M>(
        &self,
        runtime: Arc<tokio::runtime::Runtime>,
        kind: GitHubRequestKind,
        family: GitHubApiFamily,
        operation_name: String,
        request: F,
        map_body: M,
    ) -> Result<String>
    where
        F: FnOnce() -> OctocrabRequestFuture + Send + 'static,
        M: FnOnce(&[u8]) -> Result<String> + Send + 'static,
    {
        let response =
            self.run_octocrab_response(runtime, kind, family, operation_name, request)?;
        if !response.status.is_success() {
            return Err(map_http_failure(
                response.status,
                &response.headers,
                &response.body,
            ));
        }

        map_body(&response.body)
    }

    fn run_octocrab_json_without_dedupe<F>(
        &self,
        runtime: Arc<tokio::runtime::Runtime>,
        kind: GitHubRequestKind,
        family: GitHubApiFamily,
        operation_name: String,
        request: F,
    ) -> Result<Value>
    where
        F: FnOnce() -> OctocrabRequestFuture + Send + 'static,
    {
        let response =
            self.run_octocrab_response(runtime, kind, family, operation_name, request)?;
        let metadata = response_metadata_from_headers(response.status, &response.headers);

        if !response.status.is_success() {
            return Err(map_http_failure(
                response.status,
                &response.headers,
                &response.body,
            ));
        }

        let value = json_value_from_body(&response.body)?;
        if let Some(error) = graphql_rate_limit_error(&value, &metadata.rate_limit) {
            return Err(error);
        }

        Ok(value)
    }

    fn run_octocrab_response<F>(
        &self,
        runtime: Arc<tokio::runtime::Runtime>,
        kind: GitHubRequestKind,
        family: GitHubApiFamily,
        operation_name: String,
        request: F,
    ) -> Result<OctocrabResponse>
    where
        F: FnOnce() -> OctocrabRequestFuture + Send + 'static,
    {
        let _request_guard = self.acquire(kind);
        let started_at = Instant::now();
        let response = runtime.block_on(request())?;
        let metadata = response_metadata_from_headers(response.status, &response.headers);
        let graphql_cost = (family == GitHubApiFamily::GraphQl)
            .then(|| {
                json_value_from_body(&response.body)
                    .ok()
                    .and_then(|value| graphql_response_cost(&value))
            })
            .flatten();

        self.record_rate_limit_and_attribution(
            family,
            operation_name,
            &metadata.rate_limit,
            started_at.elapsed(),
            graphql_cost,
        );

        Ok(response)
    }

    fn run_conditional_json_blocking(
        &self,
        kind: GitHubRequestKind,
        family: GitHubApiFamily,
        operation_name: String,
        _read_key: Option<String>,
        args: Vec<String>,
        input: Option<String>,
    ) -> Result<ConditionalFetch<Value>> {
        self.run_conditional_json_without_dedupe(kind, family, operation_name, args, input)
    }

    fn run_json_without_dedupe(
        &self,
        kind: GitHubRequestKind,
        family: GitHubApiFamily,
        operation_name: String,
        args: Vec<String>,
        input: Option<String>,
    ) -> Result<Value> {
        let _request_guard = self.acquire(kind);
        let started_at = Instant::now();
        let output = run_gh_command(args, input)?;

        if !output.status.success() {
            let metadata = parse_response_metadata(&output.stdout);
            self.record_rate_limit_and_attribution(
                family,
                operation_name,
                &metadata.rate_limit,
                started_at.elapsed(),
                None,
            );
            return Err(map_failed_status(&output.stdout, &output.stderr));
        }

        let parsed = parse_json_output(&output.stdout)?;
        let graphql_cost =
            (family == GitHubApiFamily::GraphQl).then(|| graphql_response_cost(&parsed.value));
        self.record_rate_limit_and_attribution(
            family,
            operation_name,
            &parsed.metadata.rate_limit,
            started_at.elapsed(),
            graphql_cost.flatten(),
        );
        if let Some(error) = graphql_rate_limit_error(&parsed.value, &parsed.metadata.rate_limit) {
            return Err(error);
        }

        Ok(parsed.value)
    }

    fn run_conditional_json_without_dedupe(
        &self,
        kind: GitHubRequestKind,
        family: GitHubApiFamily,
        operation_name: String,
        args: Vec<String>,
        input: Option<String>,
    ) -> Result<ConditionalFetch<Value>> {
        let _request_guard = self.acquire(kind);
        let started_at = Instant::now();
        let output = run_gh_command(args, input)?;
        let parsed = parse_json_output(&output.stdout)?;

        self.record_rate_limit_and_attribution(
            family,
            operation_name,
            &parsed.metadata.rate_limit,
            started_at.elapsed(),
            None,
        );

        if parsed.metadata.status_code == Some(304) {
            return Ok(ConditionalFetch::NotModified {
                validator: parsed.metadata.validator,
            });
        }

        if !output.status.success() {
            return Err(map_failed_status(&output.stdout, &output.stderr));
        }

        Ok(ConditionalFetch::Modified {
            value: parsed.value,
            validator: parsed.metadata.validator,
        })
    }

    fn acquire(&self, kind: GitHubRequestKind) -> GhCliRequestGuard<'_> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        loop {
            let has_capacity = state.active_requests < MAX_CONCURRENT_GITHUB_REQUESTS;
            let mutation_ready = kind == GitHubRequestKind::Read
                || (!state.mutation_active && mutation_interval_elapsed(&state));

            if has_capacity && mutation_ready {
                state.active_requests += 1;
                if kind == GitHubRequestKind::Mutation {
                    state.mutation_active = true;
                }

                return GhCliRequestGuard {
                    coordinator: self,
                    kind,
                };
            }

            if kind == GitHubRequestKind::Mutation
                && has_capacity
                && !state.mutation_active
                && let Some(wait_duration) = mutation_interval_remaining(&state)
            {
                let (next_state, _) = self
                    .state_changed
                    .wait_timeout(state, wait_duration)
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                state = next_state;
            } else {
                state = self
                    .state_changed
                    .wait(state)
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
            }
        }
    }
}

struct GhCliRequestGuard<'a> {
    coordinator: &'a GhCliRequestCoordinator,
    kind: GitHubRequestKind,
}

impl Drop for GhCliRequestGuard<'_> {
    fn drop(&mut self) {
        let mut state = self
            .coordinator
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        state.active_requests = state.active_requests.saturating_sub(1);
        if self.kind == GitHubRequestKind::Mutation {
            state.mutation_active = false;
            state.last_mutation_completed_at = Some(Instant::now());
        }

        self.coordinator.state_changed.notify_all();
    }
}

fn mutation_interval_elapsed(state: &GhCliRequestCoordinatorState) -> bool {
    mutation_interval_remaining(state).is_none()
}

pub(super) fn mutation_interval_remaining(
    state: &GhCliRequestCoordinatorState,
) -> Option<Duration> {
    let elapsed = state.last_mutation_completed_at?.elapsed();

    MUTATION_REQUEST_INTERVAL.checked_sub(elapsed)
}

fn run_gh_command(args: Vec<String>, input: Option<String>) -> Result<Output> {
    let mut command = gh_command();
    command
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if input.is_some() {
        command.stdin(Stdio::piped());
    }

    let mut child = command.spawn().map_err(map_spawn_error)?;

    if let Some(input) = input
        && let Some(mut stdin) = child.stdin.take()
    {
        stdin
            .write_all(input.as_bytes())
            .map_err(|error| GitHubError::Transport(error.to_string()))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|error| GitHubError::Transport(error.to_string()))?;

    Ok(output)
}
