use gpui::{App, ClipboardItem, Context, ScrollStrategy, Window};
use harbor_domain::{DiffFile, FileStatus, PullRequest, RepoId};

use crate::actions::*;
use crate::panels::continuous_diff_hunk_row_index;
use crate::workspace::AppView;

impl AppView {
    pub(super) fn select_next_file(
        &mut self,
        _: &SelectNextFile,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let visible_files = self.visible_file_indices(cx);
        if visible_files.is_empty() {
            self.status = "No changed files to select".to_string();
            cx.notify();
            return;
        }

        let current_position = visible_files
            .iter()
            .position(|file_index| *file_index == self.active_file)
            .unwrap_or(visible_files.len().saturating_sub(1));
        let next_position = (current_position + 1) % visible_files.len();
        self.select_file(visible_files[next_position], cx);
    }

    pub(super) fn select_previous_file(
        &mut self,
        _: &SelectPreviousFile,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let visible_files = self.visible_file_indices(cx);
        if visible_files.is_empty() {
            self.status = "No changed files to select".to_string();
            cx.notify();
            return;
        }

        let current_position = visible_files
            .iter()
            .position(|file_index| *file_index == self.active_file)
            .unwrap_or(0);
        let previous_position = if current_position == 0 {
            visible_files.len() - 1
        } else {
            current_position - 1
        };
        self.select_file(visible_files[previous_position], cx);
    }

    pub(super) fn select_next_hunk(
        &mut self,
        _: &SelectNextHunk,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((file_index, hunk_index)) = self.next_visible_diff_hunk(cx) else {
            self.status = "No parsed diff hunks for visible files".to_string();
            cx.notify();
            return;
        };

        self.select_visible_diff_hunk(file_index, hunk_index, cx);
        cx.notify();
    }

    pub(super) fn select_previous_hunk(
        &mut self,
        _: &SelectPreviousHunk,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((file_index, hunk_index)) = self.previous_visible_diff_hunk(cx) else {
            self.status = "No parsed diff hunks for visible files".to_string();
            cx.notify();
            return;
        };

        self.select_visible_diff_hunk(file_index, hunk_index, cx);
        cx.notify();
    }

    fn next_visible_diff_hunk(&self, cx: &App) -> Option<(usize, usize)> {
        let targets = self.visible_diff_hunk_targets(cx);
        if targets.is_empty() {
            return None;
        }

        if let Some(current_position) = targets.iter().position(|(_, file_index, hunk_index)| {
            *file_index == self.active_file && *hunk_index == self.active_hunk
        }) {
            let next_position = (current_position + 1) % targets.len();
            let (_, file_index, hunk_index) = targets[next_position];
            return Some((file_index, hunk_index));
        }

        let visible_file_indices = self.visible_file_indices(cx);
        let active_file_position = visible_file_indices
            .iter()
            .position(|file_index| *file_index == self.active_file);

        active_file_position
            .and_then(|active_file_position| {
                targets
                    .iter()
                    .find(|(file_position, _, hunk_index)| {
                        *file_position > active_file_position
                            || (*file_position == active_file_position
                                && *hunk_index > self.active_hunk)
                    })
                    .copied()
            })
            .or_else(|| targets.first().copied())
            .map(|(_, file_index, hunk_index)| (file_index, hunk_index))
    }

    fn previous_visible_diff_hunk(&self, cx: &App) -> Option<(usize, usize)> {
        let targets = self.visible_diff_hunk_targets(cx);
        if targets.is_empty() {
            return None;
        }

        if let Some(current_position) = targets.iter().position(|(_, file_index, hunk_index)| {
            *file_index == self.active_file && *hunk_index == self.active_hunk
        }) {
            let previous_position = if current_position == 0 {
                targets.len() - 1
            } else {
                current_position - 1
            };
            let (_, file_index, hunk_index) = targets[previous_position];
            return Some((file_index, hunk_index));
        }

        let visible_file_indices = self.visible_file_indices(cx);
        let active_file_position = visible_file_indices
            .iter()
            .position(|file_index| *file_index == self.active_file);

        active_file_position
            .and_then(|active_file_position| {
                targets
                    .iter()
                    .rev()
                    .find(|(file_position, _, hunk_index)| {
                        *file_position < active_file_position
                            || (*file_position == active_file_position
                                && *hunk_index < self.active_hunk)
                    })
                    .copied()
            })
            .or_else(|| targets.last().copied())
            .map(|(_, file_index, hunk_index)| (file_index, hunk_index))
    }

