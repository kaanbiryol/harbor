use std::collections::{BTreeMap, HashSet};

use harbor_domain::DiffFile;

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ChangedFileTreeRow {
    Folder(ChangedFileFolderRow),
    File(ChangedFileRow),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ChangedFileFolderRow {
    pub(crate) path: String,
    pub(crate) name: String,
    pub(crate) depth: usize,
    pub(crate) file_count: usize,
    pub(crate) reviewed_file_count: usize,
    pub(crate) expanded: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ChangedFileRow {
    pub(crate) file_index: usize,
    pub(crate) name: String,
    pub(crate) depth: usize,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct ChangedFileFilters {
    pub(crate) excluded_file_types: HashSet<String>,
    pub(crate) owned_by_current_user_only: bool,
    pub(crate) owned_file_paths: HashSet<String>,
}

impl ChangedFileFilters {
    fn has_active_filter(&self) -> bool {
        !self.excluded_file_types.is_empty() || self.owned_by_current_user_only
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ChangedFileTypeFilter {
    pub(crate) key: String,
    pub(crate) label: String,
    pub(crate) file_count: usize,
    pub(crate) included: bool,
}

#[derive(Default)]
struct ChangedFileTreeNode {
    folders: BTreeMap<String, ChangedFileTreeNode>,
    files: Vec<usize>,
    file_count: usize,
    reviewed_file_count: usize,
}

impl ChangedFileTreeNode {
    fn add_file(&mut self, file_index: usize, path_segments: &[&str], reviewed: bool) {
        self.file_count += 1;
        if reviewed {
            self.reviewed_file_count += 1;
        }

        let Some((next_segment, remaining_segments)) = path_segments.split_first() else {
            self.files.push(file_index);
            return;
        };

        if remaining_segments.is_empty() {
            self.files.push(file_index);
            return;
        }

        self.folders
            .entry((*next_segment).to_string())
            .or_default()
            .add_file(file_index, remaining_segments, reviewed);
    }
}

pub(crate) fn changed_file_tree_rows(
    files: &[DiffFile],
    collapsed_folders: &HashSet<String>,
    reviewed_file_paths: &HashSet<String>,
    filters: &ChangedFileFilters,
) -> Vec<ChangedFileTreeRow> {
    let mut root = ChangedFileTreeNode::default();

    for (file_index, file) in files.iter().enumerate() {
        if !changed_file_matches_filters(file, filters) {
            continue;
        }

        let path_segments = file
            .path
            .split('/')
            .filter(|segment| !segment.is_empty())
            .collect::<Vec<_>>();
        if path_segments.is_empty() {
            continue;
        }

        root.add_file(
            file_index,
            &path_segments,
            reviewed_file_paths.contains(&file.path),
        );
    }

    let mut rows = Vec::with_capacity(root.file_count + root.folders.len());
    push_changed_file_tree_rows(
        &root,
        "",
        0,
        files,
        collapsed_folders,
        filters.has_active_filter(),
        &mut rows,
    );
    rows
}

pub(crate) fn changed_file_matches_filters(file: &DiffFile, filters: &ChangedFileFilters) -> bool {
    if filters
        .excluded_file_types
        .contains(&changed_file_type_key(file))
    {
        return false;
    }

    if filters.owned_by_current_user_only && !filters.owned_file_paths.contains(&file.path) {
        return false;
    }

    true
}

pub(crate) fn changed_file_type_filters(
    files: &[DiffFile],
    excluded_file_types: &HashSet<String>,
) -> Vec<ChangedFileTypeFilter> {
    let mut file_counts_by_type = BTreeMap::<String, usize>::new();

    for file in files {
        let file_type = changed_file_type_key(file);
        *file_counts_by_type.entry(file_type).or_default() += 1;
    }

    file_counts_by_type
        .into_iter()
        .map(|(key, file_count)| ChangedFileTypeFilter {
            label: key.clone(),
            included: !excluded_file_types.contains(&key),
            key,
            file_count,
        })
        .collect()
}

pub(crate) fn changed_file_type_key(file: &DiffFile) -> String {
    let name = changed_file_name(&file.path);

    if let Some((stem, extension)) = name.rsplit_once('.')
        && !stem.is_empty()
        && !extension.is_empty()
    {
        return extension.to_lowercase();
    }

    "no extension".to_string()
}

fn push_changed_file_tree_rows(
    node: &ChangedFileTreeNode,
    parent_path: &str,
    depth: usize,
    files: &[DiffFile],
    collapsed_folders: &HashSet<String>,
    force_expanded: bool,
    rows: &mut Vec<ChangedFileTreeRow>,
) {
    for (folder_name, child_node) in &node.folders {
        let (folder_path, folder_name, child_node) =
            compact_folder_chain(parent_path, folder_name, child_node);
        let expanded = force_expanded || !collapsed_folders.contains(&folder_path);

        rows.push(ChangedFileTreeRow::Folder(ChangedFileFolderRow {
            path: folder_path.clone(),
            name: folder_name,
            depth,
            file_count: child_node.file_count,
            reviewed_file_count: child_node.reviewed_file_count,
            expanded,
        }));

        if expanded {
            push_changed_file_tree_rows(
                child_node,
                &folder_path,
                depth + 1,
                files,
                collapsed_folders,
                force_expanded,
                rows,
            );
        }
    }

    let mut file_indices = node.files.clone();
    file_indices.sort_by(|left, right| {
        let left_name = files
            .get(*left)
            .map(|file| changed_file_name(&file.path))
            .unwrap_or_default();
        let right_name = files
            .get(*right)
            .map(|file| changed_file_name(&file.path))
            .unwrap_or_default();

        left_name.cmp(right_name)
    });

    for file_index in file_indices {
        let Some(file) = files.get(file_index) else {
            continue;
        };

        rows.push(ChangedFileTreeRow::File(ChangedFileRow {
            file_index,
            name: changed_file_name(&file.path).to_string(),
            depth,
        }));
    }
}

fn compact_folder_chain<'a>(
    parent_path: &str,
    folder_name: &str,
    child_node: &'a ChangedFileTreeNode,
) -> (String, String, &'a ChangedFileTreeNode) {
    let mut path = if parent_path.is_empty() {
        folder_name.to_string()
    } else {
        format!("{parent_path}/{folder_name}")
    };
    let mut name = folder_name.to_string();
    let mut node = child_node;

    while node.files.is_empty() && node.folders.len() == 1 {
        let Some((next_name, next_node)) = node.folders.iter().next() else {
            break;
        };

        path.push('/');
        path.push_str(next_name);
        name.push('/');
        name.push_str(next_name);
        node = next_node;
    }

    (path, name, node)
}

fn changed_file_name(path: &str) -> &str {
    path.rsplit('/')
        .next()
        .filter(|segment| !segment.is_empty())
        .unwrap_or(path)
}
