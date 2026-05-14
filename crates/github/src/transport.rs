use std::{
    collections::{HashMap, VecDeque},
    future::Future,
    io::{Cursor, ErrorKind, Read, Write},
    pin::Pin,
    process::{Command, Output, Stdio},
    sync::{Arc, Condvar, Mutex},
    time::{Duration, Instant},
};

use async_trait::async_trait;
use http::{HeaderMap, HeaderValue, StatusCode, header};
use http_body_util::BodyExt;
use octocrab::{Octocrab, auth::DeviceCodes};
use secrecy::{ExposeSecret, SecretString};
use serde_json::Value;
use url::form_urlencoded;
use zip::ZipArchive;

use crate::{
    ConditionalFetch, GitHubApiFamily, GitHubError, GitHubRateLimit, GitHubRateLimitStatus,
    GitHubRequestAttribution, HttpCacheValidator, Result,
};

const MAX_CONCURRENT_GITHUB_REQUESTS: usize = 4;
const MAX_REQUEST_ATTRIBUTION_HISTORY: usize = 100;
const MUTATION_REQUEST_INTERVAL: Duration = Duration::from_secs(1);

#[async_trait]
pub trait GitHubTransport: Send + Sync {
    async fn rest_get(&self, path: &str, query: &[(&str, &str)]) -> Result<Value>;
    async fn rest_get_conditional(
        &self,
        path: &str,
        query: &[(&str, &str)],
        _validator: Option<&HttpCacheValidator>,
    ) -> Result<ConditionalFetch<Value>> {
        let value = self.rest_get(path, query).await?;
        Ok(ConditionalFetch::Modified {
            value,
            validator: None,
        })
    }
    async fn rest_post(&self, path: &str, body: Value) -> Result<Value>;
    async fn rest_put(&self, path: &str, body: Value) -> Result<Value>;
    async fn workflow_run_log(&self, owner: &str, repo: &str, run_id: u64) -> Result<String>;
    async fn graphql(&self, query: &str, variables: Value) -> Result<Value>;

    fn latest_rate_limit(&self) -> Option<GitHubRateLimitStatus> {
        None
    }

    fn latest_rate_limits(&self) -> Vec<GitHubRateLimitStatus> {
        self.latest_rate_limit().into_iter().collect()
    }

    fn latest_request_attribution(&self) -> Option<GitHubRequestAttribution> {
        None
    }

    fn recent_request_attributions(&self) -> Vec<GitHubRequestAttribution> {
        self.latest_request_attribution().into_iter().collect()
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

#[derive(Clone)]
pub struct OctocrabTransport {
    client: Octocrab,
    coordinator: Arc<GhCliRequestCoordinator>,
    runtime: Arc<tokio::runtime::Runtime>,
}

impl std::fmt::Debug for OctocrabTransport {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("OctocrabTransport")
    }
}

impl OctocrabTransport {
    pub fn with_token(token: impl Into<String>) -> Result<Self> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("harbor-github")
            .worker_threads(2)
            .build()
            .map_err(|error| GitHubError::Transport(error.to_string()))?;
        let client = runtime.block_on(async {
            Octocrab::builder()
                .personal_token(token.into())
                .build()
                .map_err(map_octocrab_error)
        })?;

        Ok(Self {
            client,
            coordinator: Arc::new(GhCliRequestCoordinator::default()),
            runtime: Arc::new(runtime),
        })
    }

    pub async fn preflight(&self) -> Result<()> {
        self.rest_get("/user", &[]).await.map(drop)
    }
}

#[derive(Clone)]
pub struct GitHubDeviceFlow {
    client_id: String,
    codes: DeviceCodes,
}

impl GitHubDeviceFlow {
    pub fn user_code(&self) -> &str {
        &self.codes.user_code
    }

    pub fn verification_uri(&self) -> &str {
        &self.codes.verification_uri
    }

    pub fn expires_in(&self) -> u64 {
        self.codes.expires_in
    }

    pub fn interval(&self) -> u64 {
        self.codes.interval
    }

    pub async fn poll_for_token(self) -> Result<String> {
        smol::unblock(move || {
            let runtime = oauth_runtime()?;
            runtime.block_on(async move {
                let client_id = SecretString::from(self.client_id);
                let crab = oauth_device_crab()?;
                let auth = self
                    .codes
                    .poll_until_available(&crab, &client_id)
                    .await
                    .map_err(map_octocrab_error)?;

                Ok(auth.access_token.expose_secret().to_string())
            })
        })
        .await
    }
}

