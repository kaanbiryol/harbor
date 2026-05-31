use gpui::{Context, Window};

use crate::{
    actions::SignOutOfGitHub,
    workspace::{AppView, async_updates::AppViewAsyncUpdateExt},
};

use super::{GitHubAuthSource, GitHubAuthStatus, GitHubCliAvailability};

pub(super) const GITHUB_CREDENTIAL_URL: &str = "harbor://github/oauth";
pub(super) const GITHUB_AUTH_SOURCE_CREDENTIAL_URL: &str = "harbor://github/auth-source";
pub(super) const GITHUB_AUTH_SOURCE_CREDENTIAL_USERNAME: &str = "github-auth-source";
pub(super) const GITHUB_OAUTH_CLIENT_ID_ENV: &str = "HARBOR_GITHUB_OAUTH_CLIENT_ID";

impl AppView {
    pub(crate) fn auth_status(&self) -> &GitHubAuthStatus {
        &self.auth_status
    }

    pub(crate) fn github_auth_gate_visible(&self) -> bool {
        matches!(
            self.auth_status(),
            GitHubAuthStatus::SignedOut
                | GitHubAuthStatus::MissingClientId
                | GitHubAuthStatus::SigningIn { .. }
        ) || (!self.github_api.has_auth()
            && !matches!(self.auth_status(), GitHubAuthStatus::SignedIn { .. }))
    }

    pub(crate) fn github_cli_availability(&self) -> &GitHubCliAvailability {
        &self.github_cli_availability
    }

    pub(crate) fn current_github_auth_source(&self) -> Option<GitHubAuthSource> {
        match self.auth_status() {
            GitHubAuthStatus::SignedIn { source, .. } => Some(*source),
            _ => None,
        }
    }