    fn visible_diff_hunk_targets(&self, cx: &App) -> Vec<(usize, usize, usize)> {
        let visible_file_indices = self.visible_file_indices(cx);
        let mut targets = Vec::new();

        for (file_position, file_index) in visible_file_indices.into_iter().enumerate() {
            let Some(file) = self.files.get(file_index) else {
                continue;
            };
            if self.reviewed_file_paths.contains(&file.path) {
                continue;
            }

            let Some(diff) = self
                .diffs
                .get(file_index)
                .and_then(Option::as_ref)
                .filter(|diff| !diff.is_empty())
            else {
                continue;
            };

            for hunk_index in 0..diff.hunks.len() {
                targets.push((file_position, file_index, hunk_index));
            }
        }

        targets
    }

    fn select_visible_diff_hunk(
        &mut self,
        file_index: usize,
        hunk_index: usize,
        cx: &mut Context<Self>,
    ) {
        self.active_file = file_index;
        self.active_hunk = hunk_index;
        self.active_tab = PanelTab::Diff;

        if let Some(row_index) = self.file_tree_row_index_for_file(file_index, cx) {
            self.file_list_scroll
                .scroll_to_item(row_index, ScrollStrategy::Center);
        }

        let visible_file_indices = self.visible_file_indices(cx);
        if let Some(row_index) = continuous_diff_hunk_row_index(
            &self.files,
            &self.diffs,
            &visible_file_indices,
            &self.reviewed_file_paths,
            file_index,
            hunk_index,
            &self.review_threads,
            self.review_composer.as_ref(),
            self.review_comment_error.as_deref(),
            self.review_thread_reply_thread_id.as_deref(),
            self.review_comment_edit_comment_id.as_deref(),
        ) {
            self.diff_list_scroll
                .scroll_to_item(row_index, ScrollStrategy::Center);
        }

        let path = self
            .files
            .get(file_index)
            .map(|file| file.path.as_str())
            .unwrap_or("selected file");
        self.status = format!("Selected hunk {} in {path}", hunk_index + 1);
    }

    pub(super) fn copy_active_file_path(
        &mut self,
        _: &CopyActiveFilePath,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(path) = self.active_file().map(|file| file.path.clone()) else {
            self.status = "No active file path to copy".to_string();
            cx.notify();
            return;
        };

        cx.write_to_clipboard(ClipboardItem::new_string(path.clone()));
        self.status = format!("Copied {path}");
        cx.notify();
    }

    pub(super) fn open_active_file_on_github(
        &mut self,
        _: &OpenActiveFileOnGitHub,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(pr) = self.selected_pull_request() else {
            self.status = "No pull request selected".to_string();
            cx.notify();
            return;
        };

        let Some(file) = self.active_file() else {
            cx.open_url(&format!("{}/files", pr.url));
            self.status = format!("Opened GitHub files view for PR #{}", pr.number);
            cx.notify();
            return;
        };

        let url = github_file_url(pr, file).unwrap_or_else(|| format!("{}/files", pr.url));
        let path = file.path.clone();
        cx.open_url(&url);
        self.status = if file.status == FileStatus::Removed {
            format!("Opened GitHub files view because {path} was removed")
        } else {
            format!("Opened {path} on GitHub")
        };
        cx.notify();
    }

    pub(super) fn select_next(
        &mut self,
        _: &SelectNextPullRequest,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.pull_requests.is_empty() {
            let next = (self.selected_pr + 1) % self.pull_requests.len();
            self.select_pull_request(next, cx);
        } else {
            self.status = "No pull requests to select".to_string();
            cx.notify();
        }
    }

    pub(super) fn select_previous(
        &mut self,
        _: &SelectPreviousPullRequest,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.pull_requests.is_empty() {
            let previous = if self.selected_pr == 0 {
                self.pull_requests.len() - 1
            } else {
                self.selected_pr - 1
            };
            self.select_pull_request(previous, cx);
        } else {
            self.status = "No pull requests to select".to_string();
            cx.notify();
        }
    }

    pub(super) fn open_selected(
        &mut self,
        _: &OpenSelectedPullRequest,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(number) = self.selected_pull_request_number() else {
            self.status = "No pull request selected".to_string();
            cx.notify();
            return;
        };

        self.repository_switcher_open = false;
        self.pull_request_switcher_open = false;
        self.file_filter_popover_open = false;
        self.pull_request_inbox_visible = false;
        self.active_tab = PanelTab::Diff;
        self.status = format!("Opened PR #{number} details");

        if self.files.is_empty()
            && !self.is_loading_details
            && !self.is_loading_files
            && !self.is_loading_reviews
        {
            self.refresh_selected_pull_request(cx);
        } else {
            cx.notify();
        }
    }

    pub(super) fn cycle_panel_tab(
        &mut self,
        _: &CyclePanelTab,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_panel_tab(self.active_tab.next(), cx);
    }

