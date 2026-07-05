use gpui::{Context, Window};
use gpui_component::{Root, WindowExt};

use crate::{
    actions::{CloseSettings, OpenSettings},
    workspace::{AppView, GitHubAuthSource},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SettingsSection {
    GitHub,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum AuthSwitchStatus {
    CheckingGitHubCli,
    StartingOAuth,
    WaitingOAuth {
        user_code: String,
        verification_uri: String,
    },
    Failed(String),
}

impl AuthSwitchStatus {
    pub(crate) fn label(&self) -> &'static str {
        match self {
            Self::CheckingGitHubCli => "Checking GitHub CLI",
            Self::StartingOAuth => "Starting GitHub OAuth",
            Self::WaitingOAuth { .. } => "Waiting for GitHub OAuth",
            Self::Failed(_) => "Auth switch failed",
        }
    }

    pub(crate) fn message(&self) -> String {
        match self {
            Self::CheckingGitHubCli => "Checking your authenticated gh session.".to_string(),
            Self::StartingOAuth => "Starting GitHub device login.".to_string(),
            Self::WaitingOAuth { .. } => {
                "Enter the device code in your browser. Your current GitHub session remains active until this completes.".to_string()
            }
            Self::Failed(error) => error.clone(),
        }
    }
}

impl AppView {
    #[cfg(test)]
    pub(crate) fn settings_open(&self) -> bool {
        self.settings_open
    }

    #[cfg(test)]
    pub(crate) fn settings_section(&self) -> SettingsSection {
        self.settings_section
    }

    pub(crate) fn auth_switch_status(&self) -> Option<&AuthSwitchStatus> {
        self.auth_switch_status.as_ref()
    }

    pub(crate) fn open_github_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings_open = true;
        self.settings_section = SettingsSection::GitHub;
        self.github_auth_popover_open = false;
        self.review_action_comment_target = None;
        if self.current_github_auth_source() == Some(GitHubAuthSource::OAuth) {
            self.probe_github_cli_availability(cx);
        }
        self.status = "Opened settings".to_string();
        self.open_settings_dialog(window, cx);
        cx.notify();
    }

    pub(super) fn open_settings(
        &mut self,
        _: &OpenSettings,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_github_settings(window, cx);
    }

    pub(super) fn close_settings(
        &mut self,
        _: &CloseSettings,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.settings_open = false;
        if window.root::<Root>().flatten().is_some() {
            window.close_dialog(cx);
        }
        self.status = "Closed settings".to_string();
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_switch_status_messages_describe_pending_work() {
        assert_eq!(
            AuthSwitchStatus::CheckingGitHubCli.label(),
            "Checking GitHub CLI"
        );
        assert_eq!(
            AuthSwitchStatus::StartingOAuth.label(),
            "Starting GitHub OAuth"
        );
        assert!(
            AuthSwitchStatus::WaitingOAuth {
                user_code: "ABCD-1234".to_string(),
                verification_uri: "https://github.com/login/device".to_string(),
            }
            .message()
            .contains("current GitHub session remains active")
        );
    }
}
