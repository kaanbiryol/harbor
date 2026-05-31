use std::{
    collections::{HashMap, VecDeque},
    io::Write,
    process::{Command, Output, Stdio},
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
use super::response::{
    GitHubRateLimitMetadata, graphql_response_cost, json_value_from_body, parse_json_output,
    parse_response_metadata, response_metadata_from_headers,
};
use super::{OctocrabRequestFuture, OctocrabResponse};

const MAX_CONCURRENT_GITHUB_REQUESTS: usize = 4;
const MAX_REQUEST_ATTRIBUTION_HISTORY: usize = 100;
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

#[derive(Default)]
struct InFlightJsonRequest {
    result: Mutex<Option<Result<Value>>>,
    completed: Condvar,
}

enum JsonDedupeRole {
    Leader(Arc<InFlightJsonRequest>),
    Follower(Arc<InFlightJsonRequest>),
    Disabled,
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
            let output = Command::new("gh")
                .args(args)
                .output()
                .map_err(map_spawn_error)?;

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

    fn json_dedupe_role(&self, kind: GitHubRequestKind, read_key: Option<&str>) -> JsonDedupeRole {
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

    pub(super) fn latest_rate_limit(&self) -> Option<GitHubRateLimitStatus> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .latest_rate_limit
            .clone()
    }

    pub(super) fn latest_rate_limits(&self) -> Vec<GitHubRateLimitStatus> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .latest_rate_limits
            .values()
            .cloned()
            .collect()
    }

    pub(super) fn latest_request_attribution(&self) -> Option<GitHubRequestAttribution> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .latest_request_attribution
            .clone()
    }

    pub(super) fn recent_request_attributions(&self) -> Vec<GitHubRequestAttribution> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .recent_request_attributions
            .iter()
            .cloned()
            .collect()
    }

    pub(super) fn record_rate_limit_and_attribution(
        &self,
        family: GitHubApiFamily,
        operation_name: String,
        rate_limit: &GitHubRateLimitMetadata,
        duration: Duration,
        graphql_cost: Option<u64>,
    ) {
        let Some(rate_limit_status) = rate_limit.clone().into_status() else {
            return;
        };

        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let resource = rate_limit_status
            .resource
            .clone()
            .unwrap_or_else(|| family.label().to_string());
        let previous = state.latest_rate_limits.get(&resource);
        let spent = rate_limit_spent(previous, &rate_limit_status);

        let attribution = GitHubRequestAttribution {
            operation_name,
            family,
            resource: Some(resource.clone()),
            graphql_cost,
            remaining: rate_limit_status.remaining,
            limit: rate_limit_status.limit,
            used: rate_limit_status.used,
            spent,
            duration_ms: duration.as_millis().min(u128::from(u64::MAX)) as u64,
        };

        state.latest_rate_limit = Some(rate_limit_status.clone());
        state.latest_rate_limits.insert(resource, rate_limit_status);
        state.latest_request_attribution = Some(attribution.clone());
        state
            .recent_request_attributions
            .push_back(attribution.clone());
        while state.recent_request_attributions.len() > MAX_REQUEST_ATTRIBUTION_HISTORY {
            drop(state.recent_request_attributions.pop_front());
        }

        let graphql_expense = attribution.graphql_cost.or(attribution.spent);
        if attribution.family == GitHubApiFamily::GraphQl && graphql_expense.unwrap_or(0) >= 20 {
            tracing::warn!(
                operation = attribution.operation_name,
                graphql_cost = attribution.graphql_cost,
                spent = attribution.spent,
                remaining = attribution.remaining,
                limit = attribution.limit,
                duration_ms = attribution.duration_ms,
                "expensive github graphql request completed"
            );
        }

        if attribution.family == GitHubApiFamily::GraphQl {
            tracing::info!(
                operation = attribution.operation_name,
                graphql_cost = attribution.graphql_cost,
                spent = attribution.spent,
                remaining = attribution.remaining,
                limit = attribution.limit,
                duration_ms = attribution.duration_ms,
                "github graphql request completed"
            );
        }

        tracing::debug!(
            operation = attribution.operation_name,
            family = attribution.family.label(),
            resource = attribution.resource.as_deref(),
            graphql_cost = attribution.graphql_cost,
            spent = attribution.spent,
            remaining = attribution.remaining,
            limit = attribution.limit,
            duration_ms = attribution.duration_ms,
            "github request completed"
        );
    }

    fn remove_in_flight_json_read(&self, read_key: &str, in_flight: &Arc<InFlightJsonRequest>) {
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

impl InFlightJsonRequest {
    fn wait(&self) -> Result<Value> {
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

    fn complete(&self, result: Result<Value>) {
        *self
            .result
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(result);
        self.completed.notify_all();
    }
}

fn mutation_interval_elapsed(state: &GhCliRequestCoordinatorState) -> bool {
    mutation_interval_remaining(state).is_none()
}

fn rate_limit_spent(
    previous: Option<&GitHubRateLimitStatus>,
    current: &GitHubRateLimitStatus,
) -> Option<u64> {
    if let (Some(previous), Some(current_used)) = (previous, current.used)
        && let Some(previous_used) = previous.used
    {
        return current_used.checked_sub(previous_used);
    }

    if let (Some(previous), Some(current_remaining)) = (previous, current.remaining)
        && let Some(previous_remaining) = previous.remaining
    {
        return previous_remaining.checked_sub(current_remaining);
    }

    None
}

pub(super) fn mutation_interval_remaining(
    state: &GhCliRequestCoordinatorState,
) -> Option<Duration> {
    let elapsed = state.last_mutation_completed_at?.elapsed();

    MUTATION_REQUEST_INTERVAL.checked_sub(elapsed)
}

fn run_gh_command(args: Vec<String>, input: Option<String>) -> Result<Output> {
    let mut command = Command::new("gh");
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
