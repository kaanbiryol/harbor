use std::{cell::RefCell, sync::Arc};

use harbor_domain::{
    DiffFile,
    changed_files::{ChangedFileTreeRow, changed_file_tree_rows},
};

use super::{ChangedFileFilters, state::ChangedFilesUiState};

#[derive(Default)]
pub(super) struct ChangedFileTreeCache(RefCell<Option<CachedTree>>);

struct CachedTree {
    files: Arc<Vec<DiffFile>>,
    state: ChangedFilesUiState,
    rows: Arc<[ChangedFileTreeRow]>,
    visible_indices: Arc<[usize]>,
}

impl ChangedFileTreeCache {
    pub(super) fn rows(
        &self,
        files: Arc<Vec<DiffFile>>,
        state: &ChangedFilesUiState,
    ) -> Arc<[ChangedFileTreeRow]> {
        self.with_tree(files, state, |tree| tree.rows.clone())
    }

    pub(super) fn visible_indices(
        &self,
        files: Arc<Vec<DiffFile>>,
        state: &ChangedFilesUiState,
    ) -> Arc<[usize]> {
        self.with_tree(files, state, |tree| tree.visible_indices.clone())
    }

    fn with_tree<R>(
        &self,
        files: Arc<Vec<DiffFile>>,
        state: &ChangedFilesUiState,
        read: impl FnOnce(&CachedTree) -> R,
    ) -> R {
        let mut cache = self.0.borrow_mut();
        if let Some(tree) = cache.as_ref()
            && Arc::ptr_eq(&tree.files, &files)
            && tree.state.collapsed_file_tree_folders == state.collapsed_file_tree_folders
            && tree.state.reviewed_file_paths == state.reviewed_file_paths
            && tree.state.excluded_file_type_filters == state.excluded_file_type_filters
            && tree.state.show_files_owned_by_current_user == state.show_files_owned_by_current_user
            && tree.state.owned_file_paths == state.owned_file_paths
        {
            return read(tree);
        }
        let rows: Arc<[ChangedFileTreeRow]> = changed_file_tree_rows(
            &files,
            &state.collapsed_file_tree_folders,
            &state.reviewed_file_paths,
            &ChangedFileFilters {
                excluded_file_types: state.excluded_file_type_filters.clone(),
                owned_by_current_user_only: state.show_files_owned_by_current_user,
                owned_file_paths: state.owned_file_paths.clone(),
            },
        )
        .into();
        let visible_indices = rows
            .iter()
            .filter_map(|row| match row {
                ChangedFileTreeRow::File(file) => Some(file.file_index),
                ChangedFileTreeRow::Folder(_) => None,
            })
            .collect();
        let tree = CachedTree {
            files,
            state: state.clone(),
            rows,
            visible_indices,
        };
        let result = read(&tree);
        *cache = Some(tree);
        result
    }

    pub(super) fn clear(&self) {
        self.0.borrow_mut().take();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::patched_diff_file;

    #[test]
    fn reuses_rows_until_files_or_filters_change() {
        let cache = ChangedFileTreeCache::default();
        let files = Arc::new(vec![patched_diff_file()]);
        let mut state = ChangedFilesUiState::default();
        let first = cache.rows(files.clone(), &state);
        assert!(Arc::ptr_eq(&first, &cache.rows(files.clone(), &state)));
        state.reviewed_file_paths.insert(files[0].path.clone());
        let reviewed = cache.rows(files.clone(), &state);
        assert!(!Arc::ptr_eq(&first, &reviewed));
        state.show_files_owned_by_current_user = true;
        assert!(cache.visible_indices(files.clone(), &state).is_empty());
        state.owned_file_paths.insert(files[0].path.clone());
        assert_eq!(&*cache.visible_indices(files.clone(), &state), &[0]);
        let replaced = Arc::new(Vec::new());
        assert!(cache.rows(replaced, &state).is_empty());
    }

    #[test]
    #[ignore = "manual changed-file tree benchmark"]
    fn benchmark_repeated_tree_reads() {
        let files = Arc::new(
            (0..10_000)
                .map(|index| {
                    let mut file = patched_diff_file();
                    file.path = format!("src/group{}/file{index}.rs", index / 100);
                    file
                })
                .collect::<Vec<_>>(),
        );
        let state = ChangedFilesUiState::default();
        let filters = ChangedFileFilters::default();
        let started = std::time::Instant::now();
        for _ in 0..100 {
            std::hint::black_box(changed_file_tree_rows(
                &files,
                &state.collapsed_file_tree_folders,
                &state.reviewed_file_paths,
                &filters,
            ));
        }
        let uncached = started.elapsed();
        let cache = ChangedFileTreeCache::default();
        cache.rows(files.clone(), &state);
        let started = std::time::Instant::now();
        for _ in 0..100 {
            std::hint::black_box(cache.rows(files.clone(), &state));
        }
        eprintln!(
            "10,000 files, 100 reads: uncached={uncached:?}, cached={:?}",
            started.elapsed()
        );
    }
}
