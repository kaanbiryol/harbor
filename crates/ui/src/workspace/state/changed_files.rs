use std::collections::HashSet;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct ChangedFilesUiState {
    pub(crate) collapsed_file_tree_folders: HashSet<String>,
    pub(crate) expanded_diff_file_paths: HashSet<String>,
    pub(crate) collapsed_diff_file_paths: HashSet<String>,
    pub(crate) reviewed_file_paths: HashSet<String>,
    pub(crate) excluded_file_type_filters: HashSet<String>,
    pub(crate) show_files_owned_by_current_user: bool,
    pub(crate) owned_file_paths: HashSet<String>,
}

impl ChangedFilesUiState {
    pub(crate) fn reset(&mut self) {
        self.collapsed_file_tree_folders.clear();
        self.expanded_diff_file_paths.clear();
        self.collapsed_diff_file_paths.clear();
        self.reviewed_file_paths.clear();
        self.excluded_file_type_filters.clear();
        self.show_files_owned_by_current_user = false;
        self.owned_file_paths.clear();
    }
}
