use std::collections::HashSet;

use crate::panels::CheckRunFilter;

pub(crate) struct ChecksUiState {
    pub(crate) expanded_groups: HashSet<String>,
    pub(crate) filter: CheckRunFilter,
}

impl Default for ChecksUiState {
    fn default() -> Self {
        Self {
            expanded_groups: HashSet::new(),
            filter: CheckRunFilter::All,
        }
    }
}

impl ChecksUiState {
    pub(crate) fn reset(&mut self) {
        self.expanded_groups.clear();
        self.filter = CheckRunFilter::All;
    }
}
