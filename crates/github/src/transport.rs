use std::{
    collections::HashMap,
    io::{ErrorKind, Write},
    process::{Command, Output, Stdio},
    sync::{Arc, Condvar, Mutex},
    time::{Duration, Instant},
};

use async_trait::async_trait;
use serde_json::Value;

use crate::{GitHubError, GitHubRateLimit, GitHubRateLimitStatus, Result};

const MAX_CONCURRENT_GITHUB_REQUESTS: usize = 4;
const MUTATION_REQUEST_INTERVAL: Duration = Duration::from_secs(1);

#[async_trait]
pub trait GitHubTransport: Send + Sync {
    async fn rest_get(&self, path: &str, query: &[(&str, &str)]) -> Result<Value>;
    async fn rest_post(&self, path: &str, body: Value) -> Result<Value>;
    async fn rest_put(&self, path: &str, body: Value) -> Result<Value>;
    async fn workflow_run_log(&self, owner: &str, repo: &str, run_id: u64) -> Result<String>;
    async fn graphql(&self, query: &str, variables: Value) -> Result<Value>;

    fn latest_rate_limit(&self) -> Option<GitHubRateLimitStatus> {
        None
    }
}

#[derive(Clone)]
pub struct GhCliTransport {
    coordinator: Arc<GhCliRequestCoordinator>,
}

impl std::fmt::Debug for GhCliTransport {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("GhCliTransport")
    }
}

impl Default for GhCliTransport {
    fn default() -> Self {
        Self {
            coordinator: Arc::new(GhCliRequestCoordinator::default()),
        }
    }
}

impl GhCliTransport {
    pub async fn preflight(&self) -> Result<()> {
        run_status(vec!["--version".to_string()]).await?;
        run_status(vec!["auth".to_string(), "status".to_string()]).await
    }
}

#[async_trait]
impl GitHubTransport for GhCliTransport {
    async fn rest_get(&self, path: &str, query: &[(&str, &str)]) -> Result<Value> {
        let mut args = vec![
            "api".to_string(),
            "--include".to_string(),
            "--method".to_string(),
            "GET".to_string(),
            path.to_string(),
        ];

        for (key, value) in query {
            args.push("--raw-field".to_string());
            args.push(format!("{key}={value}"));
        }

        let read_key = rest_get_read_key(path, query);
        self.coordinator
            .run_json(GitHubRequestKind::Read, Some(read_key), args, None)
            .await
    }

    async fn rest_post(&self, path: &str, body: Value) -> Result<Value> {
        let args = vec![
            "api".to_string(),
            "--include".to_string(),
            "--method".to_string(),
            "POST".to_string(),
            path.to_string(),
            "--input".to_string(),
            "-".to_string(),
        ];

        self.coordinator
            .run_json(
                GitHubRequestKind::Mutation,
                None,
                args,
                Some(body.to_string()),
            )
            .await
    }

    async fn rest_put(&self, path: &str, body: Value) -> Result<Value> {
        let args = vec![
            "api".to_string(),
            "--include".to_string(),
            "--method".to_string(),
            "PUT".to_string(),
            path.to_string(),
            "--input".to_string(),
            "-".to_string(),
        ];

        self.coordinator
            .run_json(
                GitHubRequestKind::Mutation,
                None,
                args,
                Some(body.to_string()),
            )
            .await
    }

    async fn workflow_run_log(&self, owner: &str, repo: &str, run_id: u64) -> Result<String> {
        let args = vec![
            "run".to_string(),
            "view".to_string(),
            run_id.to_string(),
            "--repo".to_string(),
            format!("{owner}/{repo}"),
            "--log".to_string(),
        ];

        self.coordinator.run_text(args).await
    }

