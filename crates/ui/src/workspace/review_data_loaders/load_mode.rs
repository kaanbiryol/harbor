#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::workspace) enum ReviewDataLoadMode {
    Initial,
    Refresh,
}

impl ReviewDataLoadMode {
    pub(super) fn loaded_review_data_status(
        self,
        number: u64,
        thread_count: usize,
        has_warnings: bool,
    ) -> String {
        match (self, has_warnings) {
            (Self::Initial, false) => {
                format!("Loaded review history and {thread_count} threads for PR #{number}")
            }
            (Self::Initial, true) => {
                format!(
                    "Loaded {thread_count} review threads for PR #{number}, with review warnings"
                )
            }
            (Self::Refresh, false) => {
                format!("Refreshed review data and {thread_count} threads for PR #{number}")
            }
            (Self::Refresh, true) => {
                format!(
                    "Refreshed review data and {thread_count} threads for PR #{number}, with warnings"
                )
            }
        }
    }

    pub(super) fn loaded_threads_only_status(self, number: u64, thread_count: usize) -> String {
        match self {
            Self::Initial => {
                format!(
                    "Loaded {thread_count} review threads for PR #{number}, with review warnings"
                )
            }
            Self::Refresh => {
                format!(
                    "Refreshed {thread_count} review threads for PR #{number}, but review history failed"
                )
            }
        }
    }

    pub(super) fn loaded_reviews_only_status(self, number: u64) -> String {
        match self {
            Self::Initial => format!("Failed to load review data for PR #{number}"),
            Self::Refresh => {
                format!("Refreshed review history for PR #{number}, but threads failed")
            }
        }
    }

    pub(super) fn failed_status(self, number: u64) -> String {
        match self {
            Self::Initial => format!("Failed to load review data for PR #{number}"),
            Self::Refresh => format!("Failed to refresh review data for PR #{number}"),
        }
    }

    pub(super) fn update_error_log_message(self) -> &'static str {
        match self {
            Self::Initial => "failed to update pull request review state",
            Self::Refresh => "failed to update refreshed review state",
        }
    }
}
