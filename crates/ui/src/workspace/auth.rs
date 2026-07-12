use gpui::{AppContext, Context, Window};
use harbor_storage::SqliteStore;

use crate::{
    actions::SignOutOfGitHub,
    workspace::{AppView, async_updates::AppViewAsyncUpdateExt},
};

use super::{GitHubAuthSource, GitHubAuthStatus, GitHubCliAvailability};

pub(super) const GITHUB_CREDENTIAL_URL: &str = "harbor://github/oauth";
pub(super) const GITHUB_AUTH_SOURCE_SETTING_KEY: &str = "github.auth_source";
pub(super) const GITHUB_OAUTH_CLIENT_ID_ENV: &str = "HARBOR_GITHUB_OAUTH_CLIENT_ID";
const DEFAULT_GITHUB_OAUTH_CLIENT_ID: &str = "Ov23liH046TDAFHhtJEU";

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
        let store = self.repository_state.store();
        let source_task =
            cx.background_spawn(async move { load_saved_github_auth_source(store).await });

        self.tasks.set_auth_task(cx.spawn(async move |this, cx| {
            let source_result = source_task.await;

            this.update_or_log(cx, "failed to update github auth state", move |view, cx| {
                match source_result {
                    Ok(Some(GitHubAuthSource::GhCli)) => {
                        view.sign_in_with_github_cli(true, cx);
                    }
                    Ok(source @ (Some(GitHubAuthSource::OAuth) | None)) => {
                        view.load_github_oauth_credentials(source, cx);
                    }
                    Err(error) => {
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

    fn load_github_oauth_credentials(
        &mut self,
        saved_source: Option<GitHubAuthSource>,
        cx: &mut Context<Self>,
    ) {
        let token_task = cx.read_credentials(GITHUB_CREDENTIAL_URL);

        self.tasks.set_auth_task(cx.spawn(async move |this, cx| {
            let token_result = token_task
                .await
                .map(|credential| credential.map(|(_username, password)| password))
                .map_err(|error| error.to_string());

            this.update_or_log(
                cx,
                "failed to update github oauth state",
                move |view, cx| {
                    match token_result {
                        Ok(Some(password)) => {
                            let token = String::from_utf8(password)
                                .map_err(|error| error.to_string())
                                .and_then(|token| {
                                    let source = saved_source.unwrap_or(GitHubAuthSource::OAuth);
                                    view.configure_github_token(token)?;
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
                        Ok(None) => {
                            view.auth_status = GitHubAuthStatus::SignedOut;
                            if !view.github_api.has_auth() {
                                view.show_github_sign_in_required();
                            }
                            view.probe_github_cli_availability(cx);
                        }
                        Err(error) => {
                            view.auth_status = GitHubAuthStatus::Failed(error);
                            if !view.github_api.has_auth() {
                                view.show_github_sign_in_required();
                            }
                            view.status = "Failed to load GitHub credentials".to_string();
                        }
                    }

                    cx.notify();
                },
            );
        }));
    }

    pub(super) fn sign_out_of_github(
        &mut self,
        _: &SignOutOfGitHub,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let delete_token_task = cx.delete_credentials(GITHUB_CREDENTIAL_URL);
        let store = self.repository_state.store();
        let delete_source_task = cx.background_spawn(async move {
            delete_saved_github_auth_source(store)
                .await
                .map_err(|error| error.to_string())
        });
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
                    if let Err(error) = delete_token_result {
                        let message = error.to_string();
                        if !is_missing_credential_error(&message) {
                            view.auth_status = GitHubAuthStatus::Failed(message.clone());
                            view.status = format!("Failed to remove GitHub credentials: {message}");
                        }
                    }
                    if let Err(error) = delete_source_result {
                        view.auth_status = GitHubAuthStatus::Failed(error.clone());
                        view.status = format!("Failed to remove GitHub auth preference: {error}");
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
        self.load_repository_preferences(cx);
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

    pub(super) fn configure_github_token(&self, token: String) -> std::result::Result<(), String> {
        self.github_api
            .configure_token(token)
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
                            view.review_state.set_current_user_login(Some(login));
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

pub(super) async fn load_saved_github_auth_source(
    store: Option<SqliteStore>,
) -> std::result::Result<Option<GitHubAuthSource>, String> {
    let store = store.ok_or_else(|| "storage was not initialized".to_string())?;
    let value = store
        .load_app_setting(GITHUB_AUTH_SOURCE_SETTING_KEY)
        .await
        .map_err(|error| error.to_string())?;

    Ok(value.and_then(|value| GitHubAuthSource::from_storage_value(&value)))
}

pub(super) async fn save_github_auth_source(
    store: Option<SqliteStore>,
    source: GitHubAuthSource,
) -> std::result::Result<(), String> {
    let store = store.ok_or_else(|| "storage was not initialized".to_string())?;
    store
        .save_app_setting(GITHUB_AUTH_SOURCE_SETTING_KEY, source.storage_value())
        .await
        .map_err(|error| error.to_string())
}

async fn delete_saved_github_auth_source(
    store: Option<SqliteStore>,
) -> std::result::Result<(), String> {
    let store = store.ok_or_else(|| "storage was not initialized".to_string())?;
    store
        .delete_app_setting(GITHUB_AUTH_SOURCE_SETTING_KEY)
        .await
        .map_err(|error| error.to_string())
}

pub(super) fn github_oauth_client_id() -> Option<String> {
    std::env::var(GITHUB_OAUTH_CLIENT_ID_ENV)
        .ok()
        .and_then(normalize_github_oauth_client_id)
        .or_else(|| normalize_github_oauth_client_id(DEFAULT_GITHUB_OAUTH_CLIENT_ID.to_string()))
}

fn normalize_github_oauth_client_id(client_id: String) -> Option<String> {
    let client_id = client_id.trim();
    (!client_id.is_empty()).then(|| client_id.to_string())
}

pub(super) fn is_missing_credential_error(message: &str) -> bool {
    message.contains("-25300") || message.to_ascii_lowercase().contains("not found")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_auth_source_parses_settings() {
        assert_eq!(
            GitHubAuthSource::from_storage_value("gh_cli"),
            Some(GitHubAuthSource::GhCli)
        );
        assert_eq!(
            GitHubAuthSource::from_storage_value("token"),
            Some(GitHubAuthSource::OAuth)
        );
        assert_eq!(GitHubAuthSource::from_storage_value("unknown"), None);
    }

    #[test]
    fn normalizes_github_oauth_client_ids() {
        assert_eq!(
            normalize_github_oauth_client_id("  client-id  ".to_string()),
            Some("client-id".to_string())
        );
        assert_eq!(normalize_github_oauth_client_id("   ".to_string()), None);
    }
}
