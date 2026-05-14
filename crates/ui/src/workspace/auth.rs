use gpui::{AppContext, ClipboardItem, Context, Window};
use harbor_github::start_oauth_device_flow;

use crate::{
    actions::{SignInToGitHub, SignOutOfGitHub, UseGitHubToken},
    workspace::{AppView, async_updates::AppViewAsyncUpdateExt},
};

const GITHUB_CREDENTIAL_URL: &str = "harbor://github/oauth";
const GITHUB_CREDENTIAL_USERNAME: &str = "github";
const GITHUB_OAUTH_CLIENT_ID_ENV: &str = "HARBOR_GITHUB_OAUTH_CLIENT_ID";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum GitHubAuthStatus {
    Loading,
    SignedOut,
    MissingClientId,
    SigningIn {
        user_code: String,
        verification_uri: String,
    },
    SignedIn {
        login: Option<String>,
    },
    Failed(String),
}

impl GitHubAuthStatus {
    pub(crate) fn label(&self) -> &'static str {
        match self {
            Self::Loading => "GitHub",
            Self::SignedOut | Self::MissingClientId | Self::Failed(_) => "Sign in",
            Self::SigningIn { .. } => "Waiting",
            Self::SignedIn { .. } => "Sign out",
        }
    }
}

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
        ) || (!self.github_api.has_token()
            && !matches!(self.auth_status(), GitHubAuthStatus::SignedIn { .. }))
    }

    pub(crate) fn github_auth_popover_open(&self) -> bool {
        self.github_auth_popover_open
            && matches!(
                self.auth_status,
                GitHubAuthStatus::SigningIn { .. }
                    | GitHubAuthStatus::MissingClientId
                    | GitHubAuthStatus::Failed(_)
            )
    }

    pub(crate) fn open_github_auth_popover(&mut self, cx: &mut Context<Self>) {
        self.github_auth_popover_open = true;
        cx.notify();
    }

    pub(crate) fn dismiss_github_auth_popover(&mut self, cx: &mut Context<Self>) {
        self.github_auth_popover_open = false;
        if self.github_api.has_token() {
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
        self.github_auth_popover_open = false;
        let task = cx.read_credentials(GITHUB_CREDENTIAL_URL);

        self.tasks.set_auth_task(cx.spawn(async move |this, cx| {
            let result = task.await;

            this.update_or_log(cx, "failed to update github auth state", move |view, cx| {
                match result {
                    Ok(Some((_username, password))) => {
                        let token = String::from_utf8(password)
                            .map_err(|error| error.to_string())
                            .and_then(|token| view.configure_github_token(token));
                        match token {
                            Ok(()) => {
                                view.auth_status = GitHubAuthStatus::SignedIn { login: None };
                                view.github_auth_popover_open = false;
                                view.status = "Signed in to GitHub".to_string();
                                view.load_recent_repositories(cx);
                            }
                            Err(error) => {
                                view.auth_status = GitHubAuthStatus::Failed(error);
                                view.github_auth_popover_open = true;
                                if !view.github_api.has_token() {
                                    view.show_github_sign_in_required();
                                }
                                view.status = "Failed to load GitHub credentials".to_string();
                            }
                        }
                    }
                    Ok(None) => {
                        view.auth_status = GitHubAuthStatus::SignedOut;
                        if !view.github_api.has_token() {
                            view.show_github_sign_in_required();
                        }
                    }
                    Err(error) => {
                        view.auth_status = GitHubAuthStatus::Failed(error.to_string());
                        if !view.github_api.has_token() {
                            view.show_github_sign_in_required();
                        }
                        view.status = "Failed to load GitHub credentials".to_string();
                    }
                }

                cx.notify();
            });
        }));
    }

    pub(super) fn sign_in_to_github(
        &mut self,
        _: &SignInToGitHub,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(client_id) = std::env::var(GITHUB_OAUTH_CLIENT_ID_ENV)
            .ok()
            .filter(|client_id| !client_id.trim().is_empty())
        else {
            self.auth_status = GitHubAuthStatus::MissingClientId;
            self.github_auth_popover_open = true;
            if !self.github_api.has_token() {
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

    pub(super) fn sign_out_of_github(
        &mut self,
        _: &SignOutOfGitHub,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let delete_task = cx.delete_credentials(GITHUB_CREDENTIAL_URL);
        let clear_result = self.github_api.clear_token();
        self.auth_status = GitHubAuthStatus::SignedOut;
        self.github_auth_popover_open = false;
        self.show_github_sign_in_required();
        self.status = match clear_result {
            Ok(()) => "Signed out of GitHub".to_string(),
            Err(error) => format!("Signed out locally, but failed to clear client: {error}"),
        };

        self.tasks.set_auth_task(cx.spawn(async move |this, cx| {
            let result = delete_task.await;
            this.update_or_log(
                cx,
                "failed to update github sign out state",
                move |view, cx| {
                    if let Err(error) = result {
                        let message = error.to_string();
                        if !message.contains("-25300")
                            && !message.to_ascii_lowercase().contains("not found")
                        {
                            view.auth_status = GitHubAuthStatus::Failed(message.clone());
                            view.status = format!("Failed to remove GitHub credentials: {message}");
                        }
                    }
                    cx.notify();
                },
            );
        }));

        cx.notify();
    }

    pub(super) fn use_github_token(
        &mut self,
        _: &UseGitHubToken,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(item) = cx.read_from_clipboard() else {
            self.status = "Copy a GitHub token before importing it".to_string();
            cx.notify();
            return;
        };
        let Some(token) = item.text().filter(|token| !token.trim().is_empty()) else {
            self.status = "Copy a GitHub token before importing it".to_string();
            cx.notify();
            return;
        };

        self.persist_github_token(token.trim().to_string(), cx);
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
                        Ok(token) => view.persist_github_token(token, cx),
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
                            if !view.github_api.has_token() {
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

    fn persist_github_token(&mut self, token: String, cx: &mut Context<Self>) {
        match self.configure_github_token(token.clone()) {
            Ok(()) => {
                let write_task = cx.write_credentials(
                    GITHUB_CREDENTIAL_URL,
                    GITHUB_CREDENTIAL_USERNAME,
                    token.as_bytes(),
                );
                self.auth_status = GitHubAuthStatus::SignedIn { login: None };
                self.github_auth_popover_open = false;
                self.status = "Signed in to GitHub".to_string();
                self.load_recent_repositories(cx);

                self.tasks.set_auth_task(cx.spawn(async move |this, cx| {
                    let result = write_task.await;
                    this.update_or_log(
                        cx,
                        "failed to update github credential save state",
                        move |view, cx| {
                            if let Err(error) = result {
                                view.auth_status = GitHubAuthStatus::Failed(error.to_string());
                                view.status =
                                    format!("Signed in, but failed to save GitHub token: {error}");
                            }
                            cx.notify();
                        },
                    );
                }));
            }
            Err(error) => {
                self.auth_status = GitHubAuthStatus::Failed(error.to_string());
                self.github_auth_popover_open = true;
                if !self.github_api.has_token() {
                    self.show_github_sign_in_required();
                }
                self.status = format!("Failed to configure GitHub token: {error}");
            }
        }
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
            GitHubAuthStatus::SignedOut => "Sign in to GitHub to load repositories".to_string(),
            GitHubAuthStatus::SignedIn { .. } => "Signed in to GitHub".to_string(),
        };
    }

    fn configure_github_token(&self, token: String) -> std::result::Result<(), String> {
        self.github_api
            .configure_token(token)
            .map_err(|error| error.to_string())
    }
}
