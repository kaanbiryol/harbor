use std::collections::HashSet;

use crate::panels::CheckRunFilter;

pub(crate) struct ChecksUiState {
    pub(crate) collapsed_groups: HashSet<String>,
    pub(crate) filter: CheckRunFilter,
}

impl Default for ChecksUiState {
    fn default() -> Self {
        Self {
            collapsed_groups: HashSet::new(),
            filter: CheckRunFilter::All,
        }
    }
}

impl ChecksUiState {
    pub(crate) fn reset(&mut self) {
        self.collapsed_groups.clear();
        self.filter = CheckRunFilter::All;
    }
}
