#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PullRequestSelectionState {
    pull_request_index: usize,
    file_index: usize,
    hunk_index: usize,
}

impl PullRequestSelectionState {
    pub(crate) fn pull_request_index(&self) -> usize {
        self.pull_request_index
    }

    pub(crate) fn file_index(&self) -> usize {
        self.file_index
    }

    pub(crate) fn hunk_index(&self) -> usize {
        self.hunk_index
    }

    pub(crate) fn set_pull_request_index(&mut self, index: usize) {
        self.pull_request_index = index;
    }

    pub(crate) fn restore_pull_request_index(&mut self, index: usize, pull_request_count: usize) {
        self.pull_request_index = index.min(pull_request_count.saturating_sub(1));
    }

    pub(crate) fn reset_pull_request_index(&mut self) {
        self.pull_request_index = 0;
    }

    pub(crate) fn reset_diff_selection(&mut self) {
        self.file_index = 0;
        self.hunk_index = 0;
    }

    pub(crate) fn select_file_index(&mut self, file_index: usize) {
        self.file_index = file_index;
        self.hunk_index = 0;
    }

    pub(crate) fn set_diff_position(&mut self, file_index: usize, hunk_index: usize) {
        self.file_index = file_index;
        self.hunk_index = hunk_index;
    }

    pub(crate) fn restore_diff_position(
        &mut self,
        file_index: usize,
        hunk_index: usize,
        file_count: usize,
    ) {
        self.file_index = file_index.min(file_count.saturating_sub(1));
        self.hunk_index = hunk_index;
    }
}
