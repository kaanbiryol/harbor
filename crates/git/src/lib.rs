use std::{
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
    process::{Command, Output},
};

use thiserror::Error;

mod codeowners;
mod external_apps;

pub use codeowners::codeowners_owned_paths;
pub use external_apps::{ExternalApp, ExternalAppKind, OpenTarget, open_external_app};

pub type Result<T> = std::result::Result<T, GitError>;

#[derive(Debug, Error)]
pub enum GitError {
    #[error("local repository was not configured")]
    MissingRepository,
    #[error("local repository path is invalid: {0}")]
    InvalidRepositoryPath(String),
    #[error("no GitHub remote matching {owner}/{repo} was found")]
    RemoteMismatch { owner: String, repo: String },
    #[error("managed worktree has local changes: {0}")]
    DirtyWorktree(PathBuf),
    #[error("managed worktree path has no parent directory: {0}")]
    MissingWorktreeParent(PathBuf),
    #[error("external app launching is only supported on macOS")]
    UnsupportedPlatform,
    #[error("git command failed: {0}")]
    Command(String),
    #[error("failed to read {}: {source}", path.display())]
    ReadFile {
        path: PathBuf,
        source: std::io::Error,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalRepository {
    pub repo_path: PathBuf,
    pub remote_name: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalCheckout {
    pub repo_path: PathBuf,
    pub branch: String,
}

pub fn validate_repository_path(path: &Path, owner: &str, repo: &str) -> Result<LocalRepository> {
    if !path.exists() {
        return Err(GitError::InvalidRepositoryPath(format!(
            "{} does not exist",
            path.display()
        )));
    }

    if !path.is_dir() {
        return Err(GitError::InvalidRepositoryPath(format!(
            "{} is not a directory",
            path.display()
        )));
    }

    let root = git_stdout(path, ["rev-parse", "--show-toplevel"])?;
    let repo_path = PathBuf::from(root.trim());
    let remote_name = matching_remote(&repo_path, owner, repo)?;

    Ok(LocalRepository {
        repo_path,
        remote_name,
    })
}

pub fn create_or_update_pr_worktree(
    repo_path: &Path,
    owner: &str,
    repo: &str,
    number: u64,
) -> Result<PathBuf> {
    create_or_update_pr_worktree_in(repo_path, owner, repo, number, None)
}

/// Prepares a PR worktree under a custom root, or uses Harbor's default location.
pub fn create_or_update_pr_worktree_in(
    repo_path: &Path,
    owner: &str,
    repo: &str,
    number: u64,
    directory: Option<&Path>,
) -> Result<PathBuf> {
    let local_repository = validate_repository_path(repo_path, owner, repo)?;
    let worktree_path = match directory {
        Some(directory) if directory.is_absolute() => directory
            .join(sanitize_path_part(owner))
            .join(sanitize_path_part(repo))
            .join(format!("pr-{number}")),
        Some(_) => {
            return Err(GitError::Command(
                "worktree directory must be an absolute path".to_string(),
            ));
        }
        None => managed_worktree_path(&local_repository.repo_path, owner, repo, number)?,
    };
    let reference = pr_reference(number);

    if worktree_path.exists() {
        let common_directory = |path: &Path| -> Result<PathBuf> {
            let directory = git_stdout(
                path,
                ["rev-parse", "--path-format=absolute", "--git-common-dir"],
            )?;
            std::fs::canonicalize(directory.trim()).map_err(|error| {
                GitError::Command(format!("could not resolve worktree repository: {error}"))
            })
        };
        if common_directory(&worktree_path)? != common_directory(&local_repository.repo_path)? {
            return Err(GitError::Command(
                "worktree folder belongs to another local repository".to_string(),
            ));
        }
    }

    git_success(
        &local_repository.repo_path,
        [
            OsString::from("fetch"),
            OsString::from(local_repository.remote_name),
            OsString::from(format!("pull/{number}/head:{reference}")),
        ],
    )?;

    if worktree_path.exists() {
        if worktree_has_changes(&worktree_path)? {
            return Err(GitError::DirtyWorktree(worktree_path));
        }

        git_success(
            &worktree_path,
            [
                OsString::from("checkout"),
                OsString::from("--detach"),
                OsString::from(reference),
            ],
        )?;
    } else {
        let Some(parent) = worktree_path.parent() else {
            return Err(GitError::MissingWorktreeParent(worktree_path));
        };
        std::fs::create_dir_all(parent).map_err(|error| {
            GitError::Command(format!(
                "failed to create worktree directory {}: {error}",
                parent.display()
            ))
        })?;

        git_success(
            &local_repository.repo_path,
            [
                OsString::from("worktree"),
                OsString::from("add"),
                OsString::from("--detach"),
                worktree_path.as_os_str().to_os_string(),
                OsString::from(reference),
            ],
        )?;
    }

    Ok(worktree_path)
}

pub fn managed_worktree_path(
    repo_path: &Path,
    owner: &str,
    repo: &str,
    number: u64,
) -> Result<PathBuf> {
    let Some(parent) = repo_path.parent() else {
        return Err(GitError::MissingWorktreeParent(repo_path.to_path_buf()));
    };

    Ok(parent
        .join(".harbor-worktrees")
        .join(format!(
            "{}-{}",
            sanitize_path_part(owner),
            sanitize_path_part(repo)
        ))
        .join(format!("pr-{number}")))
}

pub fn remote_matches_repository(url: &str, owner: &str, repo: &str) -> bool {
    let Some(path) = github_remote_path(url) else {
        return false;
    };
    let path = path.trim_matches('/').trim_end_matches(".git");
    let mut parts = path.split('/');
    let Some(remote_owner) = parts.next() else {
        return false;
    };
    let Some(remote_repo) = parts.next() else {
        return false;
    };

    parts.next().is_none()
        && remote_owner.eq_ignore_ascii_case(owner)
        && remote_repo.eq_ignore_ascii_case(repo)
}

pub fn worktree_has_changes(worktree_path: &Path) -> Result<bool> {
    let status = git_stdout(
        worktree_path,
        ["status", "--porcelain", "--untracked-files=normal"],
    )?;

    Ok(status_output_is_dirty(&status))
}

fn matching_remote(repo_path: &Path, owner: &str, repo: &str) -> Result<String> {
    let remotes = git_stdout(repo_path, ["remote", "-v"])?;

    remotes
        .lines()
        .filter_map(|line| {
            let mut columns = line.split_whitespace();
            let name = columns.next()?;
            let url = columns.next()?;
            remote_matches_repository(url, owner, repo).then(|| name.to_string())
        })
        .next()
        .ok_or_else(|| GitError::RemoteMismatch {
            owner: owner.to_string(),
            repo: repo.to_string(),
        })
}

fn git_stdout<I, S>(repo_path: &Path, args: I) -> Result<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = git_output(repo_path, args)?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(GitError::Command(command_output_message("git", &output)))
    }
}

fn git_success<I, S>(repo_path: &Path, args: I) -> Result<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = git_output(repo_path, args)?;
    if output.status.success() {
        Ok(())
    } else {
        Err(GitError::Command(command_output_message("git", &output)))
    }
}

