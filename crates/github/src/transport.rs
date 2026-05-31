#[cfg(test)]
use std::time::{Duration, Instant};
use std::{future::Future, pin::Pin, process::Command, sync::Arc};

use async_trait::async_trait;
use http::{HeaderMap, HeaderValue, StatusCode, header};
use http_body_util::BodyExt;
use octocrab::Octocrab;
use serde_json::Value;
use url::form_urlencoded;

use crate::{
    ConditionalFetch, GitHubApiFamily, GitHubError, GitHubRateLimitStatus,
    GitHubRequestAttribution, HttpCacheValidator, Result,
};

mod coordinator;
mod errors;
mod oauth;
mod response;

use coordinator::{GhCliRequestCoordinator, GitHubRequestKind};
#[cfg(test)]
use coordinator::{
    GhCliRequestCoordinatorState, MUTATION_REQUEST_INTERVAL, mutation_interval_remaining,
};
#[cfg(test)]
use errors::graphql_rate_limit_error;
use errors::{map_failed_status, map_octocrab_error, map_spawn_error};
pub use oauth::{GitHubDeviceFlow, start_oauth_device_flow};
use response::workflow_log_text_from_zip;
#[cfg(test)]
use response::{GitHubRateLimitMetadata, parse_json_output};

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

#[derive(Clone, Debug)]
pub enum GitHubTransportSource {
    Token(OctocrabTransport),
    GhCli(GhCliTransport),
}

#[async_trait]
impl GitHubTransport for GitHubTransportSource {
    async fn rest_get(&self, path: &str, query: &[(&str, &str)]) -> Result<Value> {
        match self {
            Self::Token(transport) => transport.rest_get(path, query).await,
            Self::GhCli(transport) => transport.rest_get(path, query).await,
        }
    }

    async fn rest_get_conditional(
        &self,
        path: &str,
        query: &[(&str, &str)],
        validator: Option<&HttpCacheValidator>,
    ) -> Result<ConditionalFetch<Value>> {
        match self {
            Self::Token(transport) => transport.rest_get_conditional(path, query, validator).await,
            Self::GhCli(transport) => transport.rest_get_conditional(path, query, validator).await,
        }
    }

    async fn rest_post(&self, path: &str, body: Value) -> Result<Value> {
        match self {
            Self::Token(transport) => transport.rest_post(path, body).await,
            Self::GhCli(transport) => transport.rest_post(path, body).await,
        }
    }

    async fn rest_put(&self, path: &str, body: Value) -> Result<Value> {
        match self {
            Self::Token(transport) => transport.rest_put(path, body).await,
            Self::GhCli(transport) => transport.rest_put(path, body).await,
        }
    }

    async fn workflow_run_log(&self, owner: &str, repo: &str, run_id: u64) -> Result<String> {
        match self {
            Self::Token(transport) => transport.workflow_run_log(owner, repo, run_id).await,
            Self::GhCli(transport) => transport.workflow_run_log(owner, repo, run_id).await,
        }
    }

    async fn graphql(&self, query: &str, variables: Value) -> Result<Value> {
        match self {
            Self::Token(transport) => transport.graphql(query, variables).await,
            Self::GhCli(transport) => transport.graphql(query, variables).await,
        }
    }

    fn latest_rate_limit(&self) -> Option<GitHubRateLimitStatus> {
        match self {
            Self::Token(transport) => transport.latest_rate_limit(),
            Self::GhCli(transport) => transport.latest_rate_limit(),
        }
    }

    fn latest_rate_limits(&self) -> Vec<GitHubRateLimitStatus> {
        match self {
            Self::Token(transport) => transport.latest_rate_limits(),
            Self::GhCli(transport) => transport.latest_rate_limits(),
        }
    }

    fn latest_request_attribution(&self) -> Option<GitHubRequestAttribution> {
        match self {
            Self::Token(transport) => transport.latest_request_attribution(),
            Self::GhCli(transport) => transport.latest_request_attribution(),
        }
    }

    fn recent_request_attributions(&self) -> Vec<GitHubRequestAttribution> {
        match self {
            Self::Token(transport) => transport.recent_request_attributions(),
            Self::GhCli(transport) => transport.recent_request_attributions(),
        }
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

#[cfg(test)]
mod tests;