    async fn graphql(&self, query: &str, variables: Value) -> Result<Value> {
        let kind = if is_graphql_mutation(query) {
            GitHubRequestKind::Mutation
        } else {
            GitHubRequestKind::Read
        };
        let read_key =
            (kind == GitHubRequestKind::Read).then(|| graphql_read_key(query, &variables));

        if graphql_variables_need_input(&variables) {
            let args = vec![
                "api".to_string(),
                "graphql".to_string(),
                "--include".to_string(),
                "--input".to_string(),
                "-".to_string(),
            ];
            return self
                .coordinator
                .run_json(
                    kind,
                    read_key,
                    args,
                    Some(
                        serde_json::json!({
                            "query": query,
                            "variables": variables,
                        })
                        .to_string(),
                    ),
                )
                .await;
        }

        let mut args = vec![
            "api".to_string(),
            "graphql".to_string(),
            "--include".to_string(),
            "--raw-field".to_string(),
            format!("query={query}"),
        ];

        if let Some(variables) = variables.as_object() {
            for (key, value) in variables {
                let (flag, field) = graphql_field_arg(key, value)?;
                args.push(flag);
                args.push(field);
            }
        } else if !variables.is_null() {
            return Err(GitHubError::Transport(
                "graphql variables must be a JSON object".to_string(),
            ));
        }

        self.coordinator.run_json(kind, read_key, args, None).await
    }

    fn latest_rate_limit(&self) -> Option<GitHubRateLimitStatus> {
        self.coordinator.latest_rate_limit()
    }
}

