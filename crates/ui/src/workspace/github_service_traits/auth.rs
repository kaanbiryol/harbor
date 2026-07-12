use harbor_github::{GitHubRateLimitStatus, Result};

use crate::workspace::GitHubAuthSource;

pub trait GitHubAuthApi: Send + Sync {
    fn configure_token(&self, token: String, source: GitHubAuthSource) -> Result<()>;
    fn configure_gh_cli(&self) -> Result<()>;
    fn clear_auth(&self) -> Result<()>;
    fn has_auth(&self) -> bool;
    fn latest_rate_limit(&self) -> Option<GitHubRateLimitStatus>;
}