    pub(crate) fn github_oauth_unavailable_reason(&self) -> Option<&'static str> {
        github_oauth_client_id()
            .is_none()
            .then_some("Set HARBOR_GITHUB_OAUTH_CLIENT_ID to enable device login.")
    }

    pub(crate) fn github_auth_popover_open(&self) -> bool {
        self.github_auth_popover_open
            && matches!(
                self.auth_status,
                GitHubAuthStatus::SigningIn { .. }
                    | GitHubAuthStatus::SignedOut
                    | GitHubAuthStatus::MissingClientId
                    | GitHubAuthStatus::Failed(_)
                    | GitHubAuthStatus::SignedIn { .. }
            )
    }

    pub(crate) fn open_github_auth_popover(&mut self, cx: &mut Context<Self>) {
        self.github_auth_popover_open = true;
        cx.notify();
    }

    pub(crate) fn dismiss_github_auth_popover(&mut self, cx: &mut Context<Self>) {
        self.github_auth_popover_open = false;
        if self.github_api.has_auth() {
            cx.notify();
            return;
        }

        match &self.auth_status {
            GitHubAuthStatus::SigningIn { .. } => {
                self.status = "Waiting for GitHub authorization".to_string();
            }
            GitHubAuthStatus::MissingClientId | GitHubAuthStatus::Failed(_) => {
                self.auth_status = GitHubAuthStatus::SignedOut;
                self.show_github_sign_in_required();
            }
            GitHubAuthStatus::Loading
            | GitHubAuthStatus::SignedOut
            | GitHubAuthStatus::SignedIn { .. } => {}
        }
        cx.notify();
    }

    pub(super) fn load_github_credentials(&mut self, cx: &mut Context<Self>) {
        self.auth_status = GitHubAuthStatus::Loading;
        self.github_cli_availability = GitHubCliAvailability::Checking;
        self.github_auth_popover_open = false;
        let token_task = cx.read_credentials(GITHUB_CREDENTIAL_URL);
        let source_task = cx.read_credentials(GITHUB_AUTH_SOURCE_CREDENTIAL_URL);

        self.tasks.set_auth_task(cx.spawn(async move |this, cx| {
            let token_result = token_task
                .await
                .map(|credential| credential.map(|(_username, password)| password))
                .map_err(|error| error.to_string());
            let source_result = source_task
                .await
                .map(|credential| credential.map(|(_username, password)| password))
                .map_err(|error| error.to_string());

            this.update_or_log(cx, "failed to update github auth state", move |view, cx| {
                let source_result = saved_github_auth_source(source_result);
                match (token_result, source_result) {
                    (Ok(Some(password)), Ok(saved_source)) => {
                        let token = String::from_utf8(password)
                            .map_err(|error| error.to_string())
                            .and_then(|token| {
                                let source = saved_source
                                    .filter(|source| *source != GitHubAuthSource::GhCli)
                                    .unwrap_or(GitHubAuthSource::OAuth);
                                view.configure_github_token(token, source)?;
                                Ok(source)
                            });
                        match token {
                            Ok(source) => view.finish_authenticated_github_sign_in(source, cx),
                            Err(error) => {
                                view.auth_status = GitHubAuthStatus::Failed(error);
                                view.github_auth_popover_open = true;
                                if !view.github_api.has_auth() {
                                    view.show_github_sign_in_required();
                                }
                                view.status = "Failed to load GitHub credentials".to_string();
                            }
                        }
                    }
                    (Ok(None), Ok(Some(GitHubAuthSource::GhCli))) => {
                        view.sign_in_with_github_cli(true, cx);
                    }
                    (Ok(None), Ok(_)) => {
                        view.auth_status = GitHubAuthStatus::SignedOut;
                        if !view.github_api.has_auth() {
                            view.show_github_sign_in_required();
                        }
                        view.probe_github_cli_availability(cx);
                    }
                    (Err(error), _) | (_, Err(error)) => {
                        view.auth_status = GitHubAuthStatus::Failed(error);
                        if !view.github_api.has_auth() {
                            view.show_github_sign_in_required();
                        }
                        view.status = "Failed to load GitHub credentials".to_string();
                    }
                }

                cx.notify();
            });
        }));
    }

    pub(super) fn sign_out_of_github(
        &mut self,
        _: &SignOutOfGitHub,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let delete_token_task = cx.delete_credentials(GITHUB_CREDENTIAL_URL);
        let delete_source_task = cx.delete_credentials(GITHUB_AUTH_SOURCE_CREDENTIAL_URL);
        let clear_result = self.github_api.clear_auth();
        self.auth_status = GitHubAuthStatus::SignedOut;
        self.github_auth_popover_open = false;
        self.show_github_sign_in_required();
        self.probe_github_cli_availability(cx);
        self.status = match clear_result {
            Ok(()) => "Signed out of GitHub".to_string(),
            Err(error) => format!("Signed out locally, but failed to clear client: {error}"),
        };

        self.tasks.set_auth_task(cx.spawn(async move |this, cx| {
            let delete_token_result = delete_token_task.await;
            let delete_source_result = delete_source_task.await;
            this.update_or_log(
                cx,
                "failed to update github sign out state",
                move |view, cx| {
                    for result in [delete_token_result, delete_source_result] {
                        if let Err(error) = result {
                            let message = error.to_string();
                            if !is_missing_credential_error(&message) {
                                view.auth_status = GitHubAuthStatus::Failed(message.clone());
                                view.status =
                                    format!("Failed to remove GitHub credentials: {message}");
                                break;
                            }
                        }
                    }
                    cx.notify();
                },
            );
        }));

        cx.notify();
    }

    pub(super) fn finish_authenticated_github_sign_in(
        &mut self,
        source: GitHubAuthSource,
        cx: &mut Context<Self>,
    ) {
        self.auth_status = GitHubAuthStatus::SignedIn {
            login: None,
            source,
        };
        self.auth_switch_status = None;
        self.github_auth_popover_open = false;
        self.status = match source {
            GitHubAuthSource::OAuth => "Signed in to GitHub".to_string(),
            GitHubAuthSource::GhCli => "Signed in with GitHub CLI".to_string(),
        };
        self.load_recent_repositories(cx);
        self.refresh_authenticated_github_user(cx);
    }

    pub(super) fn show_github_sign_in_required(&mut self) {
        self.clear_authenticated_github_content();
        self.status = match &self.auth_status {
            GitHubAuthStatus::Loading => "Checking GitHub sign in...".to_string(),
            GitHubAuthStatus::SigningIn { .. } => {
                "Finish GitHub sign in in your browser".to_string()
            }
            GitHubAuthStatus::MissingClientId => {
                format!("Set {GITHUB_OAUTH_CLIENT_ID_ENV} to sign in with GitHub")
            }
            GitHubAuthStatus::Failed(error) => format!("GitHub sign in failed: {error}"),
            GitHubAuthStatus::SignedOut => {
                "Choose a GitHub sign-in method to load repositories".to_string()
            }
            GitHubAuthStatus::SignedIn { .. } => "Signed in to GitHub".to_string(),
        };
    }

    pub(super) fn configure_github_token(
        &self,
        token: String,
        source: GitHubAuthSource,
    ) -> std::result::Result<(), String> {
        self.github_api
            .configure_token(token, source)
            .map_err(|error| error.to_string())
    }

    fn refresh_authenticated_github_user(&self, cx: &mut Context<Self>) {
        let github_api = self.github_api.clone();

        cx.spawn(async move |this, cx| {
            let result = github_api.current_user().await;

            this.update_or_log(
                cx,
                "failed to update github account state",
                move |view, cx| match result {
                    Ok(login) => {
                        if let GitHubAuthStatus::SignedIn { source, .. } = view.auth_status {
                            if !view.github_api.has_auth() {
                                return;
                            }
                            view.auth_status = GitHubAuthStatus::SignedIn {
                                login: Some(login.clone()),
                                source,
                            };
                            view.review_state.current_user_login = Some(login);
                            cx.notify();
                        }
                    }
                    Err(error) => {
                        if matches!(view.auth_status, GitHubAuthStatus::SignedIn { .. }) {
                            tracing::warn!(%error, "failed to load signed-in github account");
                        }
                    }
                },
            );
        })
        .detach();
    }
}

fn saved_github_auth_source(
    credential: std::result::Result<Option<Vec<u8>>, String>,
) -> std::result::Result<Option<GitHubAuthSource>, String> {
    credential.and_then(|credential| match credential {
        Some(bytes) => GitHubAuthSource::from_credential_bytes(bytes),
        None => Ok(None),
    })
}

pub(super) fn github_oauth_client_id() -> Option<String> {
    std::env::var(GITHUB_OAUTH_CLIENT_ID_ENV)
        .ok()
        .filter(|client_id| !client_id.trim().is_empty())
}

pub(super) fn is_missing_credential_error(message: &str) -> bool {
    message.contains("-25300") || message.to_ascii_lowercase().contains("not found")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_auth_source_parses_credentials() {
        assert_eq!(
            saved_github_auth_source(Ok(Some(b"gh_cli".to_vec()))),
            Ok(Some(GitHubAuthSource::GhCli))
        );
        assert_eq!(
            saved_github_auth_source(Ok(Some(b"token".to_vec()))),
            Ok(Some(GitHubAuthSource::OAuth))
        );
        assert_eq!(saved_github_auth_source(Ok(None)), Ok(None));
        assert_eq!(
            saved_github_auth_source(Ok(Some(b"unknown".to_vec()))),
            Ok(None)
        );
    }
}
