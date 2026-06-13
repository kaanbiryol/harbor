use std::{
    env,
    ffi::OsStr,
    path::{Path, PathBuf},
    process::Command,
};

const GH_EXECUTABLE: &str = "gh";
const HARBOR_GH_PATH_ENV: &str = "HARBOR_GH_PATH";
const MACOS_GH_PATHS: &[&str] = &[
    "/opt/homebrew/bin/gh",
    "/usr/local/bin/gh",
    "/opt/local/bin/gh",
];

pub(super) fn gh_command() -> Command {
    Command::new(resolve_gh_executable())
}

fn resolve_gh_executable() -> PathBuf {
    resolve_gh_executable_with(
        env::var_os(HARBOR_GH_PATH_ENV).as_deref(),
        env::var_os("PATH").as_deref(),
        fallback_gh_paths(),
        is_executable_file,
    )
}

fn fallback_gh_paths() -> Vec<PathBuf> {
    let mut paths = MACOS_GH_PATHS.iter().map(PathBuf::from).collect::<Vec<_>>();

    if let Some(home) = env::var_os("HOME")
        && !home.is_empty()
    {
        append_home_fallback_paths(&mut paths, &PathBuf::from(home));
    }

    paths
}

fn append_home_fallback_paths(paths: &mut Vec<PathBuf>, home: &Path) {
    paths.push(home.join(".local/share/mise/shims/gh"));
    paths.push(home.join(".local/share/rtx/shims/gh"));
    paths.push(home.join(".asdf/shims/gh"));
    paths.push(home.join(".nix-profile/bin/gh"));
}

fn resolve_gh_executable_with<I, F>(
    configured_path: Option<&OsStr>,
    path_value: Option<&OsStr>,
    fallback_paths: I,
    is_executable: F,
) -> PathBuf
where
    I: IntoIterator,
    I::Item: AsRef<Path>,
    F: Fn(&Path) -> bool,
{
    if let Some(path) = configured_path
        && !path.is_empty()
    {
        return PathBuf::from(path);
    }

    if let Some(path) = path_value {
        for directory in env::split_paths(path) {
            let candidate = directory.join(GH_EXECUTABLE);
            if is_executable(&candidate) {
                return candidate;
            }
        }
    }

    for candidate in fallback_paths {
        let candidate = candidate.as_ref();
        if is_executable(candidate) {
            return candidate.to_path_buf();
        }
    }

    PathBuf::from(GH_EXECUTABLE)
}

fn is_executable_file(path: &Path) -> bool {
    let Ok(metadata) = path.metadata() else {
        return false;
    };

    if !metadata.is_file() {
        return false;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        metadata.permissions().mode() & 0o111 != 0
    }

    #[cfg(not(unix))]
    {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_gh_from_path_before_fallbacks() {
        let path = env::join_paths(["/usr/bin", "/custom/bin"]).unwrap();
        let fallback = [PathBuf::from("/opt/homebrew/bin/gh")];

        let resolved = resolve_gh_executable_with(None, Some(&path), fallback, |path| {
            path == Path::new("/custom/bin/gh") || path == Path::new("/opt/homebrew/bin/gh")
        });

        assert_eq!(resolved, PathBuf::from("/custom/bin/gh"));
    }

    #[test]
    fn resolves_gh_from_macos_fallbacks_when_path_is_missing_it() {
        let path = env::join_paths(["/usr/bin", "/bin"]).unwrap();
        let fallback = [PathBuf::from("/opt/homebrew/bin/gh")];

        let resolved = resolve_gh_executable_with(None, Some(&path), fallback, |path| {
            path == Path::new("/opt/homebrew/bin/gh")
        });

        assert_eq!(resolved, PathBuf::from("/opt/homebrew/bin/gh"));
    }

    #[test]
    fn configured_gh_path_takes_priority() {
        let path = env::join_paths(["/usr/bin", "/custom/bin"]).unwrap();
        let fallback = [PathBuf::from("/opt/homebrew/bin/gh")];

        let resolved = resolve_gh_executable_with(
            Some(OsStr::new("/configured/gh")),
            Some(&path),
            fallback,
            |_| true,
        );

        assert_eq!(resolved, PathBuf::from("/configured/gh"));
    }

    #[test]
    fn falls_back_to_gh_command_name_when_unresolved() {
        let path = env::join_paths(["/usr/bin", "/bin"]).unwrap();
        let fallback = [PathBuf::from("/opt/homebrew/bin/gh")];

        let resolved = resolve_gh_executable_with(None, Some(&path), fallback, |_| false);

        assert_eq!(resolved, PathBuf::from("gh"));
    }

    #[test]
    fn fallback_paths_include_version_manager_shims() {
        let mut paths = Vec::new();
        append_home_fallback_paths(&mut paths, Path::new("/Users/octocat"));

        assert!(paths.contains(&PathBuf::from("/Users/octocat/.local/share/mise/shims/gh")));
        assert!(paths.contains(&PathBuf::from("/Users/octocat/.asdf/shims/gh")));
    }
}