pub async fn start_oauth_device_flow(client_id: impl Into<String>) -> Result<GitHubDeviceFlow> {
    let client_id = client_id.into();
    smol::unblock(move || {
        let runtime = oauth_runtime()?;
        runtime.block_on(async move {
            let secret_client_id = SecretString::from(client_id.clone());
            let crab = oauth_device_crab()?;
            let codes = crab
                .authenticate_as_device(&secret_client_id, ["repo", "read:org"])
                .await
                .map_err(map_octocrab_error)?;

            Ok(GitHubDeviceFlow { client_id, codes })
        })
    })
    .await
}

fn oauth_runtime() -> Result<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("harbor-github-oauth")
        .worker_threads(1)
        .build()
        .map_err(|error| GitHubError::Transport(error.to_string()))
}

fn oauth_device_crab() -> Result<Octocrab> {
    Octocrab::builder()
        .base_uri("https://github.com")
        .map_err(map_octocrab_error)?
        .add_header(header::ACCEPT, "application/json".to_string())
        .build()
        .map_err(map_octocrab_error)
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
            .run_json(
                GitHubRequestKind::Read,
                GitHubApiFamily::Rest,
                rest_operation_name("GET", path),
                Some(read_key),
                args,
                None,
            )
            .await
    }

    async fn rest_get_conditional(
        &self,
        path: &str,
        query: &[(&str, &str)],
        validator: Option<&HttpCacheValidator>,
    ) -> Result<ConditionalFetch<Value>> {
        let mut args = vec![
            "api".to_string(),
            "--include".to_string(),
            "--method".to_string(),
            "GET".to_string(),
            path.to_string(),
        ];

        if let Some(etag) = validator.and_then(|validator| validator.etag.as_deref()) {
            args.push("--header".to_string());
            args.push(format!("If-None-Match: {etag}"));
        }
        if let Some(last_modified) =
            validator.and_then(|validator| validator.last_modified.as_deref())
        {
            args.push("--header".to_string());
            args.push(format!("If-Modified-Since: {last_modified}"));
        }

        for (key, value) in query {
            args.push("--raw-field".to_string());
            args.push(format!("{key}={value}"));
        }

        let read_key = rest_get_read_key(path, query);
        self.coordinator
            .run_conditional_json(
                GitHubRequestKind::Read,
                GitHubApiFamily::Rest,
                rest_operation_name("GET", path),
                Some(read_key),
                args,
                None,
            )
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
                GitHubApiFamily::Rest,
                rest_operation_name("POST", path),
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
                GitHubApiFamily::Rest,
                rest_operation_name("PUT", path),
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
                    GitHubApiFamily::GraphQl,
                    graphql_operation_name(query),
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

        self.coordinator
            .run_json(
                kind,
                GitHubApiFamily::GraphQl,
                graphql_operation_name(query),
                read_key,
                args,
                None,
            )
            .await
    }

    fn latest_rate_limit(&self) -> Option<GitHubRateLimitStatus> {
        self.coordinator.latest_rate_limit()
    }

    fn latest_rate_limits(&self) -> Vec<GitHubRateLimitStatus> {
        self.coordinator.latest_rate_limits()
    }

    fn latest_request_attribution(&self) -> Option<GitHubRequestAttribution> {
        self.coordinator.latest_request_attribution()
    }

    fn recent_request_attributions(&self) -> Vec<GitHubRequestAttribution> {
        self.coordinator.recent_request_attributions()
    }
}

type OctocrabRequestFuture = Pin<Box<dyn Future<Output = Result<OctocrabResponse>> + Send>>;

struct OctocrabResponse {
    status: StatusCode,
    headers: HeaderMap,
    body: Vec<u8>,
}

