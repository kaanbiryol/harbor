use harbor_domain::FileViewedState;
use serde::Deserialize;
use serde_json::Value;

use crate::{GitHubError, Result};

use super::GraphQlPageInfo;

#[derive(Debug, Deserialize)]
struct GraphQlPullRequestFileViewedStatesResponse {
    data: Option<GraphQlPullRequestFileViewedStatesData>,
}

#[derive(Debug, Deserialize)]
struct GraphQlPullRequestFileViewedStatesData {
    repository: Option<GraphQlPullRequestFileViewedStatesRepository>,
}

#[derive(Debug, Deserialize)]
struct GraphQlPullRequestFileViewedStatesRepository {
    #[serde(rename = "pullRequest")]
    pull_request: Option<GraphQlPullRequestFileViewedStatesPullRequest>,
}

#[derive(Debug, Deserialize)]
struct GraphQlPullRequestFileViewedStatesPullRequest {
    files: GraphQlPullRequestFileViewedStatesConnection,
}

#[derive(Debug, Deserialize)]
struct GraphQlPullRequestFileViewedStatesConnection {
    #[serde(default)]
    nodes: Vec<Option<GraphQlPullRequestFileViewedStateNode>>,
    #[serde(default, rename = "pageInfo")]
    page_info: GraphQlPageInfo,
}

#[derive(Debug, Deserialize)]
struct GraphQlPullRequestFileViewedStateNode {
    path: String,
    #[serde(rename = "viewerViewedState")]
    viewer_viewed_state: String,
}

pub(crate) struct PullRequestFileViewedStatesPage {
    pub(crate) file_states: Vec<PullRequestFileViewedState>,
    pub(crate) has_next_page: bool,
    pub(crate) end_cursor: Option<String>,
}

pub(crate) struct PullRequestFileViewedState {
    pub(crate) path: String,
    pub(crate) viewed_state: FileViewedState,
}

pub(crate) fn pull_request_file_viewed_states_page_from_graphql_value(
    value: Value,
) -> Result<PullRequestFileViewedStatesPage> {
    let response: GraphQlPullRequestFileViewedStatesResponse =
        serde_json::from_value(value).map_err(|error| GitHubError::Mapping(error.to_string()))?;
    let data = response
        .data
        .ok_or_else(|| GitHubError::Mapping("missing GraphQL response data".to_string()))?;
    let repository = data
        .repository
        .ok_or_else(|| GitHubError::Mapping("missing GraphQL repository".to_string()))?;
    let pull_request = repository
        .pull_request
        .ok_or_else(|| GitHubError::Mapping("missing GraphQL pull request".to_string()))?;

    Ok(PullRequestFileViewedStatesPage {
        file_states: pull_request
            .files
            .nodes
            .into_iter()
            .flatten()
            .map(GraphQlPullRequestFileViewedStateNode::into_domain)
            .collect(),
        has_next_page: pull_request.files.page_info.has_next_page,
        end_cursor: pull_request.files.page_info.end_cursor,
    })
}

impl GraphQlPullRequestFileViewedStateNode {
    fn into_domain(self) -> PullRequestFileViewedState {
        PullRequestFileViewedState {
            path: self.path,
            viewed_state: map_file_viewed_state(&self.viewer_viewed_state),
        }
    }
}

fn map_file_viewed_state(state: &str) -> FileViewedState {
    match state {
        "VIEWED" => FileViewedState::Viewed,
        "DISMISSED" => FileViewedState::ChangedSinceViewed,
        _ => FileViewedState::Unviewed,
    }
}
