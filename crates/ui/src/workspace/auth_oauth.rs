use gpui::{AppContext, ClipboardItem, Context, Window};
use harbor_github::start_oauth_device_flow;

use crate::{
    actions::{SignInToGitHub, SwitchGitHubAuthToOAuth},
    workspace::{
        AppView, AuthSwitchStatus,
        async_updates::AppViewAsyncUpdateExt,
        auth::{
            GITHUB_CREDENTIAL_URL, GITHUB_OAUTH_CLIENT_ID_ENV, github_oauth_client_id,
            save_github_auth_source,
        },
    },
};

use super::{GitHubAuthSource, GitHubAuthStatus};

const GITHUB_CREDENTIAL_USERNAME: &str = "github";

impl AppView {
    pub(super) fn sign_in_to_github(
        &mut self,
        _: &SignInToGitHub,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(client_id) = github_oauth_client_id() else {
            self.auth_status = GitHubAuthStatus::MissingClientId;
            self.github_auth_popover_open = true;
            if !self.github_api.has_auth() {
                self.show_github_sign_in_required();
            }
            self.status = format!("Set {GITHUB_OAUTH_CLIENT_ID_ENV} to sign in with GitHub");
            cx.notify();
            return;
        };

        self.auth_status = GitHubAuthStatus::Loading;
        self.github_auth_popover_open = false;
        self.status = "Starting GitHub sign in".to_string();
        let task = cx.background_spawn(async move { start_oauth_device_flow(client_id).await });

        self.tasks.set_auth_task(cx.spawn(async move |this, cx| {
            let result = task.await;

            this.update_or_log(
                cx,
                "failed to update github device flow state",
                move |view, cx| {
                    match result {
                        Ok(flow) => {
                            let user_code = flow.user_code().to_string();
                            let verification_uri = flow.verification_uri().to_string();
                            view.auth_status = GitHubAuthStatus::SigningIn {
                                user_code: user_code.clone(),
                                verification_uri: verification_uri.clone(),
                            };
                            view.github_auth_popover_open = true;
                            view.status =
                                "Enter the GitHub device code in your browser".to_string();
                            cx.write_to_clipboard(ClipboardItem::new_string(user_code));
                            cx.open_url(&verification_uri);
                            view.poll_github_device_flow(flow, cx);
                        }
                        Err(error) => {
                            view.auth_status = GitHubAuthStatus::Failed(error.to_string());
                            view.github_auth_popover_open = true;
                            view.status = format!("Failed to start GitHub sign in: {error}");
                        }
                    }

                    cx.notify();
                },
            );
        }));
    }

    pub(super) fn switch_github_auth_to_oauth(
        &mut self,
        _: &SwitchGitHubAuthToOAuth,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.current_github_auth_source() == Some(GitHubAuthSource::OAuth) {
            self.status = "GitHub OAuth is already selected".to_string();
            cx.notify();
            return;
        }

        let Some(client_id) = github_oauth_client_id() else {
            let message = format!("Set {GITHUB_OAUTH_CLIENT_ID_ENV} to sign in with GitHub");
            self.auth_switch_status = Some(AuthSwitchStatus::Failed(message.clone()));
            self.status = message;
            cx.notify();
            return;
        };

        self.auth_switch_status = Some(AuthSwitchStatus::StartingOAuth);
        self.status = "Starting GitHub OAuth switch".to_string();
        let task = cx.background_spawn(async move { start_oauth_device_flow(client_id).await });

        self.tasks.set_auth_task(cx.spawn(async move |this, cx| {
            let result = task.await;

            this.update_or_log(
                cx,
                "failed to update github auth switch state",
                move |view, cx| {
                    match result {
                        Ok(flow) => {
                            let user_code = flow.user_code().to_string();
                            let verification_uri = flow.verification_uri().to_string();
                            view.auth_switch_status = Some(AuthSwitchStatus::WaitingOAuth {
                                user_code: user_code.clone(),
                                verification_uri: verification_uri.clone(),
                            });
                            view.status =
                                "Enter the GitHub device code in your browser".to_string();
                            cx.write_to_clipboard(ClipboardItem::new_string(user_code));
                            cx.open_url(&verification_uri);
                            view.poll_github_auth_switch_device_flow(flow, cx);
                        }
                        Err(error) => {
                            let message = format!("Failed to start GitHub OAuth: {error}");
                            view.auth_switch_status =
                                Some(AuthSwitchStatus::Failed(message.clone()));
                            view.status = message;
                        }
                    }

                    cx.notify();
                },
            );
        }));
    }

    pub(crate) fn open_github_device_verification(&mut self, cx: &mut Context<Self>) {
        if let GitHubAuthStatus::SigningIn {
            verification_uri, ..
        } = &self.auth_status
        {
            cx.open_url(verification_uri);
            self.status = "Opened GitHub device verification".to_string();
            cx.notify();
        }
    }

