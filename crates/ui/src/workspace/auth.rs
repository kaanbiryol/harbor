use gpui::{AppContext, ClipboardItem, Context, Window};
use harbor_github::{GhCliTransport, start_oauth_device_flow};

use crate::{
    actions::{
        SignInToGitHub, SignOutOfGitHub, SwitchGitHubAuthToGhCli, SwitchGitHubAuthToOAuth,
        UseGitHubCli,
    },
    workspace::{AppView, AuthSwitchStatus, async_updates::AppViewAsyncUpdateExt},
};

use super::{GitHubAuthSource, GitHubAuthStatus, GitHubCliAvailability};

const GITHUB_CREDENTIAL_URL: &str = "harbor://github/oauth";
const GITHUB_CREDENTIAL_USERNAME: &str = "github";
const GITHUB_AUTH_SOURCE_CREDENTIAL_URL: &str = "harbor://github/auth-source";
const GITHUB_AUTH_SOURCE_CREDENTIAL_USERNAME: &str = "github-auth-source";
const GITHUB_OAUTH_CLIENT_ID_ENV: &str = "HARBOR_GITHUB_OAUTH_CLIENT_ID";

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

    pub(super) fn use_github_cli(
        &mut self,
        _: &UseGitHubCli,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.sign_in_with_github_cli(true, cx);
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

    fn sign_in_with_github_cli(&mut self, persist_source: bool, cx: &mut Context<Self>) {
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
                let write_source_task = cx.write_credentials(
                    GITHUB_AUTH_SOURCE_CREDENTIAL_URL,
                    GITHUB_AUTH_SOURCE_CREDENTIAL_USERNAME,
                    source.credential_value().as_bytes(),
                );
                self.finish_authenticated_github_sign_in(source, cx);

                self.tasks.set_auth_task(cx.spawn(async move |this, cx| {
                    let write_token_result = write_token_task.await;
                    let write_source_result = write_source_task.await;
                    this.update_or_log(
                        cx,
                        "failed to update github credential save state",
                        move |view, cx| {
                            for result in [write_token_result, write_source_result] {
                                if let Err(error) = result {
                                    view.auth_status = GitHubAuthStatus::Failed(error.to_string());
                                    view.status = format!(
                                        "Signed in, but failed to save GitHub auth: {error}"
                                    );
                                    break;
                                }
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

    fn persist_github_cli_auth_source(&mut self, cx: &mut Context<Self>) {
        let delete_token_task = cx.delete_credentials(GITHUB_CREDENTIAL_URL);
        let write_source_task = cx.write_credentials(
            GITHUB_AUTH_SOURCE_CREDENTIAL_URL,
            GITHUB_AUTH_SOURCE_CREDENTIAL_USERNAME,
            GitHubAuthSource::GhCli.credential_value().as_bytes(),
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
                        view.auth_status = GitHubAuthStatus::Failed(error.to_string());
                        view.status =
                            format!("Signed in with GitHub CLI, but failed to save auth source: {error}");
                    }

                    cx.notify();
                },
            );
        }));
    }

    fn finish_authenticated_github_sign_in(
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

    fn configure_github_token(
        &self,
        token: String,
        source: GitHubAuthSource,
    ) -> std::result::Result<(), String> {
        self.github_api
            .configure_token(token, source)
            .map_err(|error| error.to_string())
    }

    fn configure_github_cli_auth(&self) -> std::result::Result<(), String> {
        self.github_api
            .configure_gh_cli()
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

fn saved_github_auth_source(
    credential: std::result::Result<Option<Vec<u8>>, String>,
) -> std::result::Result<Option<GitHubAuthSource>, String> {
    credential.and_then(|credential| match credential {
        Some(bytes) => GitHubAuthSource::from_credential_bytes(bytes),
        None => Ok(None),
    })
}

fn github_oauth_client_id() -> Option<String> {
    std::env::var(GITHUB_OAUTH_CLIENT_ID_ENV)
        .ok()
        .filter(|client_id| !client_id.trim().is_empty())
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

fn is_missing_credential_error(message: &str) -> bool {
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
