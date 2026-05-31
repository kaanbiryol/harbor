#[cfg(target_os = "macos")]
use std::ffi::OsString;
use std::path::PathBuf;
#[cfg(target_os = "macos")]
use std::process::Command;

use crate::{GitError, Result};

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

    #[cfg(target_os = "macos")]
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
            Err(GitError::Command(crate::command_output_message(
                "open", &output,
            )))
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = (app, target);
        Err(GitError::UnsupportedPlatform)
    }
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

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

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