    pub(crate) fn copy_github_device_code(&mut self, cx: &mut Context<Self>) {
        if let GitHubAuthStatus::SigningIn { user_code, .. } = &self.auth_status {
            cx.write_to_clipboard(ClipboardItem::new_string(user_code.clone()));
            self.status = "Copied GitHub device code".to_string();
            cx.notify();
        }
    }

    pub(crate) fn open_github_auth_switch_verification(&mut self, cx: &mut Context<Self>) {
        if let Some(AuthSwitchStatus::WaitingOAuth {
            verification_uri, ..
        }) = &self.auth_switch_status
        {
            cx.open_url(verification_uri);
            self.status = "Opened GitHub device verification".to_string();
            cx.notify();
        }
    }

    pub(crate) fn copy_github_auth_switch_device_code(&mut self, cx: &mut Context<Self>) {
        if let Some(AuthSwitchStatus::WaitingOAuth { user_code, .. }) = &self.auth_switch_status {
            cx.write_to_clipboard(ClipboardItem::new_string(user_code.clone()));
            self.status = "Copied GitHub device code".to_string();
            cx.notify();
        }
    }

    fn poll_github_device_flow(
        &mut self,
        flow: harbor_github::GitHubDeviceFlow,
        cx: &mut Context<Self>,
    ) {
        let user_code = flow.user_code().to_string();
        let task = cx.background_spawn(async move { flow.poll_for_token().await });

        self.tasks.set_auth_task(cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update_or_log(
                cx,
                "failed to update github token state",
                move |view, cx| {
                    match result {
                        Ok(token) => view.persist_github_token(token, GitHubAuthSource::OAuth, cx),
                        Err(error) => {
                            let still_showing_this_device_code = matches!(
                                &view.auth_status,
                                GitHubAuthStatus::SigningIn {
                                    user_code: active_user_code,
                                    ..
                                } if active_user_code == &user_code
                            );
                            if !still_showing_this_device_code {
                                return;
                            }

                            view.auth_status = GitHubAuthStatus::Failed(error.to_string());
                            view.github_auth_popover_open = true;
                            if !view.github_api.has_auth() {
                                view.show_github_sign_in_required();
                            }
                            view.status = format!("GitHub sign in failed: {error}");
                        }
                    }
                    cx.notify();
                },
            );
        }));
    }

    fn poll_github_auth_switch_device_flow(
        &mut self,
        flow: harbor_github::GitHubDeviceFlow,
        cx: &mut Context<Self>,
    ) {
        let user_code = flow.user_code().to_string();
        let task = cx.background_spawn(async move { flow.poll_for_token().await });

        self.tasks.set_auth_task(cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update_or_log(
                cx,
                "failed to update github auth switch token state",
                move |view, cx| {
                    match result {
                        Ok(token) => {
                            view.persist_github_token(token, GitHubAuthSource::OAuth, cx);
                        }
                        Err(error) => {
                            let still_showing_this_device_code = matches!(
                                &view.auth_switch_status,
                                Some(AuthSwitchStatus::WaitingOAuth {
                                    user_code: active_user_code,
                                    ..
                                }) if active_user_code == &user_code
                            );
                            if !still_showing_this_device_code {
                                return;
                            }

                            let message = format!("GitHub OAuth switch failed: {error}");
                            view.auth_switch_status =
                                Some(AuthSwitchStatus::Failed(message.clone()));
                            view.status = message;
                        }
                    }
                    cx.notify();
                },
            );
        }));
    }

    fn persist_github_token(
        &mut self,
        token: String,
        source: GitHubAuthSource,
        cx: &mut Context<Self>,
    ) {
        match self.configure_github_token(token.clone(), source) {
            Ok(()) => {
                let write_token_task = cx.write_credentials(
                    GITHUB_CREDENTIAL_URL,
                    GITHUB_CREDENTIAL_USERNAME,
                    token.as_bytes(),
                );
                let write_source_task =
                    cx.background_spawn(async move { save_github_auth_source(source).await });
                self.finish_authenticated_github_sign_in(source, cx);

                self.tasks.set_auth_task(cx.spawn(async move |this, cx| {
                    let write_token_result = write_token_task.await;
                    let write_source_result = write_source_task.await;
                    this.update_or_log(
                        cx,
                        "failed to update github credential save state",
                        move |view, cx| {
                            if let Err(error) = write_token_result {
                                view.auth_status = GitHubAuthStatus::Failed(error.to_string());
                                view.status =
                                    format!("Signed in, but failed to save GitHub auth: {error}");
                            } else if let Err(error) = write_source_result {
                                view.auth_status = GitHubAuthStatus::Failed(error.clone());
                                view.status = format!(
                                    "Signed in, but failed to save GitHub auth preference: {error}"
                                );
                            }
                            cx.notify();
                        },
                    );
                }));
            }
            Err(error) => {
                self.auth_status = GitHubAuthStatus::Failed(error.to_string());
                self.github_auth_popover_open = true;
                if !self.github_api.has_auth() {
                    self.show_github_sign_in_required();
                }
                self.status = format!("Failed to configure GitHub OAuth token: {error}");
            }
        }
    }
}
