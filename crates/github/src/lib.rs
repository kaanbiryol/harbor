mod client;
mod dto;
mod transport;

pub use client::GitHubClient;
pub use transport::{GhCliTransport, GitHubTransport};

pub type Result<T> = std::result::Result<T, GitHubError>;

#[derive(Debug, thiserror::Error)]
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
