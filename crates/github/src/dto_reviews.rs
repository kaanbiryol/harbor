use harbor_domain::{
    PullRequestReview, PullRequestReviewState, ReactionContent, ReviewComment,
    ReviewCommentPosition, ReviewCommentRange, ReviewReaction, ReviewSide, ReviewThread,
    ReviewThreadState,
};
use serde::Deserialize;
use serde_json::Value;

use crate::{GitHubError, Result};

#[derive(Debug, Deserialize)]
struct ApiUser {
    login: String,
    #[serde(default, rename = "avatarUrl")]
    avatar_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApiPullRequestReview {
    id: u64,
    #[serde(default)]
    node_id: Option<String>,
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
struct GraphQlConnection<T> {
    #[serde(default)]
    nodes: Vec<Option<T>>,
    #[serde(default, rename = "pageInfo")]
    page_info: GraphQlPageInfo,
}

impl<T> Default for GraphQlConnection<T> {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            page_info: GraphQlPageInfo::default(),
        }
    }
}

pub(crate) struct ReviewThreadsPage {
    pub(crate) threads: Vec<ReviewThread>,
    pub(crate) has_next_page: bool,
    pub(crate) end_cursor: Option<String>,
    pub(crate) comment_cursors: Vec<ReviewThreadCommentCursor>,
}

pub(crate) struct ReviewThreadCommentsPage {
    pub(crate) comments: Vec<ReviewComment>,
    pub(crate) has_next_page: bool,
    pub(crate) end_cursor: Option<String>,
}

#[derive(Clone)]
pub(crate) struct ReviewThreadCommentCursor {
    pub(crate) thread_id: String,
    pub(crate) after: Option<String>,
    context: ReviewThreadCommentContext,
}

#[derive(Clone)]
struct ReviewThreadCommentContext {
    path: String,
    line: Option<u32>,
    original_line: Option<u32>,
    side: ReviewSide,
}

#[derive(Debug, Deserialize)]
struct GraphQlReviewThread {
    id: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(default, rename = "line")]
    line: Option<u32>,
    #[serde(default, rename = "diffSide")]
    diff_side: Option<String>,
    #[serde(default, rename = "startLine")]
    start_line: Option<u32>,
    #[serde(default, rename = "startDiffSide")]
    start_diff_side: Option<String>,
    #[serde(default, rename = "originalLine")]
    original_line: Option<u32>,
    #[serde(default, rename = "isResolved")]
    is_resolved: bool,
    #[serde(default, rename = "isOutdated")]
    is_outdated: bool,
    comments: GraphQlConnection<GraphQlReviewComment>,
}

#[derive(Debug, Deserialize)]
struct GraphQlReviewThreadCommentsResponse {
    data: Option<GraphQlReviewThreadCommentsData>,
}

#[derive(Debug, Deserialize)]
struct GraphQlReviewThreadCommentsData {
    node: Option<GraphQlReviewThreadCommentsNode>,
}

#[derive(Debug, Deserialize)]
struct GraphQlReviewThreadCommentsNode {
    comments: GraphQlConnection<GraphQlReviewComment>,
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
    #[serde(default, rename = "viewerDidAuthor")]
    viewer_did_author: bool,
    #[serde(default, rename = "viewerCanUpdate")]
    viewer_can_update: bool,
    #[serde(default, rename = "viewerCanDelete")]
    viewer_can_delete: bool,
    #[serde(default, rename = "viewerCanReact")]
    viewer_can_react: bool,
    #[serde(default, rename = "reactionGroups")]
    reaction_groups: Vec<GraphQlReactionGroup>,
}

#[derive(Debug, Deserialize)]
struct GraphQlReactionGroup {
    content: String,
    #[serde(default, rename = "viewerHasReacted")]
    viewer_has_reacted: bool,
    #[serde(default)]
    users: GraphQlReactionUsers,
}

#[derive(Debug, Default, Deserialize)]
struct GraphQlReactionUsers {
    #[serde(default, rename = "totalCount")]
    total_count: usize,
}

pub fn current_user_login_from_value(value: Value) -> Result<String> {
    let user: ApiUser =
        serde_json::from_value(value).map_err(|error| GitHubError::Mapping(error.to_string()))?;

    Ok(user.login)
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

    let mut comment_cursors = Vec::new();
    let threads = review_threads
        .nodes
        .into_iter()
        .flatten()
        .map(|thread| {
            let cursor = thread.next_comment_cursor();
            let thread = thread.into_domain();
            if let Some(cursor) = cursor {
                comment_cursors.push(cursor);
            }
            thread
        })
        .collect();

    Ok(ReviewThreadsPage {
        threads,
        has_next_page: review_threads.page_info.has_next_page,
        end_cursor: review_threads.page_info.end_cursor,
        comment_cursors,
    })
}