async fn run_status(args: Vec<String>) -> Result<()> {
    smol::unblock(move || {
        let output = Command::new("gh")
            .args(args)
            .output()
            .map_err(map_spawn_error)?;

        if output.status.success() {
            Ok(())
        } else {
            Err(map_failed_status(&output.stdout, &output.stderr))
        }
    })
    .await
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GitHubRequestKind {
    Read,
    Mutation,
}

#[derive(Default)]
struct GhCliRequestCoordinator {
    state: Mutex<GhCliRequestCoordinatorState>,
    state_changed: Condvar,
}

#[derive(Default)]
struct GhCliRequestCoordinatorState {
    active_requests: usize,
    mutation_active: bool,
    last_mutation_completed_at: Option<Instant>,
    in_flight_json_reads: HashMap<String, Arc<InFlightJsonRequest>>,
    latest_rate_limit: Option<GitHubRateLimitStatus>,
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
    async fn run_json(
        self: &Arc<Self>,
        kind: GitHubRequestKind,
        read_key: Option<String>,
        args: Vec<String>,
        input: Option<String>,
    ) -> Result<Value> {
        let coordinator = self.clone();

        smol::unblock(move || coordinator.run_json_blocking(kind, read_key, args, input)).await
    }

    async fn run_text(self: &Arc<Self>, args: Vec<String>) -> Result<String> {
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

    fn run_json_blocking(
        &self,
        kind: GitHubRequestKind,
        read_key: Option<String>,
        args: Vec<String>,
        input: Option<String>,
    ) -> Result<Value> {
        match self.json_dedupe_role(kind, read_key.as_deref()) {
            JsonDedupeRole::Follower(in_flight) => in_flight.wait(),
            JsonDedupeRole::Leader(in_flight) => {
                let result = self.run_json_without_dedupe(kind, args, input);
                in_flight.complete(result.clone());
                if let Some(read_key) = read_key {
                    self.remove_in_flight_json_read(&read_key, &in_flight);
                }
                result
            }
            JsonDedupeRole::Disabled => self.run_json_without_dedupe(kind, args, input),
        }
    }

    fn run_json_without_dedupe(
        &self,
        kind: GitHubRequestKind,
        args: Vec<String>,
        input: Option<String>,
    ) -> Result<Value> {
        let _request_guard = self.acquire(kind);
        let output = run_gh_command(args, input)?;

        if !output.status.success() {
            self.record_rate_limit(parse_rate_limit_metadata(&output.stdout));
            return Err(map_failed_status(&output.stdout, &output.stderr));
        }

        let parsed = parse_json_output(&output.stdout)?;
        self.record_rate_limit(parsed.rate_limit.clone());
        if let Some(error) = graphql_rate_limit_error(&parsed.value, &parsed.rate_limit) {
            return Err(error);
        }

        Ok(parsed.value)
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

    fn latest_rate_limit(&self) -> Option<GitHubRateLimitStatus> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .latest_rate_limit
            .clone()
    }

    fn record_rate_limit(&self, rate_limit: GitHubRateLimitMetadata) {
        let Some(rate_limit) = rate_limit.into_status() else {
            return;
        };

        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .latest_rate_limit = Some(rate_limit);
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

fn mutation_interval_remaining(state: &GhCliRequestCoordinatorState) -> Option<Duration> {
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

struct ParsedJsonOutput {
    value: Value,
    rate_limit: GitHubRateLimitMetadata,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct GitHubRateLimitMetadata {
    retry_after_seconds: Option<u64>,
    reset_epoch_seconds: Option<u64>,
    resource: Option<String>,
    remaining: Option<u64>,
    limit: Option<u64>,
}

impl GitHubRateLimitMetadata {
    fn into_status(self) -> Option<GitHubRateLimitStatus> {
        (self.retry_after_seconds.is_some()
            || self.reset_epoch_seconds.is_some()
            || self.resource.is_some()
            || self.remaining.is_some()
            || self.limit.is_some())
        .then_some(GitHubRateLimitStatus {
            retry_after_seconds: self.retry_after_seconds,
            reset_epoch_seconds: self.reset_epoch_seconds,
            resource: self.resource,
            remaining: self.remaining,
            limit: self.limit,
        })
    }
}

fn parse_json_output(stdout: &[u8]) -> Result<ParsedJsonOutput> {
    if stdout.is_empty() {
        return Ok(ParsedJsonOutput {
            value: Value::Null,
            rate_limit: GitHubRateLimitMetadata::default(),
        });
    }

    let rate_limit = parse_rate_limit_metadata(stdout);
    let Some(json) = json_body(stdout) else {
        return Ok(ParsedJsonOutput {
            value: Value::Null,
            rate_limit,
        });
    };
    let json = json.trim_ascii();
    let value = if json.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(json).map_err(|error| GitHubError::Mapping(error.to_string()))?
    };

    Ok(ParsedJsonOutput { value, rate_limit })
}

fn json_body(stdout: &[u8]) -> Option<&[u8]> {
    stdout
        .iter()
        .position(|byte| matches!(byte, b'{' | b'['))
        .map(|index| &stdout[index..])
}

fn parse_rate_limit_metadata(stdout: &[u8]) -> GitHubRateLimitMetadata {
    let header_bytes = json_body(stdout)
        .and_then(|json_body| stdout.len().checked_sub(json_body.len()))
        .map_or(stdout, |json_start| &stdout[..json_start]);
    let mut metadata = GitHubRateLimitMetadata::default();

    for line in String::from_utf8_lossy(header_bytes).lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();

        match key.trim().to_ascii_lowercase().as_str() {
            "retry-after" => metadata.retry_after_seconds = value.parse().ok(),
            "x-ratelimit-reset" => metadata.reset_epoch_seconds = value.parse().ok(),
            "x-ratelimit-resource" => metadata.resource = Some(value.to_string()),
            "x-ratelimit-remaining" => metadata.remaining = value.parse().ok(),
            "x-ratelimit-limit" => metadata.limit = value.parse().ok(),
            _ => {}
        }
    }

    metadata
}

fn graphql_rate_limit_error(
    value: &Value,
    metadata: &GitHubRateLimitMetadata,
) -> Option<GitHubError> {
    let errors = value.get("errors")?.as_array()?;

    for error in errors {
        let message = error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("GitHub GraphQL request was rate limited");
        let error_type = error
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let lower_message = message.to_ascii_lowercase();

        if error_type.eq_ignore_ascii_case("RATE_LIMITED") || lower_message.contains("rate limit") {
            return Some(rate_limit_error(message.to_string(), metadata.clone()));
        }
    }

    None
}

fn rate_limit_error(message: String, metadata: GitHubRateLimitMetadata) -> GitHubError {
    let lower_message = message.to_ascii_lowercase();
    let limit = GitHubRateLimit {
        message,
        retry_after_seconds: metadata.retry_after_seconds,
        reset_epoch_seconds: metadata.reset_epoch_seconds,
        resource: metadata.resource,
        remaining: metadata.remaining,
        limit: metadata.limit,
    };

    if lower_message.contains("secondary") || lower_message.contains("abuse") {
        GitHubError::SecondaryRateLimited(limit)
    } else {
        GitHubError::RateLimited(limit)
    }
}

fn rest_get_read_key(path: &str, query: &[(&str, &str)]) -> String {
    let mut key = format!("rest-get:{path}");

    for (name, value) in query {
        key.push('\n');
        key.push_str(name);
        key.push('=');
        key.push_str(value);
    }

    key
}

fn graphql_read_key(query: &str, variables: &Value) -> String {
    format!("graphql:{query}\n{}", variables)
}

fn is_graphql_mutation(query: &str) -> bool {
    query.trim_start().starts_with("mutation")
}

fn graphql_field_arg(key: &str, value: &Value) -> Result<(String, String)> {
    let field = match value {
        Value::Null => (String::from("--field"), format!("{key}=null")),
        Value::Bool(value) => (String::from("--field"), format!("{key}={value}")),
        Value::Number(value) => (String::from("--field"), format!("{key}={value}")),
        Value::String(value) => (String::from("--raw-field"), format!("{key}={value}")),
        Value::Array(_) | Value::Object(_) => {
            return Err(GitHubError::Transport(format!(
                "complex graphql variable `{key}` is not supported by GhCliTransport yet"
            )));
        }
    };

    Ok(field)
}

fn graphql_variables_need_input(variables: &Value) -> bool {
    variables
        .as_object()
        .is_some_and(|variables| variables.values().any(value_is_complex))
}

fn value_is_complex(value: &Value) -> bool {
    matches!(value, Value::Array(_) | Value::Object(_))
}

fn map_spawn_error(error: std::io::Error) -> GitHubError {
    if error.kind() == ErrorKind::NotFound {
        GitHubError::MissingCli
    } else {
        GitHubError::Transport(error.to_string())
    }
}

fn map_failed_status(stdout: &[u8], stderr: &[u8]) -> GitHubError {
    let metadata = parse_rate_limit_metadata(stdout);
    let mut message = failure_message(stdout, stderr);
    if message.is_empty() && metadata.remaining == Some(0) {
        message = "GitHub rate limit exceeded".to_string();
    }
    let lower_message = message.to_lowercase();

    if lower_message.contains("not logged")
        || lower_message.contains("authentication")
        || lower_message.contains("gh auth login")
    {
        GitHubError::UnauthenticatedCli
    } else if lower_message.contains("rate limit")
        || lower_message.contains("too many requests")
        || metadata.remaining == Some(0)
    {
        rate_limit_error(message, metadata)
    } else if message.is_empty() {
        GitHubError::Transport("gh command exited with a non-zero status".to_string())
    } else {
        GitHubError::Transport(message)
    }
}

fn failure_message(stdout: &[u8], stderr: &[u8]) -> String {
    let stderr_message = String::from_utf8_lossy(stderr).trim().to_string();
    if !stderr_message.is_empty() {
        return stderr_message;
    }

    json_body(stdout)
        .and_then(|body| serde_json::from_slice::<Value>(body.trim_ascii()).ok())
        .and_then(|value| {
            value
                .get("message")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn parses_included_rate_limit_headers_and_json_body() {
        let output = concat!(
            "HTTP/2 200 OK\r\n",
            "x-ratelimit-limit: 5000\r\n",
            "x-ratelimit-remaining: 42\r\n",
            "x-ratelimit-reset: 1770000000\r\n",
            "x-ratelimit-resource: graphql\r\n",
            "retry-after: 5\r\n",
            "\r\n",
            "{\"data\":{\"viewer\":{\"login\":\"octocat\"}}}"
        );

        let parsed = parse_json_output(output.as_bytes()).unwrap();

        assert_eq!(
            parsed.value,
            json!({ "data": { "viewer": { "login": "octocat" } } })
        );
        assert_eq!(parsed.rate_limit.remaining, Some(42));
        assert_eq!(parsed.rate_limit.limit, Some(5000));
        assert_eq!(parsed.rate_limit.reset_epoch_seconds, Some(1_770_000_000));
        assert_eq!(parsed.rate_limit.resource.as_deref(), Some("graphql"));
        assert_eq!(parsed.rate_limit.retry_after_seconds, Some(5));
    }

    #[test]
    fn treats_header_only_success_as_null_json() {
        let output = concat!(
            "HTTP/2 204 No Content\r\n",
            "x-ratelimit-remaining: 41\r\n",
            "\r\n"
        );

        let parsed = parse_json_output(output.as_bytes()).unwrap();

        assert_eq!(parsed.value, Value::Null);
        assert_eq!(parsed.rate_limit.remaining, Some(41));
    }

    #[test]
    fn maps_graphql_primary_rate_limit_errors() {
        let metadata = GitHubRateLimitMetadata {
            retry_after_seconds: None,
            reset_epoch_seconds: Some(1_770_000_000),
            resource: Some("graphql".to_string()),
            remaining: Some(0),
            limit: Some(5000),
        };
        let value = json!({
            "errors": [
                {
                    "type": "RATE_LIMITED",
                    "message": "API rate limit exceeded"
                }
            ]
        });

        let error = graphql_rate_limit_error(&value, &metadata).unwrap();

        match error {
            GitHubError::RateLimited(limit) => {
                assert_eq!(limit.message, "API rate limit exceeded");
                assert_eq!(limit.reset_epoch_seconds, Some(1_770_000_000));
                assert_eq!(limit.remaining, Some(0));
                assert_eq!(limit.limit, Some(5000));
                assert_eq!(limit.resource.as_deref(), Some("graphql"));
            }
            other => panic!("expected primary rate limit error, got {other:?}"),
        }
    }

    #[test]
    fn records_latest_successful_rate_limit() {
        let coordinator = GhCliRequestCoordinator::default();
        coordinator.record_rate_limit(GitHubRateLimitMetadata {
            retry_after_seconds: None,
            reset_epoch_seconds: Some(1_770_000_000),
            resource: Some("graphql".to_string()),
            remaining: Some(42),
            limit: Some(5000),
        });

        let rate_limit = coordinator.latest_rate_limit().unwrap();

        assert_eq!(rate_limit.resource.as_deref(), Some("graphql"));
        assert_eq!(rate_limit.remaining, Some(42));
        assert_eq!(rate_limit.limit, Some(5000));
        assert_eq!(rate_limit.reset_epoch_seconds, Some(1_770_000_000));
    }

    #[test]
    fn mutation_interval_remaining_expires_without_overflow() {
        let state = GhCliRequestCoordinatorState {
            last_mutation_completed_at: Some(Instant::now() - MUTATION_REQUEST_INTERVAL * 2),
            ..Default::default()
        };

        assert_eq!(mutation_interval_remaining(&state), None);
    }

    #[test]
    fn maps_secondary_rate_limit_failures() {
        let error = map_failed_status(
            b"HTTP/2 403\r\nretry-after: 60\r\n\r\n",
            b"You have exceeded a secondary rate limit",
        );

        match error {
            GitHubError::SecondaryRateLimited(limit) => {
                assert_eq!(limit.retry_after_seconds, Some(60));
            }
            other => panic!("expected secondary rate limit error, got {other:?}"),
        }
    }
}
