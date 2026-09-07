use std::{path::PathBuf, time::Duration};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppPreferences {
    pub notifications: NotificationPreferences,
    pub focused_refresh: RefreshInterval,
    pub background_refresh: RefreshInterval,
    pub code_font_size: f32,
    pub wrap_diff_lines: bool,
    pub editor: PreferredEditor,
    pub terminal: PreferredTerminal,
    pub worktree_directory: Option<PathBuf>,
}

impl Default for AppPreferences {
    fn default() -> Self {
        Self {
            notifications: NotificationPreferences::default(),
            focused_refresh: RefreshInterval::FiveMinutes,
            background_refresh: RefreshInterval::ThirtyMinutes,
            code_font_size: 12.5,
            wrap_diff_lines: true,
            editor: PreferredEditor::VsCode,
            terminal: PreferredTerminal::Terminal,
            worktree_directory: None,
        }
    }
}

impl AppPreferences {
    pub fn normalize(&mut self) {
        self.code_font_size = if self.code_font_size.is_finite() {
            self.code_font_size.clamp(10.0, 24.0)
        } else {
            Self::default().code_font_size
        };
        if self
            .worktree_directory
            .as_ref()
            .is_some_and(|path| !path.is_absolute())
        {
            self.worktree_directory = None;
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NotificationPreferences {
    pub enabled: bool,
    pub checks_failed: bool,
    pub checks_passed: bool,
    pub approvals: bool,
    pub changes_requested: bool,
    pub review_activity: bool,
    pub pull_request_updates: bool,
}

impl Default for NotificationPreferences {
    fn default() -> Self {
        Self {
            enabled: true,
            checks_failed: true,
            checks_passed: true,
            approvals: true,
            changes_requested: true,
            review_activity: true,
            pull_request_updates: true,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefreshInterval {
    Manual,
    OneMinute,
    FiveMinutes,
    FifteenMinutes,
    ThirtyMinutes,
    OneHour,
}

impl RefreshInterval {
    pub const ALL: [Self; 6] = [
        Self::Manual,
        Self::OneMinute,
        Self::FiveMinutes,
        Self::FifteenMinutes,
        Self::ThirtyMinutes,
        Self::OneHour,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Manual => "Manual only",
            Self::OneMinute => "Every minute",
            Self::FiveMinutes => "Every 5 minutes",
            Self::FifteenMinutes => "Every 15 minutes",
            Self::ThirtyMinutes => "Every 30 minutes",
            Self::OneHour => "Every hour",
        }
    }

    pub fn duration(self) -> Option<Duration> {
        let minutes = match self {
            Self::Manual => return None,
            Self::OneMinute => 1,
            Self::FiveMinutes => 5,
            Self::FifteenMinutes => 15,
            Self::ThirtyMinutes => 30,
            Self::OneHour => 60,
        };
        Some(Duration::from_secs(minutes * 60))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreferredEditor {
    VsCode,
    Cursor,
    Zed,
    Xcode,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreferredTerminal {
    Terminal,
    Ghostty,
    Warp,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_invalid_display_and_directory_preferences() {
        let mut preferences = AppPreferences {
            code_font_size: f32::NAN,
            worktree_directory: Some(PathBuf::from("relative/path")),
            ..AppPreferences::default()
        };
        preferences.normalize();
        assert_eq!(preferences.code_font_size, 12.5);
        assert_eq!(preferences.worktree_directory, None);
        preferences.code_font_size = 200.0;
        preferences.normalize();
        assert_eq!(preferences.code_font_size, 24.0);
    }
}
