use crate::gpui_component::{Root, WindowExt};
use gpui::{AppContext, Context, PathPromptOptions, Task, Window};
use harbor_domain::preferences::{AppPreferences, PreferredEditor, PreferredTerminal};
use harbor_git::ExternalApp;
use harbor_sync::SyncPolicy;

use crate::{
    actions::{CloseSettings, OpenSettings},
    workspace::{AppView, GitHubAuthSource},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SettingsSection {
    General,
    Notifications,
    LocalDevelopment,
    GitHub,
}

impl SettingsSection {
    pub(crate) const ALL: [Self; 4] = [
        Self::General,
        Self::Notifications,
        Self::LocalDevelopment,
        Self::GitHub,
    ];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Notifications => "Notifications",
            Self::LocalDevelopment => "Local Development",
            Self::GitHub => "GitHub",
        }
    }
}

#[derive(Default)]
pub(crate) struct PreferencesState {
    pub(crate) value: AppPreferences,
    pub(crate) error: Option<String>,
    pub(crate) loading: bool,
    revision: u64,
    save_task: Option<Task<()>>,
}

pub(crate) fn preferred_editor_app(editor: PreferredEditor) -> ExternalApp {
    match editor {
        PreferredEditor::VsCode => ExternalApp::VsCode,
        PreferredEditor::Cursor => ExternalApp::Cursor,
        PreferredEditor::Zed => ExternalApp::Zed,
        PreferredEditor::Xcode => ExternalApp::Xcode,
    }
}