fn git_output<I, S>(repo_path: &Path, args: I) -> Result<Output>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .args(args)
        .output()
        .map_err(|error| GitError::Command(format!("failed to run git: {error}")))
}

fn command_output_message(command: &str, output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let detail = if !stderr.trim().is_empty() {
        stderr.trim()
    } else {
        stdout.trim()
    };

    if detail.is_empty() {
        format!("{command} exited with {}", output.status)
    } else {
        detail.to_string()
    }
}

fn github_remote_path(url: &str) -> Option<&str> {
    let url = url.trim().trim_end_matches('/');

    if let Some(path) = url.strip_prefix("git@github.com:") {
        return Some(path);
    }

    if let Some(path) = url.strip_prefix("ssh://git@github.com/") {
        return Some(path);
    }

    url.split_once("github.com/").map(|(_, path)| path)
}

fn pr_reference(number: u64) -> String {
    format!("refs/harbor/pr/{number}")
}

fn sanitize_path_part(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') {
                character
            } else {
                '-'
            }
        })
        .collect()
}

fn status_output_is_dirty(status: &str) -> bool {
    !status.trim().is_empty()
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn matches_common_github_remote_urls() {
        assert!(remote_matches_repository(
            "https://github.com/acme/app.git",
            "acme",
            "app"
        ));
        assert!(remote_matches_repository(
            "git@github.com:Acme/App.git",
            "acme",
            "app"
        ));
        assert!(remote_matches_repository(
            "ssh://git@github.com/acme/app",
            "acme",
            "app"
        ));
        assert!(!remote_matches_repository(
            "https://github.com/acme/other.git",
            "acme",
            "app"
        ));
    }

    #[test]
    fn builds_managed_worktree_path_next_to_repository() {
        let path = managed_worktree_path(Path::new("/Users/me/app"), "acme", "app", 42)
            .expect("worktree path");

        assert_eq!(
            path,
            PathBuf::from("/Users/me/.harbor-worktrees/acme-app/pr-42")
        );
    }

    #[test]
    fn detects_dirty_status_output() {
        assert!(!status_output_is_dirty(""));
        assert!(!status_output_is_dirty("   \n"));
        assert!(status_output_is_dirty(" M src/main.rs\n"));
        assert!(status_output_is_dirty("?? src/new.rs\n"));
    }

    #[test]
    fn validates_repository_paths_against_the_github_remote() {
        let fixture = LocalGitFixture::new("validate-repository");
        fixture.commit_and_publish_pull_request("first\n");
        let nested = fixture.source.join("nested");
        std::fs::create_dir_all(&nested).expect("create nested directory");

        let repository = validate_repository_path(&nested, "Acme", "App").expect("valid repo");
        assert_eq!(
            repository.repo_path.canonicalize().expect("repo path"),
            fixture.source.canonicalize().expect("source path")
        );
        assert_eq!(repository.remote_name, "origin");

        assert!(matches!(
            validate_repository_path(&fixture.source, "acme", "other"),
            Err(GitError::RemoteMismatch { .. })
        ));
        assert!(matches!(
            validate_repository_path(&fixture.root.join("missing"), "acme", "app"),
            Err(GitError::InvalidRepositoryPath(_))
        ));
        assert!(matches!(
            validate_repository_path(&fixture.source.join("README.md"), "acme", "app"),
            Err(GitError::InvalidRepositoryPath(_))
        ));
        assert!(matches!(
            validate_repository_path(&fixture.root, "acme", "app"),
            Err(GitError::Command(_))
        ));

        fixture.cleanup();
    }

    #[test]
    fn reports_tracked_and_untracked_worktree_changes() {
        let fixture = LocalGitFixture::new("worktree-changes");
        fixture.commit_and_publish_pull_request("first\n");
        assert!(!worktree_has_changes(&fixture.source).expect("clean status"));

        std::fs::write(fixture.source.join("new.txt"), "new\n").expect("untracked file");
        assert!(worktree_has_changes(&fixture.source).expect("untracked status"));

        std::fs::remove_file(fixture.source.join("new.txt")).expect("remove untracked file");
        std::fs::write(fixture.source.join("README.md"), "edited\n").expect("edit file");
        assert!(worktree_has_changes(&fixture.source).expect("modified status"));

        fixture.cleanup();
    }

    #[test]
    fn creates_and_updates_pull_request_worktrees_from_a_local_remote() {
        let fixture = LocalGitFixture::new("worktree-update");
        let first_sha = fixture.commit_and_publish_pull_request("first\n");

        let worktree = create_or_update_pr_worktree(&fixture.source, "acme", "app", 7)
            .expect("create pull request worktree");
        assert_eq!(
            git_stdout(&worktree, ["rev-parse", "HEAD"]).expect("worktree head"),
            first_sha
        );

        let second_sha = fixture.commit_and_publish_pull_request("second\n");
        let updated_worktree = create_or_update_pr_worktree(&fixture.source, "acme", "app", 7)
            .expect("update pull request worktree");
        assert_eq!(updated_worktree, worktree);
        assert_eq!(
            git_stdout(&worktree, ["rev-parse", "HEAD"]).expect("updated worktree head"),
            second_sha
        );

        fixture.cleanup();
    }

    #[test]
    fn refuses_to_update_a_dirty_pull_request_worktree() {
        let fixture = LocalGitFixture::new("dirty-worktree");
        fixture.commit_and_publish_pull_request("first\n");
        let worktree = create_or_update_pr_worktree(&fixture.source, "acme", "app", 7)
            .expect("create pull request worktree");
        std::fs::write(worktree.join("README.md"), "dirty\n").expect("dirty worktree file");

        let result = create_or_update_pr_worktree(&fixture.source, "acme", "app", 7);
        assert_eq!(
            result.expect_err("dirty worktree should fail").to_string(),
            format!("managed worktree has local changes: {}", worktree.display())
        );

        fixture.cleanup();
    }

    #[test]
    fn custom_worktree_directory_creates_updates_and_preserves_local_changes() {
        let fixture = LocalGitFixture::new("custom-worktree");
        fixture.commit_and_publish_pull_request("first\n");
        let directory = fixture.root.join("custom worktrees");
        let worktree =
            create_or_update_pr_worktree_in(&fixture.source, "acme", "app", 7, Some(&directory))
                .expect("custom worktree");
        assert_eq!(worktree, directory.join("acme/app/pr-7"));
        let second_sha = fixture.commit_and_publish_pull_request("second\n");
        assert_eq!(
            create_or_update_pr_worktree_in(&fixture.source, "acme", "app", 7, Some(&directory))
                .expect("update custom worktree"),
            worktree
        );
        assert_eq!(
            git_stdout(&worktree, ["rev-parse", "HEAD"]).expect("worktree head"),
            second_sha
        );
        std::fs::write(worktree.join("README.md"), "local changes\n").expect("modify file");
        assert!(matches!(
            create_or_update_pr_worktree_in(&fixture.source, "acme", "app", 7, Some(&directory)),
            Err(GitError::DirtyWorktree(_))
        ));
        fixture.cleanup();
    }

    #[test]
    fn refuses_custom_worktree_owned_by_another_checkout() {
        let fixture = LocalGitFixture::new("foreign-worktree");
        let other = LocalGitFixture::new("other-checkout");
        fixture.commit_and_publish_pull_request("first\n");
        other.commit_and_publish_pull_request("other\n");
        let directory = fixture.root.join("custom");
        let worktree =
            create_or_update_pr_worktree_in(&other.source, "acme", "app", 7, Some(&directory))
                .expect("other worktree");
        let previous_head = git_stdout(&worktree, ["rev-parse", "HEAD"]).expect("other head");
        let error =
            create_or_update_pr_worktree_in(&fixture.source, "acme", "app", 7, Some(&directory))
                .expect_err("foreign worktree rejected");
        assert!(error.to_string().contains("another local repository"));
        assert_eq!(
            git_stdout(&worktree, ["rev-parse", "HEAD"]).expect("unchanged head"),
            previous_head
        );
        other.cleanup();
        fixture.cleanup();
    }

    struct LocalGitFixture {
        root: PathBuf,
        source: PathBuf,
    }

    impl LocalGitFixture {
        fn new(name: &str) -> Self {
            let sequence = TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "harbor-git-{name}-{}-{sequence}",
                std::process::id()
            ));
            let remote = root.join("github.com").join("acme").join("app.git");
            let source = root.join("source");
            std::fs::create_dir_all(remote.parent().expect("remote parent"))
                .expect("create git fixture root");
            run_git(
                &root,
                ["init", "--bare", remote.to_str().expect("remote path")],
            );
            run_git(&root, ["init", source.to_str().expect("source path")]);
            run_git(&source, ["config", "user.name", "Harbor Tests"]);
            run_git(&source, ["config", "user.email", "harbor@example.com"]);
            run_git(
                &source,
                [
                    "remote",
                    "add",
                    "origin",
                    remote.to_str().expect("remote path"),
                ],
            );

            Self { root, source }
        }

        fn commit_and_publish_pull_request(&self, contents: &str) -> String {
            std::fs::write(self.source.join("README.md"), contents).expect("write fixture file");
            run_git(&self.source, ["add", "README.md"]);
            run_git(&self.source, ["commit", "-m", "fixture"]);
            let sha = git_stdout(&self.source, ["rev-parse", "HEAD"]).expect("fixture head");
            let pull_request_ref = format!("{sha}:refs/pull/7/head");
            run_git(
                &self.source,
                ["push", "--force", "origin", &pull_request_ref],
            );
            sha
        }

        fn cleanup(self) {
            std::fs::remove_dir_all(&self.root).expect("remove git fixture");
        }
    }

    fn run_git<const N: usize>(directory: &Path, args: [&str; N]) {
        let output = Command::new("git")
            .arg("-C")
            .arg(directory)
            .args(args)
            .output()
            .expect("run git fixture command");
        assert!(
            output.status.success(),
            "git fixture command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
