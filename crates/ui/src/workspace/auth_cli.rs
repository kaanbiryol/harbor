use gpui::{AppContext, Context, Window};
use harbor_github::GhCliTransport;

use crate::{
    actions::{SwitchGitHubAuthToGhCli, UseGitHubCli},
    workspace::{
        AppView, AuthSwitchStatus,
        async_updates::AppViewAsyncUpdateExt,
        auth::{GITHUB_CREDENTIAL_URL, is_missing_credential_error, save_github_auth_source},
    },
};

use super::{GitHubAuthSource, GitHubAuthStatus, GitHubCliAvailability};

impl AppView {
    pub(super) fn use_github_cli(
        &mut self,
        _: &UseGitHubCli,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.sign_in_with_github_cli(true, cx);
    }

    pub(super) fn switch_github_auth_to_gh_cli(
        &mut self,
        _: &SwitchGitHubAuthToGhCli,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.current_github_auth_source() == Some(GitHubAuthSource::GhCli) {
            self.status = "GitHub CLI is already selected".to_string();
            cx.notify();
            return;
        }

        self.auth_switch_status = Some(AuthSwitchStatus::CheckingGitHubCli);
        self.github_cli_availability = GitHubCliAvailability::Checking;
        self.status = "Checking GitHub CLI authentication".to_string();
        let task = cx.background_spawn(async move { preflight_github_cli().await });

        self.tasks.set_auth_task(cx.spawn(async move |this, cx| {
            let result = task.await;

            this.update_or_log(
                cx,
                "failed to update github cli switch state",
                move |view, cx| {
                    match result {
                        Ok(()) => {
                            view.github_cli_availability = GitHubCliAvailability::Available;
                            match view.configure_github_cli_auth() {
                                Ok(()) => {
                                    view.finish_authenticated_github_sign_in(
                                        GitHubAuthSource::GhCli,
                                        cx,
                                    );
                                    view.persist_github_cli_auth_source(cx);
                                }
                                Err(error) => {
                                    let message =
                                        format!("Failed to configure GitHub CLI auth: {error}");
                                    view.auth_switch_status =
                                        Some(AuthSwitchStatus::Failed(message.clone()));
                                    view.status = message;
                                }
                            }
                        }
                        Err(error) => {
                            let reason = github_cli_unavailable_reason(error);
                            view.github_cli_availability =
                                GitHubCliAvailability::Unavailable(reason.clone());
                            view.auth_switch_status =
                                Some(AuthSwitchStatus::Failed(reason.clone()));
                            view.status = format!("GitHub CLI is unavailable: {reason}");
                        }
                    }

                    cx.notify();
                },
            );
        }));
    }

    pub(super) fn sign_in_with_github_cli(&mut self, persist_source: bool, cx: &mut Context<Self>) {
        self.auth_status = GitHubAuthStatus::Loading;
        self.github_auth_popover_open = false;
        self.github_cli_availability = GitHubCliAvailability::Checking;
        self.status = "Checking GitHub CLI authentication".to_string();
        let task = cx.background_spawn(async move { preflight_github_cli().await });

        self.tasks.set_auth_task(cx.spawn(async move |this, cx| {
            let result = task.await;

            this.update_or_log(
                cx,
                "failed to update github cli auth state",
                move |view, cx| {
                    match result {
                        Ok(()) => {
                            view.github_cli_availability = GitHubCliAvailability::Available;
                            match view.configure_github_cli_auth() {
                                Ok(()) => {
                                    view.finish_authenticated_github_sign_in(
                                        GitHubAuthSource::GhCli,
                                        cx,
                                    );
                                    if persist_source {
                                        view.persist_github_cli_auth_source(cx);
                                    }
                                }
                                Err(error) => {
                                    view.auth_status = GitHubAuthStatus::Failed(error.clone());
                                    view.github_auth_popover_open = true;
                                    view.status =
                                        format!("Failed to configure GitHub CLI auth: {error}");
                                }
                            }
                        }
                        Err(error) => {
                            let reason = github_cli_unavailable_reason(error);
                            view.github_cli_availability =
                                GitHubCliAvailability::Unavailable(reason.clone());
                            view.auth_status = GitHubAuthStatus::SignedOut;
                            view.show_github_sign_in_required();
                            view.status = format!("GitHub CLI is unavailable: {reason}");
                        }
                    }

                    cx.notify();
                },
            );
        }));
    }

    fn persist_github_cli_auth_source(&mut self, cx: &mut Context<Self>) {
        let delete_token_task = cx.delete_credentials(GITHUB_CREDENTIAL_URL);
        let write_source_task =
            cx.background_spawn(
                async move { save_github_auth_source(GitHubAuthSource::GhCli).await },
            );

        self.tasks.set_auth_task(cx.spawn(async move |this, cx| {
            let delete_token_result = delete_token_task.await;
            let write_source_result = write_source_task.await;

            this.update_or_log(
                cx,
                "failed to update github cli credential state",
                move |view, cx| {
                    if let Err(error) = delete_token_result {
                        let message = error.to_string();
                        if !is_missing_credential_error(&message) {
                            view.auth_status = GitHubAuthStatus::Failed(message.clone());
                            view.status = format!(
                                "Signed in with GitHub CLI, but failed to remove saved token: {message}"
                            );
                            cx.notify();
                            return;
                        }
                    }

                    if let Err(error) = write_source_result {
                        view.auth_status = GitHubAuthStatus::Failed(error.clone());
                        view.status =
                            format!("Signed in with GitHub CLI, but failed to save auth source: {error}");
                    }

                    cx.notify();
                },
            );
        }));
    }

    fn configure_github_cli_auth(&self) -> std::result::Result<(), String> {
        self.github_api
            .configure_gh_cli()
            .map_err(|error| error.to_string())
    }

    pub(crate) fn probe_github_cli_availability(&mut self, cx: &mut Context<Self>) {
        self.github_cli_availability = GitHubCliAvailability::Checking;
        let task = cx.background_spawn(async move { preflight_github_cli().await });

        cx.spawn(async move |this, cx| {
            let result = task.await;

            this.update_or_log(
                cx,
                "failed to update github cli availability",
                move |view, cx| {
                    view.github_cli_availability = match result {
                        Ok(()) => GitHubCliAvailability::Available,
                        Err(error) => {
                            GitHubCliAvailability::Unavailable(github_cli_unavailable_reason(error))
                        }
                    };
                    cx.notify();
                },
            );
        })
        .detach();
    }
}

async fn preflight_github_cli() -> harbor_github::Result<()> {
    GhCliTransport::default().preflight().await
}

fn github_cli_unavailable_reason(error: harbor_github::GitHubError) -> String {
    match error {
        harbor_github::GitHubError::MissingCli => {
            "GitHub CLI is not installed or not available on PATH.".to_string()
        }
        harbor_github::GitHubError::Unauthenticated
        | harbor_github::GitHubError::UnauthenticatedCli => {
            "Run `gh auth login` to authenticate GitHub CLI.".to_string()
        }
        error => error.to_string(),
    }
}