pub(crate) fn preferred_terminal_app(terminal: PreferredTerminal) -> ExternalApp {
    match terminal {
        PreferredTerminal::Terminal => ExternalApp::Terminal,
        PreferredTerminal::Ghostty => ExternalApp::Ghostty,
        PreferredTerminal::Warp => ExternalApp::Warp,
    }
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
        self.repository_state.repository_switcher_open = false;
        self.pull_request_inbox_search_open = false;
        self.pull_request_filter_popover_open = false;
        self.file_filter_popover_open = false;
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
        self.settings_section = SettingsSection::General;
        cx.notify();
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

    pub(crate) fn update_preferences(
        &mut self,
        update: impl FnOnce(&mut AppPreferences),
        cx: &mut Context<Self>,
    ) {
        let previous = self.preferences.value.clone();
        update(&mut self.preferences.value);
        self.preferences.value.normalize();
        if previous == self.preferences.value {
            return;
        }
        self.preferences.revision += 1;
        self.apply_preferences(&previous, cx);
        self.save_preferences(cx);
        cx.notify();
    }

    fn apply_preferences(&mut self, previous: &AppPreferences, cx: &mut Context<Self>) {
        self.prepare_diff_display(cx);
        let preferences = &self.preferences.value;
        if previous.code_font_size != preferences.code_font_size
            || previous.wrap_diff_lines != preferences.wrap_diff_lines
        {
            let scroll = self.diff_list_state.logical_scroll_top();
            self.diff_list_state
                .splice(0..self.diff_list_items.len(), self.diff_list_items.len());
            self.diff_list_state.scroll_to(scroll);
        }
        self.sync_runtime.set_policy(SyncPolicy {
            focused_interval: preferences.focused_refresh.duration().unwrap_or_default(),
            background_interval: preferences
                .background_refresh
                .duration()
                .unwrap_or_default(),
        });
        if previous.focused_refresh != preferences.focused_refresh
            || previous.background_refresh != preferences.background_refresh
        {
            self.tasks.clear_sync_task();
            self.ensure_sync_loop(cx);
        }
    }

    pub(super) fn load_preferences(&mut self, cx: &mut Context<Self>) {
        let Some(store) = self.repository_state.store() else {
            return;
        };
        self.preferences.loading = true;
        let revision = self.preferences.revision;
        let task = cx.background_spawn(async move { harbor_sync::load_preferences(&store).await });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            if let Err(error) = this.update(cx, |view, cx| {
                view.preferences.loading = false;
                match result {
                    Ok(preferences) if revision == 0 && view.preferences.revision == revision => {
                        let previous = std::mem::replace(&mut view.preferences.value, preferences);
                        view.apply_preferences(&previous, cx);
                    }
                    Ok(_) => view.save_preferences(cx),
                    Err(error) => {
                        view.preferences.error = Some(format!("Could not load settings: {error}"))
                    }
                }
                view.load_github_credentials(cx);
                view.load_repository_preferences(cx);
                cx.notify();
            }) {
                tracing::warn!(%error, "failed to load preferences into workspace");
            }
        })
        .detach();
    }

    fn save_preferences(&mut self, cx: &mut Context<Self>) {
        if self.preferences.save_task.is_some() {
            return;
        }
        let Some(store) = self.repository_state.store() else {
            self.preferences.error = Some(
                "Settings apply for this session, but local storage is unavailable.".to_string(),
            );
            return;
        };
        self.preferences.error = None;
        self.preferences.save_task = Some(cx.spawn(async move |this, cx| {
            loop {
                let (preferences, revision) = match this.update(cx, |view, _| {
                    (view.preferences.value.clone(), view.preferences.revision)
                }) {
                    Ok(snapshot) => snapshot,
                    Err(error) => {
                        tracing::warn!(%error, "failed to read preferences for saving");
                        return;
                    }
                };
                let store = store.clone();
                let result = cx
                    .background_executor()
                    .spawn(async move { harbor_sync::save_preferences(&store, &preferences).await })
                    .await;
                if let Err(error) = &result {
                    tracing::warn!(%error, "failed to persist preferences");
                }
                match this.update(cx, |view, cx| {
                    if view.preferences.revision != revision {
                        return true;
                    }
                    view.preferences.error = result
                        .err()
                        .map(|error| format!("Could not save settings: {error}"));
                    view.preferences.save_task = None;
                    cx.notify();
                    false
                }) {
                    Ok(true) => continue,
                    Ok(false) => return,
                    Err(error) => {
                        tracing::warn!(%error, "failed to finish saving preferences");
                        return;
                    }
                }
            }
        }));
    }

    pub(crate) fn choose_worktree_directory(&mut self, cx: &mut Context<Self>) {
        let selected = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Choose a folder for PR worktrees".into()),
        });
        cx.spawn(async move |this, cx| {
            let result = selected.await;
            if let Err(error) = this.update(cx, |view, cx| {
                match result {
                    Ok(Ok(Some(paths))) => {
                        if let Some(path) = paths.into_iter().next() {
                            view.update_preferences(
                                |preferences| preferences.worktree_directory = Some(path),
                                cx,
                            );
                        }
                    }
                    Ok(Ok(None)) => {}
                    Ok(Err(error)) => {
                        view.preferences.error =
                            Some(format!("Could not choose worktree folder: {error}"))
                    }
                    Err(error) => {
                        view.preferences.error =
                            Some(format!("Folder picker was interrupted: {error}"))
                    }
                }
                cx.notify();
            }) {
                tracing::warn!(%error, "failed to update worktree folder preference");
            }
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[gpui::test]
    async fn settings_sections_render_and_github_entry_opens_account_settings(
        cx: &mut gpui::TestAppContext,
    ) {
        use crate::gpui_component::{Root, Theme, ThemeMode};
        cx.update(|cx| {
            crate::gpui_component::init(cx);
            Theme::change(ThemeMode::Dark, None, cx);
        });
        let mut entity = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| AppView::new_without_startup_tasks(window, cx));
            entity = Some(view.clone());
            Root::new(view, window, cx)
        });
        let view = entity.expect("view");
        cx.update(|window, cx| {
            view.update(cx, |view, cx| view.open_settings(&OpenSettings, window, cx))
        });
        for section in SettingsSection::ALL {
            view.update(cx, |view, cx| {
                view.settings_section = section;
                cx.notify();
            });
            cx.refresh().expect("settings section renders");
            let bounds = cx
                .debug_bounds("settings-section-content")
                .expect("settings content");
            assert!(bounds.size.height > gpui::px(0.0) && bounds.size.height <= gpui::px(440.0));
        }
        cx.update(|window, cx| {
            view.update(cx, |view, cx| {
                view.close_settings(&CloseSettings, window, cx);
                view.open_github_settings(window, cx);
                assert_eq!(view.settings_section, SettingsSection::GitHub);
            })
        });
    }

    #[gpui::test]
    async fn rapid_preference_changes_persist_the_latest_value(cx: &mut gpui::TestAppContext) {
        use harbor_storage::{SqliteStore, StorageConfig};
        // SQLite wakes futures from threads outside GPUI's test dispatcher.
        cx.executor().allow_parking();
        let path = std::env::temp_dir().join(format!(
            "harbor-preferences-{}-{}.sqlite3",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("test time")
                .as_nanos()
        ));
        let store = cx
            .background_executor
            .spawn({
                let path = path.clone();
                async move {
                    SqliteStore::connect(StorageConfig {
                        database_path: path,
                    })
                    .await
                    .expect("test store")
                }
            })
            .await;
        cx.update(crate::gpui_component::init);
        let (view, cx) = cx.add_window_view(AppView::new_without_startup_tasks);
        let save = view.update(cx, |view, cx| {
            view.repository_state.set_store(store.clone());
            for font_size in 10..=24 {
                view.update_preferences(
                    |preferences| preferences.code_font_size = font_size as f32,
                    cx,
                );
            }
            view.update_preferences(|preferences| preferences.editor = PreferredEditor::Zed, cx);
            view.preferences.save_task.take().expect("save task")
        });
        save.await;
        let loaded = cx
            .background_executor
            .spawn(async move {
                harbor_sync::load_preferences(&store)
                    .await
                    .expect("saved preferences")
            })
            .await;
        assert_eq!(loaded.code_font_size, 24.0);
        assert_eq!(loaded.editor, PreferredEditor::Zed);
        view.read_with(cx, |view, _| assert!(view.preferences.error.is_none()));
        cx.background_executor
            .spawn(async move {
                std::fs::remove_file(path).expect("remove test database");
            })
            .await;
    }

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
