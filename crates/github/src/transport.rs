#[cfg(test)]
use std::time::{Duration, Instant};

use async_trait::async_trait;
use serde_json::Value;

#[cfg(test)]
use crate::GitHubApiFamily;
use crate::{
    ConditionalFetch, GitHubError, GitHubRateLimitStatus, GitHubRequestAttribution,
    HttpCacheValidator, Result,
};

mod coordinator;
mod errors;
mod gh_cli;
mod gh_command;
mod oauth;
mod octocrab;
mod response;

#[cfg(test)]
use coordinator::{
    GhCliRequestCoordinator, GhCliRequestCoordinatorState, MUTATION_REQUEST_INTERVAL,
    mutation_interval_remaining,
};
#[cfg(test)]
use errors::{graphql_rate_limit_error, map_failed_status};
pub use gh_cli::GhCliTransport;
pub use oauth::{GitHubDeviceFlow, start_oauth_device_flow};
pub use octocrab::OctocrabTransport;
#[cfg(test)]
use response::{GitHubRateLimitMetadata, parse_json_output, workflow_log_text_from_zip};

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
