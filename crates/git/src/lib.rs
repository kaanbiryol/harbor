use std::{
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
    process::{Command, Output},
};

use thiserror::Error;

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

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ExternalApp {
    VsCode,
    Cursor,
    Zed,
    Finder,
    Terminal,
    Ghostty,
    Warp,
    Xcode,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExternalAppKind {
    Editor,
    Finder,
    Terminal,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OpenTarget {
    File(PathBuf),
    Directory(PathBuf),
    Reveal(PathBuf),
}

impl ExternalApp {
    pub const ALL: [Self; 8] = [
        Self::VsCode,
        Self::Cursor,
        Self::Zed,
        Self::Finder,
        Self::Terminal,
        Self::Ghostty,
        Self::Warp,
        Self::Xcode,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::VsCode => "VS Code",
            Self::Cursor => "Cursor",
            Self::Zed => "Zed",
            Self::Finder => "Finder",
            Self::Terminal => "Terminal",
            Self::Ghostty => "Ghostty",
            Self::Warp => "Warp",
            Self::Xcode => "Xcode",
        }
    }

    pub fn kind(self) -> ExternalAppKind {
        match self {
            Self::Finder => ExternalAppKind::Finder,
            Self::Terminal | Self::Ghostty | Self::Warp => ExternalAppKind::Terminal,
            Self::VsCode | Self::Cursor | Self::Zed | Self::Xcode => ExternalAppKind::Editor,
        }
    }

    pub fn is_available(self) -> bool {
        #[cfg(target_os = "macos")]
        {
            if matches!(self, Self::Finder | Self::Terminal) {
                return true;
            }

            application_paths(self.macos_app_name())
                .into_iter()
                .any(|path| path.exists())
        }

        #[cfg(not(target_os = "macos"))]
        {
            false
        }
    }

    fn macos_app_name(self) -> &'static str {
        match self {
            Self::VsCode => "Visual Studio Code",
            Self::Cursor => "Cursor",
            Self::Zed => "Zed",
            Self::Finder => "Finder",
            Self::Terminal => "Terminal",
            Self::Ghostty => "Ghostty",
            Self::Warp => "Warp",
            Self::Xcode => "Xcode",
        }
    }
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
    let local_repository = validate_repository_path(repo_path, owner, repo)?;
    let worktree_path = managed_worktree_path(&local_repository.repo_path, owner, repo, number)?;
    let reference = pr_reference(number);

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

pub fn open_external_app(app: ExternalApp, target: OpenTarget) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        if !app.is_available() {
            return Err(GitError::Command(format!(
                "{} is not installed",
                app.label()
            )));
        }

        let args = macos_open_args(app, &target);
        let output = Command::new("open")
            .args(args.iter().map(OsString::as_os_str))
            .output()
            .map_err(|error| GitError::Command(format!("failed to run open: {error}")))?;

        if output.status.success() {
            Ok(())
        } else {
            Err(GitError::Command(command_output_message("open", &output)))
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = (app, target);
        Err(GitError::UnsupportedPlatform)
    }
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

#[cfg(target_os = "macos")]
fn application_paths(app_name: &str) -> Vec<PathBuf> {
    let mut paths = vec![PathBuf::from("/Applications").join(format!("{app_name}.app"))];

    if let Some(home) = std::env::var_os("HOME") {
        paths.push(
            PathBuf::from(home)
                .join("Applications")
                .join(format!("{app_name}.app")),
        );
    }

    paths
}

#[cfg(target_os = "macos")]
fn macos_open_args(app: ExternalApp, target: &OpenTarget) -> Vec<OsString> {
    match (app, target) {
        (ExternalApp::Finder, OpenTarget::Reveal(path)) => {
            vec![OsString::from("-R"), path.as_os_str().to_os_string()]
        }
        (_, OpenTarget::File(path) | OpenTarget::Directory(path) | OpenTarget::Reveal(path)) => {
            vec![
                OsString::from("-a"),
                OsString::from(app.macos_app_name()),
                path.as_os_str().to_os_string(),
            ]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[cfg(target_os = "macos")]
    #[test]
    fn builds_macos_open_arguments() {
        let args = macos_open_args(
            ExternalApp::Zed,
            &OpenTarget::File(PathBuf::from("/tmp/app/src/main.rs")),
        );

        assert_eq!(
            args,
            vec![
                OsString::from("-a"),
                OsString::from("Zed"),
                OsString::from("/tmp/app/src/main.rs")
            ]
        );

        let args = macos_open_args(
            ExternalApp::Finder,
            &OpenTarget::Reveal(PathBuf::from("/tmp/app/src/main.rs")),
        );

        assert_eq!(
            args,
            vec![OsString::from("-R"), OsString::from("/tmp/app/src/main.rs")]
        );
    }
}
