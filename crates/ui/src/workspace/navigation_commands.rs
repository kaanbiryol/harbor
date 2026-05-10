use gpui::{App, ClipboardItem, Context, ScrollStrategy, Window};
use harbor_domain::{DiffFile, FileStatus, PullRequest};

use crate::actions::*;
use crate::panels::{ContinuousDiffLayoutInput, continuous_diff_hunk_item_index};
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
            .position(|file_index| *file_index == self.diff_selection.file_index)
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
            .position(|file_index| *file_index == self.diff_selection.file_index)
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
            *file_index == self.diff_selection.file_index
                && *hunk_index == self.diff_selection.hunk_index
        }) {
            let next_position = (current_position + 1) % targets.len();
            let (_, file_index, hunk_index) = targets[next_position];
            return Some((file_index, hunk_index));
        }

        let visible_file_indices = self.visible_file_indices(cx);
        let active_file_position = visible_file_indices
            .iter()
            .position(|file_index| *file_index == self.diff_selection.file_index);

        active_file_position
            .and_then(|active_file_position| {
                targets
                    .iter()
                    .find(|(file_position, _, hunk_index)| {
                        *file_position > active_file_position
                            || (*file_position == active_file_position
                                && *hunk_index > self.diff_selection.hunk_index)
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
            *file_index == self.diff_selection.file_index
                && *hunk_index == self.diff_selection.hunk_index
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
            .position(|file_index| *file_index == self.diff_selection.file_index);

        active_file_position
            .and_then(|active_file_position| {
                targets
                    .iter()
                    .rev()
                    .find(|(file_position, _, hunk_index)| {
                        *file_position < active_file_position
                            || (*file_position == active_file_position
                                && *hunk_index < self.diff_selection.hunk_index)
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
        self.diff_selection.file_index = file_index;
        self.diff_selection.hunk_index = hunk_index;
        self.active_tab = PanelTab::Diff;
        self.sync_diff_list_items(cx);

        if let Some(row_index) = self.file_tree_row_index_for_file(file_index, cx) {
            self.file_list_scroll
                .scroll_to_item(row_index, ScrollStrategy::Center);
        }

        let visible_file_indices = self.visible_file_indices(cx);
        if let Some(item_index) = continuous_diff_hunk_item_index(
            ContinuousDiffLayoutInput {
                files: &self.files,
                diffs: &self.diffs,
                visible_file_indices: &visible_file_indices,
                reviewed_file_paths: &self.reviewed_file_paths,
                review_threads: &self.review_threads,
                review_composer: self.review_composer_state.composer.as_ref(),
            },
            file_index,
            hunk_index,
        ) {
            self.scroll_diff_list_to_item(item_index);
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

#[cfg(test)]
mod tests {
    use harbor_domain::FileStatus;

    use super::*;
    use crate::test_fixtures::{diff_file, pull_request};

    #[test]
    fn builds_active_file_github_url() {
        let file = diff_file("src/ui/app view.rs", FileStatus::Modified);

        assert_eq!(
            github_file_url(&pull_request(), &file).as_deref(),
            Some("https://github.com/acme/app/blob/abc123/src/ui/app%20view.rs")
        );
    }

    #[test]
    fn falls_back_for_removed_github_files() {
        let file = diff_file("src/deleted.rs", FileStatus::Removed);

        assert_eq!(github_file_url(&pull_request(), &file), None);
    }
}