    pub(super) fn toggle_pull_request_inbox(
        &mut self,
        _: &TogglePullRequestInbox,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.pull_request_inbox_visible = !self.pull_request_inbox_visible;
        self.repository_switcher_open = false;
        self.pull_request_switcher_open = false;
        self.file_filter_popover_open = false;
        self.status = if self.pull_request_inbox_visible {
            "Pull request inbox shown".to_string()
        } else {
            "Pull request inbox hidden".to_string()
        };
        cx.notify();
    }

    pub(crate) fn select_panel_tab(&mut self, tab: PanelTab, cx: &mut Context<Self>) {
        if self.active_tab == tab {
            return;
        }

        self.active_tab = tab;
        self.status = format!("Switched to {} panel", tab.label());
        cx.notify();
    }

    pub(super) fn toggle_repository_switcher(
        &mut self,
        _: &ToggleRepositorySwitcher,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.repository_switcher_open = !self.repository_switcher_open;
        if self.repository_switcher_open {
            self.pull_request_switcher_open = false;
            self.file_filter_popover_open = false;
            self.repository_search_input.update(cx, |input, cx| {
                input.set_value("", window, cx);
                input.focus(window, cx);
            });
            self.reset_repository_switcher_selection(cx);
        }
        self.status = if self.repository_switcher_open {
            "Repository switcher opened".to_string()
        } else {
            "Repository switcher closed".to_string()
        };
        cx.notify();
    }

    pub(super) fn close_panel(&mut self, _: &ClosePanel, _: &mut Window, cx: &mut Context<Self>) {
        self.repository_switcher_open = false;
        self.pull_request_switcher_open = false;
        self.file_filter_popover_open = false;
        self.status = "Closed transient UI".to_string();
        cx.notify();
    }

    pub(crate) fn select_repository_from_switcher(
        &mut self,
        repository: RepoId,
        cx: &mut Context<Self>,
    ) {
        let selected_repository = repository.full_name();
        if self.configured_repo.as_ref() == Some(&repository) {
            self.status = format!("Selected repository {selected_repository}");
            cx.notify();
            return;
        }

        self.load_pull_requests(repository, cx);
    }

    pub(super) fn set_placeholder_status(&mut self, label: &str, cx: &mut Context<Self>) {
        self.status = format!(
            "{label} is wired as a command placeholder for {}",
            self.selected_pr_label()
        );
        cx.notify();
    }

    pub(super) fn refresh_selected(
        &mut self,
        _: &RefreshSelectedPullRequest,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.selected_pull_request_number().is_some() {
            self.refresh_selected_pull_request(cx);
        } else if let Some(repo) = self.configured_repo.clone() {
            self.refresh_pull_requests(repo, cx);
        } else {
            self.status =
                "Select a repository from the header before refreshing pull requests".to_string();
            cx.notify();
        }
    }

    pub(super) fn open_in_browser(
        &mut self,
        _: &OpenPullRequestInBrowser,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(pr) = self.selected_pull_request() else {
            self.status = "No pull request selected".to_string();
            cx.notify();
            return;
        };

        let url = pr.url.clone();
        let number = pr.number;
        cx.open_url(&url);
        self.status = format!("Opened PR #{number} in browser");
        cx.notify();
    }

    pub(super) fn open_logs(&mut self, _: &OpenLogs, _: &mut Window, cx: &mut Context<Self>) {
        self.active_tab = PanelTab::Logs;
        if self.selected_pull_request().is_some() {
            self.load_selected_workflow_logs(cx);
        } else {
            self.set_placeholder_status("Open logs", cx);
        }
    }

    pub(super) fn filter_current_list(
        &mut self,
        _: &FilterCurrentList,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.file_filter_popover_open = !self.file_filter_popover_open;
        self.repository_switcher_open = false;
        self.pull_request_switcher_open = false;
        self.status = if self.file_filter_popover_open {
            "Opened changed-file filters".to_string()
        } else {
            "Closed changed-file filters".to_string()
        };
        cx.notify();
    }
}

pub(crate) fn github_file_url(pr: &PullRequest, file: &DiffFile) -> Option<String> {
    if file.status == FileStatus::Removed || pr.head_sha.is_empty() || file.path.is_empty() {
        return None;
    }

    Some(format!(
        "https://github.com/{}/{}/blob/{}/{}",
        encode_path_component(&pr.repo.owner),
        encode_path_component(&pr.repo.name),
        encode_path_component(&pr.head_sha),
        encode_github_path(&file.path)
    ))
}

fn encode_github_path(path: &str) -> String {
    path.split('/')
        .map(encode_path_component)
        .collect::<Vec<_>>()
        .join("/")
}

fn encode_path_component(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());

    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }

    encoded
}
