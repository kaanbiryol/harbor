use harbor_domain::{
    PullRequestReview, PullRequestReviewState, ReviewComment, ReviewCommentPosition, ReviewSide,
    ReviewThread, ReviewThreadState,
};
use serde::Deserialize;
use serde_json::Value;

use crate::{GitHubError, Result};

#[derive(Debug, Deserialize)]
struct ApiUser {
    login: String,
}

#[derive(Debug, Deserialize)]
struct ApiPullRequestReview {
    id: u64,
    state: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    submitted_at: Option<chrono::DateTime<chrono::Utc>>,
    user: Option<ApiUser>,
}

#[derive(Debug, Deserialize)]
struct GraphQlReviewThreadsResponse {
    data: Option<GraphQlReviewThreadsData>,
}

#[derive(Debug, Deserialize)]
struct GraphQlReviewThreadsData {
    repository: Option<GraphQlReviewThreadsRepository>,
}

#[derive(Debug, Deserialize)]
struct GraphQlReviewThreadsRepository {
    #[serde(rename = "pullRequest")]
    pull_request: Option<GraphQlReviewThreadsPullRequest>,
}

#[derive(Debug, Deserialize)]
struct GraphQlReviewThreadsPullRequest {
    #[serde(rename = "reviewThreads")]
    review_threads: GraphQlReviewThreadConnection,
}

#[derive(Debug, Deserialize)]
struct GraphQlReviewThreadConnection {
    #[serde(default)]
    nodes: Vec<Option<GraphQlReviewThread>>,
    #[serde(default, rename = "pageInfo")]
    page_info: GraphQlPageInfo,
}

#[derive(Debug, Default, Deserialize)]
struct GraphQlPageInfo {
    #[serde(default, rename = "hasNextPage")]
    has_next_page: bool,
    #[serde(default, rename = "endCursor")]
    end_cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(bound(deserialize = "T: Deserialize<'de>"))]
struct GraphQlNodes<T> {
    #[serde(default)]
    nodes: Vec<Option<T>>,
}

impl<T> Default for GraphQlNodes<T> {
    fn default() -> Self {
        Self { nodes: Vec::new() }
    }
}

pub(crate) struct ReviewThreadsPage {
    pub(crate) threads: Vec<ReviewThread>,
    pub(crate) has_next_page: bool,
    pub(crate) end_cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GraphQlReviewThread {
    id: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(default, rename = "line")]
    line: Option<u32>,
    #[serde(default, rename = "originalLine")]
    original_line: Option<u32>,
    #[serde(default, rename = "isResolved")]
    is_resolved: bool,
    #[serde(default, rename = "isOutdated")]
    is_outdated: bool,
    comments: GraphQlNodes<GraphQlReviewComment>,
}

#[derive(Debug, Deserialize)]
struct GraphQlReviewComment {
    id: String,
    body: String,
    author: Option<ApiUser>,
    #[serde(rename = "createdAt")]
    created_at: chrono::DateTime<chrono::Utc>,
    #[serde(default, rename = "updatedAt")]
    updated_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    line: Option<u32>,
    #[serde(default, rename = "originalLine")]
    original_line: Option<u32>,
}

pub fn pull_request_reviews_from_value(value: Value) -> Result<Vec<PullRequestReview>> {
    let reviews: Vec<ApiPullRequestReview> =
        serde_json::from_value(value).map_err(|error| GitHubError::Mapping(error.to_string()))?;

    Ok(reviews
        .into_iter()
        .map(ApiPullRequestReview::into_domain)
        .collect())
}

#[cfg(test)]
pub(crate) fn review_threads_from_graphql_value(value: Value) -> Result<Vec<ReviewThread>> {
    Ok(review_threads_page_from_graphql_value(value)?.threads)
}

pub(crate) fn review_threads_page_from_graphql_value(value: Value) -> Result<ReviewThreadsPage> {
    let response: GraphQlReviewThreadsResponse =
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
    let review_threads = pull_request.review_threads;

    Ok(ReviewThreadsPage {
        threads: review_threads
            .nodes
            .into_iter()
            .flatten()
            .map(GraphQlReviewThread::into_domain)
            .collect(),
        has_next_page: review_threads.page_info.has_next_page,
        end_cursor: review_threads.page_info.end_cursor,
    })
}

impl ApiPullRequestReview {
    fn into_domain(self) -> PullRequestReview {
        PullRequestReview {
            id: self.id.to_string(),
            author: self
                .user
                .map(|user| user.login)
                .unwrap_or_else(|| "ghost".to_string()),
            state: map_pull_request_review_state(&self.state),
            body: self.body.filter(|body| !body.is_empty()),
            submitted_at: self.submitted_at,
        }
    }
}

impl GraphQlReviewThread {
    fn into_domain(self) -> ReviewThread {
        let fallback_path = self.path.unwrap_or_default();
        let fallback_line = self.line;
        let fallback_original_line = self.original_line;
        let comments: Vec<ReviewComment> = self
            .comments
            .nodes
            .into_iter()
            .flatten()
            .map(|comment| {
                comment.into_domain(fallback_path.clone(), fallback_line, fallback_original_line)
            })
            .collect();
        let path = if fallback_path.is_empty() {
            comments
                .iter()
                .find_map(|comment| comment.position.as_ref())
                .map(|position| position.path.clone())
                .unwrap_or_else(|| "unknown".to_string())
        } else {
            fallback_path
        };

        ReviewThread {
            id: self.id,
            path,
            state: map_review_thread_state(self.is_resolved, self.is_outdated),
            comments,
        }
    }
}

impl GraphQlReviewComment {
    fn into_domain(
        self,
        fallback_path: String,
        fallback_line: Option<u32>,
        fallback_original_line: Option<u32>,
    ) -> ReviewComment {
        let path = self.path.unwrap_or(fallback_path);
        let line = self.line.or(fallback_line);
        let original_line = self.original_line.or(fallback_original_line);
        let position = if path.is_empty() && line.is_none() && original_line.is_none() {
            None
        } else {
            Some(ReviewCommentPosition {
                path,
                line,
                original_line,
                side: ReviewSide::Right,
            })
        };

        ReviewComment {
            id: self.id,
            author: self
                .author
                .map(|user| user.login)
                .unwrap_or_else(|| "ghost".to_string()),
            body: self.body,
            created_at: self.created_at,
            updated_at: self.updated_at,
            position,
        }
    }
}

fn map_pull_request_review_state(state: &str) -> PullRequestReviewState {
    match state.to_ascii_lowercase().as_str() {
        "pending" => PullRequestReviewState::Pending,
        "approved" => PullRequestReviewState::Approved,
        "changes_requested" => PullRequestReviewState::ChangesRequested,
        "dismissed" => PullRequestReviewState::Dismissed,
        _ => PullRequestReviewState::Commented,
    }
}

fn map_review_thread_state(is_resolved: bool, is_outdated: bool) -> ReviewThreadState {
    if is_resolved {
        ReviewThreadState::Resolved
    } else if is_outdated {
        ReviewThreadState::Outdated
    } else {
        ReviewThreadState::Unresolved
    }
}