#[async_trait]
impl GitHubTransport for OctocrabTransport {
    async fn rest_get(&self, path: &str, query: &[(&str, &str)]) -> Result<Value> {
        let uri = path_with_query(path, query);
        let read_key = rest_get_read_key(path, query);
        let client = self.client.clone();
        let runtime = self.runtime.clone();
        self.coordinator
            .run_octocrab_json(
                runtime,
                GitHubRequestKind::Read,
                GitHubApiFamily::Rest,
                rest_operation_name("GET", path),
                Some(read_key),
                move || {
                    Box::pin(async move {
                        let response = client._get(uri).await.map_err(map_octocrab_error)?;
                        octocrab_response(response).await
                    })
                },
            )
            .await
    }

    async fn rest_get_conditional(
        &self,
        path: &str,
        query: &[(&str, &str)],
        validator: Option<&HttpCacheValidator>,
    ) -> Result<ConditionalFetch<Value>> {
        let uri = path_with_query(path, query);
        let headers = conditional_headers(validator)?;
        let client = self.client.clone();
        let runtime = self.runtime.clone();
        self.coordinator
            .run_octocrab_conditional_json(
                runtime,
                GitHubRequestKind::Read,
                GitHubApiFamily::Rest,
                rest_operation_name("GET", path),
                move || {
                    Box::pin(async move {
                        let response = client
                            ._get_with_headers(uri, Some(headers))
                            .await
                            .map_err(map_octocrab_error)?;
                        octocrab_response(response).await
                    })
                },
            )
            .await
    }

    async fn rest_post(&self, path: &str, body: Value) -> Result<Value> {
        let path = path.to_string();
        let client = self.client.clone();
        let runtime = self.runtime.clone();
        self.coordinator
            .run_octocrab_json(
                runtime,
                GitHubRequestKind::Mutation,
                GitHubApiFamily::Rest,
                rest_operation_name("POST", &path),
                None,
                move || {
                    Box::pin(async move {
                        let response = client
                            ._post(path, Some(&body))
                            .await
                            .map_err(map_octocrab_error)?;
                        octocrab_response(response).await
                    })
                },
            )
            .await
    }

    async fn rest_put(&self, path: &str, body: Value) -> Result<Value> {
        let path = path.to_string();
        let client = self.client.clone();
        let runtime = self.runtime.clone();
        self.coordinator
            .run_octocrab_json(
                runtime,
                GitHubRequestKind::Mutation,
                GitHubApiFamily::Rest,
                rest_operation_name("PUT", &path),
                None,
                move || {
                    Box::pin(async move {
                        let response = client
                            ._put(path, Some(&body))
                            .await
                            .map_err(map_octocrab_error)?;
                        octocrab_response(response).await
                    })
                },
            )
            .await
    }

    async fn workflow_run_log(&self, owner: &str, repo: &str, run_id: u64) -> Result<String> {
        let path = format!("/repos/{owner}/{repo}/actions/runs/{run_id}/logs");
        let client = self.client.clone();
        let runtime = self.runtime.clone();
        self.coordinator
            .run_octocrab_text(
                runtime,
                GitHubRequestKind::Read,
                GitHubApiFamily::Rest,
                rest_operation_name("GET", &path),
                move || {
                    Box::pin(async move {
                        let response = client._get(path).await.map_err(map_octocrab_error)?;
                        let response = client
                            .follow_location_to_data(response)
                            .await
                            .map_err(map_octocrab_error)?;
                        octocrab_response(response).await
                    })
                },
                workflow_log_text_from_zip,
            )
            .await
    }

    async fn graphql(&self, query: &str, variables: Value) -> Result<Value> {
        let kind = if is_graphql_mutation(query) {
            GitHubRequestKind::Mutation
        } else {
            GitHubRequestKind::Read
        };
        let read_key =
            (kind == GitHubRequestKind::Read).then(|| graphql_read_key(query, &variables));
        let body = serde_json::json!({
            "query": query,
            "variables": variables,
        });
        let client = self.client.clone();
        let runtime = self.runtime.clone();
        self.coordinator
            .run_octocrab_json(
                runtime,
                kind,
                GitHubApiFamily::GraphQl,
                graphql_operation_name(query),
                read_key,
                move || {
                    Box::pin(async move {
                        let response = client
                            ._post("/graphql", Some(&body))
                            .await
                            .map_err(map_octocrab_error)?;
                        octocrab_response(response).await
                    })
                },
            )
            .await
    }

    fn latest_rate_limit(&self) -> Option<GitHubRateLimitStatus> {
        self.coordinator.latest_rate_limit()
    }

