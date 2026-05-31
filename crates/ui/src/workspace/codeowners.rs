use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use harbor_domain::DiffFile;

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
        return wildcard_matches(normalized_pattern, file_name(path))
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

fn file_name(path: &str) -> &str {
    path.rsplit('/')
        .next()
        .filter(|segment| !segment.is_empty())
        .unwrap_or(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_user_and_team_ownership() {
        let rules = parse_codeowners_rules(
            r#"
            # ignored
            *.rs @acme/octocat
            docs/ @someone-else
            "#,
            "octocat",
        );

        assert_eq!(
            rules,
            vec![
                CodeownersRule {
                    pattern: "*.rs".to_string(),
                    owned_by_current_user: true,
                },
                CodeownersRule {
                    pattern: "docs/".to_string(),
                    owned_by_current_user: false,
                },
            ]
        );
    }

    #[test]
    fn matches_codeowners_patterns_against_paths() {
        assert!(codeowners_pattern_matches_path(
            "*.rs",
            "crates/ui/src/lib.rs"
        ));
        assert!(codeowners_pattern_matches_path("docs/", "docs/guide.md"));
        assert!(codeowners_pattern_matches_path(
            "crates/ui",
            "crates/ui/src/lib.rs"
        ));
        assert!(!codeowners_pattern_matches_path("docs/", "src/docs.rs"));
    }
}
