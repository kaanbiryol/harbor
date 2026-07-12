#[path = "state/detail.rs"]
mod detail;
#[path = "state/inbox.rs"]
mod inbox;
#[path = "state/notification.rs"]
mod notification;
#[path = "state/overview.rs"]
mod overview;
#[path = "state/panel_lists.rs"]
mod panel_lists;
#[path = "state/repository.rs"]
mod repository;
#[path = "state/repository_actions.rs"]
mod repository_actions;
#[path = "state/review_composer.rs"]
mod review_composer;
#[path = "state/review_runtime.rs"]
mod review_runtime;
#[path = "state/selection.rs"]
mod selection;
#[path = "state/sync_runtime.rs"]
mod sync_runtime;
#[path = "state/tasks.rs"]
mod tasks;
#[path = "state/workflow_log.rs"]
mod workflow_log;

pub(crate) use detail::{PullRequestDetailLoadedState, PullRequestDetailUiState};
pub(crate) use inbox::{PullRequestInboxState, PullRequestRowEnrichmentKey};
pub(crate) use notification::NotificationState;
pub(crate) use overview::{OverviewMarkdownState, OverviewUiState};
pub(crate) use panel_lists::PanelListState;
pub(crate) use repository::RepositoryUiState;
pub(crate) use repository_actions::RepositoryActionsUiState;
#[cfg(test)]
pub(crate) use review_composer::ReviewComposerMode;
pub(crate) use review_composer::ReviewComposerState;
pub(crate) use review_runtime::ReviewRuntimeState;
pub(crate) use selection::PullRequestSelectionState;
pub(crate) use sync_runtime::SyncRuntimeState;
pub(crate) use tasks::WorkspaceTasks;
pub(crate) use workflow_log::WorkflowLogState;

#[cfg(test)]
mod tests {
    use super::*;
    use harbor_domain::{ReviewCommentRange, ReviewSide};

    use crate::workspace::{ReviewComposer, ReviewLineSelection, ReviewLineTarget};

    #[test]
    fn pull_request_detail_state_tracks_section_load_transitions() {
        let mut state =
            PullRequestDetailUiState::new(Vec::new(), Vec::new(), WorkflowLogState::new());

        assert!(state.should_load_details());
        state.start_details_load();
        assert!(state.details_loading());
        assert!(!state.should_load_details());

        state.apply_details_failure("metadata failed");
        assert_eq!(state.details_error(), Some("metadata failed"));
        assert!(!state.should_load_details());

        state.reset_for_selection();
        assert!(state.should_load_details());

        state.restore_loaded_sections(PullRequestDetailLoadedState {
            details: true,
            files: false,
            checks: true,
            commits: true,
            workflows: false,
            reviews: true,
        });
        assert!(state.details_loaded());
        assert!(!state.should_load_checks());
        assert_eq!(
            state.loaded_sections(true),
            PullRequestDetailLoadedState {
                details: true,
                files: false,
                checks: true,
                commits: true,
                workflows: false,
                reviews: true,
            }
        );
    }

    #[test]
    fn pull_request_selection_state_restores_indexes_with_bounds() {
        let mut state = PullRequestSelectionState::default();

        state.restore_pull_request_index(4, 2);
        assert_eq!(state.pull_request_index(), 1);

        state.set_diff_position(2, 3);
        assert_eq!(state.file_index(), 2);
        assert_eq!(state.hunk_index(), 3);

        state.restore_diff_position(5, 8, 3);
        assert_eq!(state.file_index(), 2);
        assert_eq!(state.hunk_index(), 8);

        state.select_file_index(1);
        assert_eq!(state.file_index(), 1);
        assert_eq!(state.hunk_index(), 0);

        state.reset_pull_request_index();
        state.reset_diff_selection();
        assert_eq!(state, PullRequestSelectionState::default());
    }

    #[test]
    fn review_composer_modes_are_mutually_exclusive() {
        let target = ReviewLineTarget {
            hunk_index: 0,
            line_index: 1,
            range: ReviewCommentRange {
                path: "src/lib.rs".to_string(),
                line: 10,
                side: ReviewSide::Right,
                start_line: None,
                start_side: None,
            },
        };
        let selection = ReviewLineSelection {
            anchor: target.clone(),
            current: target.clone(),
        };
        let composer = ReviewComposer {
            anchor: target,
            range: selection.current.range.clone(),
        };

        let modes = [
            ReviewComposerMode::Idle,
            ReviewComposerMode::Selecting {
                line_selection: selection.clone(),
            },
            ReviewComposerMode::Inline {
                composer,
                line_selection: selection,
            },
            ReviewComposerMode::ThreadReply {
                thread_id: "thread-1".to_string(),
            },
            ReviewComposerMode::CommentEdit {
                comment_id: "comment-1".to_string(),
            },
        ];

        for mode in modes {
            let active_count = [
                matches!(mode, ReviewComposerMode::Selecting { .. }),
                matches!(mode, ReviewComposerMode::Inline { .. }),
                matches!(mode, ReviewComposerMode::ThreadReply { .. }),
                matches!(mode, ReviewComposerMode::CommentEdit { .. }),
            ]
            .into_iter()
            .filter(|active| *active)
            .count();
            assert!(active_count <= 1);
        }
    }
}