    fn latest_rate_limits(&self) -> Vec<GitHubRateLimitStatus> {
        self.coordinator.latest_rate_limits()
    }

    fn latest_request_attribution(&self) -> Option<GitHubRequestAttribution> {
        self.coordinator.latest_request_attribution()
    }

    fn recent_request_attributions(&self) -> Vec<GitHubRequestAttribution> {
        self.coordinator.recent_request_attributions()
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
    latest_rate_limits: HashMap<String, GitHubRateLimitStatus>,
    latest_request_attribution: Option<GitHubRequestAttribution>,
    recent_request_attributions: VecDeque<GitHubRequestAttribution>,
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

    async fn run_conditional_json(
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

    async fn run_octocrab_json<F>(
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

    async fn run_octocrab_conditional_json<F>(
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

    async fn run_octocrab_text<F, M>(
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

    fn latest_rate_limit(&self) -> Option<GitHubRateLimitStatus> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .latest_rate_limit
            .clone()
    }

    fn latest_rate_limits(&self) -> Vec<GitHubRateLimitStatus> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .latest_rate_limits
            .values()
            .cloned()
            .collect()
    }

    fn latest_request_attribution(&self) -> Option<GitHubRequestAttribution> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .latest_request_attribution
            .clone()
    }

    fn recent_request_attributions(&self) -> Vec<GitHubRequestAttribution> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .recent_request_attributions
            .iter()
            .cloned()
            .collect()
    }

    fn record_rate_limit_and_attribution(
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
    metadata: GitHubResponseMetadata,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct GitHubRateLimitMetadata {
    retry_after_seconds: Option<u64>,
    reset_epoch_seconds: Option<u64>,
    resource: Option<String>,
    remaining: Option<u64>,
    limit: Option<u64>,
    used: Option<u64>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct GitHubResponseMetadata {
    status_code: Option<u16>,
    validator: Option<HttpCacheValidator>,
    rate_limit: GitHubRateLimitMetadata,
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
            used: self.used,
        })
    }
}

fn parse_json_output(stdout: &[u8]) -> Result<ParsedJsonOutput> {
    let metadata = parse_response_metadata(stdout);
    if stdout.is_empty() {
        return Ok(ParsedJsonOutput {
            value: Value::Null,
            metadata,
        });
    }

    let Some(json) = json_body(stdout) else {
        return Ok(ParsedJsonOutput {
            value: Value::Null,
            metadata,
        });
    };
    let json = json.trim_ascii();
    let value = if json.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(json).map_err(|error| GitHubError::Mapping(error.to_string()))?
    };

    Ok(ParsedJsonOutput { value, metadata })
}

fn json_body(stdout: &[u8]) -> Option<&[u8]> {
    stdout
        .iter()
        .position(|byte| matches!(byte, b'{' | b'['))
        .map(|index| &stdout[index..])
}

fn parse_rate_limit_metadata(stdout: &[u8]) -> GitHubRateLimitMetadata {
    parse_response_metadata(stdout).rate_limit
}

fn parse_response_metadata(stdout: &[u8]) -> GitHubResponseMetadata {
    let header_bytes = json_body(stdout)
        .and_then(|json_body| stdout.len().checked_sub(json_body.len()))
        .map_or(stdout, |json_start| &stdout[..json_start]);
    let mut rate_limit = GitHubRateLimitMetadata::default();
    let mut etag = None;
    let mut last_modified = None;
    let mut status_code = None;

    for line in String::from_utf8_lossy(header_bytes).lines() {
        if let Some(status) = http_status_code(line) {
            status_code = Some(status);
            continue;
        }

        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();

        match key.trim().to_ascii_lowercase().as_str() {
            "etag" => etag = Some(value.to_string()),
            "last-modified" => last_modified = Some(value.to_string()),
            "retry-after" => rate_limit.retry_after_seconds = value.parse().ok(),
            "x-ratelimit-reset" => rate_limit.reset_epoch_seconds = value.parse().ok(),
            "x-ratelimit-resource" => rate_limit.resource = Some(value.to_string()),
            "x-ratelimit-remaining" => rate_limit.remaining = value.parse().ok(),
            "x-ratelimit-limit" => rate_limit.limit = value.parse().ok(),
            "x-ratelimit-used" => rate_limit.used = value.parse().ok(),
            _ => {}
        }
    }

    let validator = Some(HttpCacheValidator {
        etag,
        last_modified,
    })
    .filter(|validator| !validator.is_empty());

    GitHubResponseMetadata {
        status_code,
        validator,
        rate_limit,
    }
}

