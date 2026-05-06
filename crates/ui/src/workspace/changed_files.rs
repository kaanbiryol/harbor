use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use harbor_domain::{DiffFile, FileStatus};

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
    pub(crate) query: String,
    pub(crate) excluded_file_types: HashSet<String>,
    pub(crate) owned_by_current_user_only: bool,
    pub(crate) owned_file_paths: HashSet<String>,
}

impl ChangedFileFilters {
    fn has_active_filter(&self) -> bool {
        !self.query.is_empty()
            || !self.excluded_file_types.is_empty()
            || self.owned_by_current_user_only
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
    let filters = ChangedFileFilters {
        query: normalized_search_query(&filters.query),
        excluded_file_types: filters.excluded_file_types.clone(),
        owned_by_current_user_only: filters.owned_by_current_user_only,
        owned_file_paths: filters.owned_file_paths.clone(),
    };
    let mut root = ChangedFileTreeNode::default();

    for (file_index, file) in files.iter().enumerate() {
        if !changed_file_matches_filters(file, &filters) {
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

    changed_file_matches_query(file, &filters.query)
}

pub(crate) fn changed_file_matches_query(file: &DiffFile, query: &str) -> bool {
    let query = normalized_search_query(query);

    if query.is_empty() {
        return true;
    }

    if file.path.to_lowercase().contains(&query) {
        return true;
    }

    if file
        .previous_path
        .as_deref()
        .map(|path| path.to_lowercase().contains(&query))
        .unwrap_or(false)
    {
        return true;
    }

    changed_file_status_label(file.status).contains(&query)
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

pub(super) fn codeowners_owned_file_paths(
    repository_path: &Path,
    files: &[DiffFile],
    current_user_login: &str,
) -> Result<HashSet<String>, String> {
    let Some(codeowners_path) = codeowners_path(repository_path) else {
        return Ok(HashSet::new());
    };
    let contents = fs::read_to_string(&codeowners_path)
        .map_err(|error| format!("failed to read {}: {error}", codeowners_path.display()))?;
    let rules = parse_codeowners_rules(&contents, current_user_login);
    if rules.is_empty() {
        return Ok(HashSet::new());
    }

    let mut owned_paths = HashSet::new();
    for file in files {
        let mut owned = false;

        for rule in &rules {
            if codeowners_pattern_matches_path(&rule.pattern, &file.path) {
                owned = rule.owned_by_current_user;
            }
        }

        if owned {
            owned_paths.insert(file.path.clone());
        }
    }

    Ok(owned_paths)
}

fn codeowners_path(repository_path: &Path) -> Option<PathBuf> {
    [
        repository_path.join(".github").join("CODEOWNERS"),
        repository_path.join("CODEOWNERS"),
        repository_path.join("docs").join("CODEOWNERS"),
    ]
    .into_iter()
    .find(|path| path.is_file())
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CodeownersRule {
    pattern: String,
    owned_by_current_user: bool,
}

fn parse_codeowners_rules(contents: &str, current_user_login: &str) -> Vec<CodeownersRule> {
    contents
        .lines()
        .filter_map(|line| parse_codeowners_rule(line, current_user_login))
        .collect()
}

fn parse_codeowners_rule(line: &str, current_user_login: &str) -> Option<CodeownersRule> {
    let line = line.split('#').next().unwrap_or_default().trim();
    if line.is_empty() {
        return None;
    }

    let mut parts = line.split_whitespace();
    let pattern = parts.next()?.trim();
    let owned_by_current_user =
        parts.any(|owner| codeowner_matches_user(owner, current_user_login));

    Some(CodeownersRule {
        pattern: pattern.to_string(),
        owned_by_current_user,
    })
}

fn codeowner_matches_user(owner: &str, current_user_login: &str) -> bool {
    let owner = owner.trim().trim_start_matches('@');
    owner == current_user_login
        || owner
            .rsplit('/')
            .next()
            .map(|segment| segment == current_user_login)
            .unwrap_or(false)
}

fn codeowners_pattern_matches_path(pattern: &str, path: &str) -> bool {
    let normalized_pattern = pattern.trim().trim_start_matches('/');
    if normalized_pattern.is_empty() {
        return false;
    }

    if let Some(directory_pattern) = normalized_pattern.strip_suffix('/') {
        return path == directory_pattern || path.starts_with(&format!("{directory_pattern}/"));
    }

    if !normalized_pattern.contains('/') {
        return wildcard_matches(normalized_pattern, changed_file_name(path))
            || path
                .split('/')
                .any(|segment| wildcard_matches(normalized_pattern, segment));
    }

    wildcard_matches(normalized_pattern, path)
        || path == normalized_pattern
        || path.starts_with(&format!("{normalized_pattern}/"))
}

fn wildcard_matches(pattern: &str, value: &str) -> bool {
    wildcard_matches_bytes(pattern.as_bytes(), value.as_bytes())
}

fn wildcard_matches_bytes(pattern: &[u8], value: &[u8]) -> bool {
    match pattern.split_first() {
        None => value.is_empty(),
        Some((b'*', remaining_pattern)) => {
            wildcard_matches_bytes(remaining_pattern, value)
                || value
                    .split_first()
                    .map(|(_, remaining_value)| wildcard_matches_bytes(pattern, remaining_value))
                    .unwrap_or(false)
        }
        Some((b'?', remaining_pattern)) => value
            .split_first()
            .map(|(_, remaining_value)| wildcard_matches_bytes(remaining_pattern, remaining_value))
            .unwrap_or(false),
        Some((expected, remaining_pattern)) => value
            .split_first()
            .map(|(actual, remaining_value)| {
                expected == actual && wildcard_matches_bytes(remaining_pattern, remaining_value)
            })
            .unwrap_or(false),
    }
}

pub(crate) fn changed_file_status_label(status: FileStatus) -> &'static str {
    match status {
        FileStatus::Added => "added",
        FileStatus::Modified => "modified",
        FileStatus::Removed => "removed",
        FileStatus::Renamed => "renamed",
        FileStatus::Copied => "copied",
        FileStatus::Changed => "changed",
        FileStatus::Unchanged => "unchanged",
    }
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
        let folder_path = if parent_path.is_empty() {
            folder_name.clone()
        } else {
            format!("{parent_path}/{folder_name}")
        };
        let expanded = force_expanded || !collapsed_folders.contains(&folder_path);

        rows.push(ChangedFileTreeRow::Folder(ChangedFileFolderRow {
            path: folder_path.clone(),
            name: folder_name.clone(),
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

fn changed_file_name(path: &str) -> &str {
    path.rsplit('/')
        .next()
        .filter(|segment| !segment.is_empty())
        .unwrap_or(path)
}

fn normalized_search_query(query: &str) -> String {
    query.trim().to_lowercase()
}
