use std::{process::Command, sync::Arc};

use async_trait::async_trait;
use serde_json::Value;

use crate::{
    ConditionalFetch, GitHubApiFamily, GitHubError, GitHubRateLimitStatus,
    GitHubRequestAttribution, GitHubTransport, HttpCacheValidator, Result,
};

use super::{
    coordinator::{GhCliRequestCoordinator, GitHubRequestKind},
    errors::{map_failed_status, map_spawn_error},
    graphql_field_arg, graphql_operation_name, graphql_read_key, graphql_variables_need_input,
    is_graphql_mutation, rest_get_read_key, rest_operation_name,
};

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