fn response_metadata_from_headers(
    status: StatusCode,
    headers: &HeaderMap,
) -> GitHubResponseMetadata {
    let etag = header_string(headers, header::ETAG);
    let last_modified = header_string(headers, header::LAST_MODIFIED);
    let validator = Some(HttpCacheValidator {
        etag,
        last_modified,
    })
    .filter(|validator| !validator.is_empty());

    GitHubResponseMetadata {
        status_code: Some(status.as_u16()),
        validator,
        rate_limit: GitHubRateLimitMetadata {
            retry_after_seconds: header_u64(headers, header::RETRY_AFTER),
            reset_epoch_seconds: header_u64(headers, "x-ratelimit-reset"),
            resource: header_string(headers, "x-ratelimit-resource"),
            remaining: header_u64(headers, "x-ratelimit-remaining"),
            limit: header_u64(headers, "x-ratelimit-limit"),
            used: header_u64(headers, "x-ratelimit-used"),
        },
    }
}

fn header_string<K>(headers: &HeaderMap, key: K) -> Option<String>
where
    K: header::AsHeaderName,
{
    headers
        .get(key)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
}

fn header_u64<K>(headers: &HeaderMap, key: K) -> Option<u64>
where
    K: header::AsHeaderName,
{
    headers.get(key)?.to_str().ok()?.parse().ok()
}

fn json_value_from_body(body: &[u8]) -> Result<Value> {
    let body = body.trim_ascii();
    if body.is_empty() {
        return Ok(Value::Null);
    }

    serde_json::from_slice(body).map_err(|error| GitHubError::Mapping(error.to_string()))
}

async fn octocrab_response(
    response: http::Response<http_body_util::combinators::BoxBody<bytes::Bytes, octocrab::Error>>,
) -> Result<OctocrabResponse> {
    let status = response.status();
    let headers = response.headers().clone();
    let body = response
        .into_body()
        .collect()
        .await
        .map_err(map_octocrab_error)?
        .to_bytes()
        .to_vec();

    Ok(OctocrabResponse {
        status,
        headers,
        body,
    })
}

fn conditional_headers(validator: Option<&HttpCacheValidator>) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();

    if let Some(etag) = validator.and_then(|validator| validator.etag.as_deref()) {
        headers.insert(
            header::IF_NONE_MATCH,
            HeaderValue::from_str(etag)
                .map_err(|error| GitHubError::Transport(error.to_string()))?,
        );
    }
    if let Some(last_modified) = validator.and_then(|validator| validator.last_modified.as_deref())
    {
        headers.insert(
            header::IF_MODIFIED_SINCE,
            HeaderValue::from_str(last_modified)
                .map_err(|error| GitHubError::Transport(error.to_string()))?,
        );
    }

    Ok(headers)
}

fn path_with_query(path: &str, query: &[(&str, &str)]) -> String {
    if query.is_empty() {
        return path.to_string();
    }

    let mut encoded = form_urlencoded::Serializer::new(String::new());
    for (key, value) in query {
        encoded.append_pair(key, value);
    }

    let separator = if path.contains('?') { '&' } else { '?' };
    format!("{path}{separator}{}", encoded.finish())
}

fn workflow_log_text_from_zip(body: &[u8]) -> Result<String> {
    let reader = Cursor::new(body);
    let mut archive =
        ZipArchive::new(reader).map_err(|error| GitHubError::Mapping(error.to_string()))?;
    let mut entries = Vec::new();

    for index in 0..archive.len() {
        let mut file = archive
            .by_index(index)
            .map_err(|error| GitHubError::Mapping(error.to_string()))?;
        if file.is_dir() {
            continue;
        }

        let name = file.name().to_string();
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)
            .map_err(|error| GitHubError::Mapping(error.to_string()))?;
        let text = String::from_utf8_lossy(&bytes).to_string();
        entries.push((name, text));
    }

    entries.sort_by(|left, right| left.0.cmp(&right.0));

    let mut output = String::new();
    for (_, text) in entries {
        if !output.is_empty() && !output.ends_with('\n') {
            output.push('\n');
        }
        output.push_str(&text);
        if !output.ends_with('\n') {
            output.push('\n');
        }
    }

    Ok(output)
}

