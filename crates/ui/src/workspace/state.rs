mod changed_files;
mod checks;
mod detail;
mod inbox;
mod notification;
mod overview;
mod panel_lists;
mod pull_request_search;
mod repository;
mod repository_actions;
mod selection;
mod sync_runtime;
mod tasks;
pub(crate) use changed_files::ChangedFilesUiState;
pub(crate) use checks::ChecksUiState;
pub(crate) use detail::{PullRequestDetailLoadedState, PullRequestDetailUiState};
pub(crate) use inbox::{PullRequestInboxState, PullRequestRowEnrichmentKey};
pub(crate) use notification::NotificationState;
pub(crate) use overview::{OverviewMarkdownState, OverviewUiState};
pub(crate) use panel_lists::PanelListState;
pub(crate) use pull_request_search::PullRequestSearchState;
pub(crate) use repository::RepositoryUiState;
pub(crate) use repository_actions::RepositoryActionsUiState;
pub(crate) use selection::PullRequestSelectionState;
pub(crate) use sync_runtime::SyncRuntimeState;
pub(crate) use tasks::{RepositoryActionsTaskKind, SelectedPullRequestTaskKind, WorkspaceTasks};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pull_request_detail_state_tracks_section_load_transitions() {
        let mut state = PullRequestDetailUiState::new(Vec::new(), Vec::new());

        assert!(state.should_load_details());
        assert!(!state.details_ready());
        state.start_details_load();
        assert!(state.details_loading());
        assert!(!state.details_ready());
        assert!(!state.should_load_details());

        state.apply_details_failure("metadata failed");
        assert_eq!(state.details_error(), Some("metadata failed"));
        assert!(state.details_ready());
        assert!(!state.should_load_details());

        state.reset_for_selection();
        assert!(!state.details_ready());
        assert!(state.should_load_details());

        state.start_details_load();
        state.mark_details_available();
        assert!(state.details_ready());

        state.restore_loaded_sections(PullRequestDetailLoadedState {
            details: true,
            files: false,
            checks: true,
            commits: true,
            workflows: false,
            reviews: true,
        });
        assert!(state.details_loaded());
        assert!(state.commits_finished());
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
}
