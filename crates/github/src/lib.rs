use async_trait::async_trait;
use serde_json::Value;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, GitHubError>;

#[derive(Debug, Error)]
pub enum GitHubError {
    #[error("github cli is not installed or not available on PATH")]
    MissingCli,
    #[error("github cli is not authenticated")]
    UnauthenticatedCli,
    #[error("github transport failed: {0}")]
    Transport(String),
    #[error("github response could not be mapped into the domain model: {0}")]
    Mapping(String),
}

#[async_trait]
pub trait GitHubTransport: Send + Sync {
    async fn rest_get(&self, path: &str, query: &[(&str, &str)]) -> Result<Value>;
    async fn rest_post(&self, path: &str, body: Value) -> Result<Value>;
    async fn graphql(&self, query: &str, variables: Value) -> Result<Value>;
}

#[derive(Clone, Debug, Default)]
pub struct GhCliTransport;

#[async_trait]
impl GitHubTransport for GhCliTransport {
    async fn rest_get(&self, path: &str, _query: &[(&str, &str)]) -> Result<Value> {
        Err(GitHubError::Transport(format!(
            "GhCliTransport::rest_get is not implemented yet for {path}"
        )))
    }

    async fn rest_post(&self, path: &str, _body: Value) -> Result<Value> {
        Err(GitHubError::Transport(format!(
            "GhCliTransport::rest_post is not implemented yet for {path}"
        )))
    }

    async fn graphql(&self, _query: &str, _variables: Value) -> Result<Value> {
        Err(GitHubError::Transport(
            "GhCliTransport::graphql is not implemented yet".to_string(),
        ))
    }
}