pub(crate) fn review_thread_comments_page_from_graphql_value(
    value: Value,
    cursor: &ReviewThreadCommentCursor,
) -> Result<ReviewThreadCommentsPage> {
    let response: GraphQlReviewThreadCommentsResponse =
        serde_json::from_value(value).map_err(|error| GitHubError::Mapping(error.to_string()))?;
    let data = response
        .data
        .ok_or_else(|| GitHubError::Mapping("missing GraphQL response data".to_string()))?;
    let node = data
        .node
        .ok_or_else(|| GitHubError::Mapping("missing GraphQL review thread node".to_string()))?;
    let comments = node.comments;

    Ok(ReviewThreadCommentsPage {
        comments: comments
            .nodes
            .into_iter()
            .flatten()
            .map(|comment| {
                comment.into_domain(
                    cursor.context.path.clone(),
                    cursor.context.line,
                    cursor.context.original_line,
                    cursor.context.side,
                )
            })
            .collect(),
        has_next_page: comments.page_info.has_next_page,
        end_cursor: comments.page_info.end_cursor,
    })
}

impl ApiPullRequestReview {
    fn into_domain(self) -> PullRequestReview {
        PullRequestReview {
            id: self.id.to_string(),
            node_id: self.node_id,
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
    fn next_comment_cursor(&self) -> Option<ReviewThreadCommentCursor> {
        if !self.comments.page_info.has_next_page {
            return None;
        }

        let fallback_side =
            review_thread_side(self.diff_side.as_deref(), self.line, self.original_line);

        Some(ReviewThreadCommentCursor {
            thread_id: self.id.clone(),
            after: self.comments.page_info.end_cursor.clone(),
            context: ReviewThreadCommentContext {
                path: self.path.clone().unwrap_or_default(),
                line: self.line,
                original_line: self.original_line,
                side: fallback_side,
            },
        })
    }

    fn into_domain(self) -> ReviewThread {
        let fallback_side =
            review_thread_side(self.diff_side.as_deref(), self.line, self.original_line);
        let fallback_start_side = self.start_diff_side.as_deref().map(map_review_side);
        let fallback_path = self.path.unwrap_or_default();
        let fallback_line = self.line;
        let fallback_original_line = self.original_line;
        let range_line =
            review_thread_range_line(fallback_side, fallback_line, fallback_original_line);
        let range = range_line.map(|line| ReviewCommentRange {
            path: fallback_path.clone(),
            line,
            side: fallback_side,
            start_line: self.start_line,
            start_side: fallback_start_side,
        });
        let comments: Vec<ReviewComment> = self
            .comments
            .nodes
            .into_iter()
            .flatten()
            .map(|comment| {
                comment.into_domain(
                    fallback_path.clone(),
                    fallback_line,
                    fallback_original_line,
                    fallback_side,
                )
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
            range,
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
        fallback_side: ReviewSide,
    ) -> ReviewComment {
        let path = self.path.unwrap_or(fallback_path);
        let line = self.line.or(fallback_line);
        let original_line = self.original_line.or(fallback_original_line);
        let side = fallback_side;
        let position = if path.is_empty() && line.is_none() && original_line.is_none() {
            None
        } else {
            Some(ReviewCommentPosition {
                path,
                line,
                original_line,
                side,
            })
        };

        ReviewComment {
            id: self.id,
            author: self
                .author
                .as_ref()
                .map(|user| user.login.clone())
                .unwrap_or_else(|| "ghost".to_string()),
            author_avatar_url: self.author.and_then(|user| user.avatar_url),
            body: self.body,
            created_at: self.created_at,
            updated_at: self.updated_at,
            position,
            viewer_did_author: self.viewer_did_author,
            viewer_can_update: self.viewer_can_update,
            viewer_can_delete: self.viewer_can_delete,
            viewer_can_react: self.viewer_can_react,
            reactions: self
                .reaction_groups
                .into_iter()
                .filter_map(GraphQlReactionGroup::into_domain)
                .collect(),
        }
    }
}

impl GraphQlReactionGroup {
    fn into_domain(self) -> Option<ReviewReaction> {
        Some(ReviewReaction {
            content: map_reaction_content(&self.content)?,
            count: self
                .users
                .total_count
                .max(usize::from(self.viewer_has_reacted)),
            viewer_has_reacted: self.viewer_has_reacted,
        })
    }
}

fn map_review_side(side: &str) -> ReviewSide {
    if side.eq_ignore_ascii_case("LEFT") {
        ReviewSide::Left
    } else {
        ReviewSide::Right
    }
}

fn review_thread_side(
    diff_side: Option<&str>,
    line: Option<u32>,
    original_line: Option<u32>,
) -> ReviewSide {
    diff_side.map(map_review_side).unwrap_or_else(|| {
        if line.is_none() && original_line.is_some() {
            ReviewSide::Left
        } else {
            ReviewSide::Right
        }
    })
}

fn review_thread_range_line(
    side: ReviewSide,
    line: Option<u32>,
    original_line: Option<u32>,
) -> Option<u32> {
    match side {
        ReviewSide::Left => original_line.or(line),
        ReviewSide::Right => line.or(original_line),
    }
}

fn map_reaction_content(content: &str) -> Option<ReactionContent> {
    match content.to_ascii_uppercase().as_str() {
        "THUMBS_UP" => Some(ReactionContent::ThumbsUp),
        "THUMBS_DOWN" => Some(ReactionContent::ThumbsDown),
        "LAUGH" => Some(ReactionContent::Laugh),
        "CONFUSED" => Some(ReactionContent::Confused),
        "HEART" => Some(ReactionContent::Heart),
        "HOORAY" => Some(ReactionContent::Hooray),
        "ROCKET" => Some(ReactionContent::Rocket),
        "EYES" => Some(ReactionContent::Eyes),
        _ => None,
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
