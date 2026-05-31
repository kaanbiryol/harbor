use harbor_github::{
    GhCliTransport, GitHubClient, GitHubError, GitHubTransportSource, OctocrabTransport, Result,
};

use super::{GitHubAuthApi, GitHubAuthSource, RealGitHubApi};

impl RealGitHubApi {
    pub(super) fn client(&self) -> Result<GitHubClient<GitHubTransportSource>> {
        self.client
            .lock()
            .map_err(|error| GitHubError::Transport(error.to_string()))?
            .clone()
            .ok_or(GitHubError::Unauthenticated)
    }

    pub(super) fn cached_current_user_login(&self) -> Result<Option<String>> {
        self.current_user_login
            .lock()
            .map(|login| login.clone())
            .map_err(|error| GitHubError::Transport(error.to_string()))
    }

    pub(super) fn cache_current_user_login(&self, login: String) -> Result<()> {
        self.current_user_login
            .lock()
            .map(|mut cached_login| {
                *cached_login = Some(login);
            })
            .map_err(|error| GitHubError::Transport(error.to_string()))
    }

    fn clear_current_user_login(&self) -> Result<()> {
        self.current_user_login
            .lock()
            .map(|mut login| {
                *login = None;
            })
            .map_err(|error| GitHubError::Transport(error.to_string()))
    }
}

impl GitHubAuthApi for RealGitHubApi {
    fn configure_token(&self, token: String, _source: GitHubAuthSource) -> Result<()> {
        let transport = OctocrabTransport::with_token(token)?;
        let mut client = self
            .client
            .lock()
            .map_err(|error| GitHubError::Transport(error.to_string()))?;
        *client = Some(GitHubClient::new(GitHubTransportSource::Token(transport)));
        self.clear_current_user_login()?;
        Ok(())
    }

    fn configure_gh_cli(&self) -> Result<()> {
        let mut client = self
            .client
            .lock()
            .map_err(|error| GitHubError::Transport(error.to_string()))?;
        *client = Some(GitHubClient::new(GitHubTransportSource::GhCli(
            GhCliTransport::default(),
        )));
        self.clear_current_user_login()?;
        Ok(())
    }

    fn clear_auth(&self) -> Result<()> {
        self.client
            .lock()
            .map(|mut client| {
                *client = None;
            })
            .map_err(|error| GitHubError::Transport(error.to_string()))?;
        self.clear_current_user_login()?;
        Ok(())
    }

    fn has_auth(&self) -> bool {
        self.has_configured_client()
    }
}