fn http_status_code(line: &str) -> Option<u16> {
    let mut parts = line.split_whitespace();
    let protocol = parts.next()?;
    if !protocol.starts_with("HTTP/") {
        return None;
    }

    parts.next()?.parse().ok()
}

fn graphql_response_cost(value: &Value) -> Option<u64> {
    value
        .pointer("/data/rateLimit/cost")
        .and_then(Value::as_u64)
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
        used: metadata.used,
    };

    if lower_message.contains("secondary") || lower_message.contains("abuse") {
        GitHubError::SecondaryRateLimited(Box::new(limit))
    } else {
        GitHubError::RateLimited(Box::new(limit))
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

fn rest_operation_name(method: &str, path: &str) -> String {
    format!("{method} {path}")
}

fn graphql_read_key(query: &str, variables: &Value) -> String {
    format!("graphql:{query}\n{}", variables)
}

fn graphql_operation_name(query: &str) -> String {
    let mut saw_operation_keyword = false;
    for token in query
        .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .filter(|token| !token.is_empty())
    {
        if saw_operation_keyword {
            return token.to_string();
        }

        if matches!(token, "query" | "mutation" | "subscription") {
            saw_operation_keyword = true;
        }
    }

    if is_graphql_mutation(query) {
        "GraphQL mutation".to_string()
    } else {
        "GraphQL query".to_string()
    }
}

fn is_graphql_mutation(query: &str) -> bool {
    query.trim_start().starts_with("mutation")
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

fn map_octocrab_error(error: octocrab::Error) -> GitHubError {
    match &error {
        octocrab::Error::GitHub { source, .. }
            if source.status_code == StatusCode::UNAUTHORIZED =>
        {
            GitHubError::Unauthenticated
        }
        octocrab::Error::GitHub { source, .. } => {
            let metadata = GitHubRateLimitMetadata::default();
            let message = source.message.clone();
            if message.to_ascii_lowercase().contains("rate limit") {
                rate_limit_error(message, metadata)
            } else {
                GitHubError::Transport(source.message.clone())
            }
        }
        _ => GitHubError::Transport(error.to_string()),
    }
}

fn map_http_failure(status: StatusCode, headers: &HeaderMap, body: &[u8]) -> GitHubError {
    let metadata = response_metadata_from_headers(status, headers).rate_limit;
    let mut message = failure_message_from_body(body);
    if message.is_empty() && metadata.remaining == Some(0) {
        message = "GitHub rate limit exceeded".to_string();
    }
    let lower_message = message.to_ascii_lowercase();

    if status == StatusCode::UNAUTHORIZED
        || status == StatusCode::FORBIDDEN && lower_message.contains("bad credentials")
    {
        GitHubError::Unauthenticated
    } else if lower_message.contains("rate limit")
        || lower_message.contains("too many requests")
        || metadata.remaining == Some(0)
    {
        rate_limit_error(message, metadata)
    } else if message.is_empty() {
        GitHubError::Transport(format!("github request failed with HTTP {status}"))
    } else {
        GitHubError::Transport(message)
    }
}

fn failure_message_from_body(body: &[u8]) -> String {
    serde_json::from_slice::<Value>(body.trim_ascii())
        .ok()
        .and_then(|value| {
            value
                .get("message")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_default()
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
        GitHubError::Unauthenticated
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
    use zip::{ZipWriter, write::SimpleFileOptions};

    use super::*;

    #[test]
    fn parses_included_rate_limit_headers_and_json_body() {
        let output = concat!(
            "HTTP/2 200 OK\r\n",
            "x-ratelimit-limit: 5000\r\n",
            "x-ratelimit-remaining: 42\r\n",
            "x-ratelimit-reset: 1770000000\r\n",
            "x-ratelimit-resource: graphql\r\n",
            "x-ratelimit-used: 12\r\n",
            "retry-after: 5\r\n",
            "\r\n",
            "{\"data\":{\"viewer\":{\"login\":\"octocat\"}}}"
        );

        let parsed = parse_json_output(output.as_bytes()).unwrap();

        assert_eq!(
            parsed.value,
            json!({ "data": { "viewer": { "login": "octocat" } } })
        );
        assert_eq!(parsed.metadata.status_code, Some(200));
        assert_eq!(parsed.metadata.rate_limit.remaining, Some(42));
        assert_eq!(parsed.metadata.rate_limit.limit, Some(5000));
        assert_eq!(
            parsed.metadata.rate_limit.reset_epoch_seconds,
            Some(1_770_000_000)
        );
        assert_eq!(
            parsed.metadata.rate_limit.resource.as_deref(),
            Some("graphql")
        );
        assert_eq!(parsed.metadata.rate_limit.used, Some(12));
        assert_eq!(parsed.metadata.rate_limit.retry_after_seconds, Some(5));
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
        assert_eq!(parsed.metadata.status_code, Some(204));
        assert_eq!(parsed.metadata.rate_limit.remaining, Some(41));
    }

    #[test]
    fn parses_conditional_not_modified_metadata() {
        let output = concat!(
            "HTTP/2 304 Not Modified\r\n",
            "etag: \"abc\"\r\n",
            "last-modified: Wed, 01 May 2026 10:00:00 GMT\r\n",
            "x-ratelimit-resource: core\r\n",
            "x-ratelimit-remaining: 4999\r\n",
            "\r\n"
        );

        let parsed = parse_json_output(output.as_bytes()).unwrap();

        assert_eq!(parsed.value, Value::Null);
        assert_eq!(parsed.metadata.status_code, Some(304));
        assert_eq!(
            parsed.metadata.validator,
            Some(HttpCacheValidator {
                etag: Some("\"abc\"".to_string()),
                last_modified: Some("Wed, 01 May 2026 10:00:00 GMT".to_string()),
            })
        );
        assert_eq!(parsed.metadata.rate_limit.resource.as_deref(), Some("core"));
        assert_eq!(parsed.metadata.rate_limit.remaining, Some(4999));
    }

    #[test]
    fn maps_graphql_primary_rate_limit_errors() {
        let metadata = GitHubRateLimitMetadata {
            retry_after_seconds: None,
            reset_epoch_seconds: Some(1_770_000_000),
            resource: Some("graphql".to_string()),
            remaining: Some(0),
            limit: Some(5000),
            used: None,
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
        coordinator.record_rate_limit_and_attribution(
            GitHubApiFamily::GraphQl,
            "HarborRepositoryPullRequests".to_string(),
            &GitHubRateLimitMetadata {
                retry_after_seconds: None,
                reset_epoch_seconds: Some(1_770_000_000),
                resource: Some("graphql".to_string()),
                remaining: Some(42),
                limit: Some(5000),
                used: Some(12),
            },
            Duration::from_millis(25),
            Some(7),
        );

        let rate_limit = coordinator.latest_rate_limit().unwrap();

        assert_eq!(rate_limit.resource.as_deref(), Some("graphql"));
        assert_eq!(rate_limit.remaining, Some(42));
        assert_eq!(rate_limit.limit, Some(5000));
        assert_eq!(rate_limit.reset_epoch_seconds, Some(1_770_000_000));
        assert_eq!(rate_limit.used, Some(12));

        let attribution = coordinator.latest_request_attribution().unwrap();
        assert_eq!(attribution.operation_name, "HarborRepositoryPullRequests");
        assert_eq!(attribution.family, GitHubApiFamily::GraphQl);
        assert_eq!(attribution.graphql_cost, Some(7));
        assert_eq!(attribution.duration_ms, 25);

        let recent = coordinator.recent_request_attributions();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].operation_name, "HarborRepositoryPullRequests");
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

    #[test]
    fn extracts_workflow_log_archive_in_name_order() {
        let mut archive = ZipWriter::new(Cursor::new(Vec::new()));
        archive
            .start_file("2_test.txt", SimpleFileOptions::default())
            .unwrap();
        archive.write_all(b"test\n").unwrap();
        archive
            .start_file("1_build.txt", SimpleFileOptions::default())
            .unwrap();
        archive.write_all(b"build").unwrap();
        let bytes = archive.finish().unwrap().into_inner();

        let text = workflow_log_text_from_zip(&bytes).unwrap();

        assert_eq!(text, "build\ntest\n");
    }
}
