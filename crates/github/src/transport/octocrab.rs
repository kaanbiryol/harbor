use std::{future::Future, pin::Pin, sync::Arc};

use async_trait::async_trait;
use http::{HeaderMap, HeaderValue, StatusCode, header};
use http_body_util::BodyExt;
use octocrab::Octocrab;
use serde_json::Value;
use url::form_urlencoded;

use crate::{
    ConditionalFetch, GitHubApiFamily, GitHubError, GitHubRateLimitStatus,
    GitHubRequestAttribution, GitHubTransport, HttpCacheValidator, Result,
};

use super::{
    coordinator::{GhCliRequestCoordinator, GitHubRequestKind},
    errors::map_octocrab_error,
    graphql_operation_name, graphql_read_key, is_graphql_mutation,
    response::workflow_log_text_from_zip,
    rest_get_read_key, rest_operation_name,
};

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

pub(super) type OctocrabRequestFuture =
    Pin<Box<dyn Future<Output = Result<OctocrabResponse>> + Send>>;

pub(super) struct OctocrabResponse {
    pub(super) status: StatusCode,
    pub(super) headers: HeaderMap,
    pub(super) body: Vec<u8>,
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
