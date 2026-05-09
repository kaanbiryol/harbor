use harbor_domain::{RepoId, ReviewCommentRange, ReviewSide};
use serde_json::{Map, Value, json};

use crate::{GitHubError, Result};

use super::{PullRequestListFilter, SubmitPullRequestReviewEvent};

pub(super) const REPOSITORY_PULL_REQUESTS_QUERY: &str = r#"
query HarborRepositoryPullRequests($searchQuery: String!, $after: String) {
  search(query: $searchQuery, type: ISSUE, first: 50, after: $after) {
    pageInfo {
      hasNextPage
      endCursor
    }
    nodes {
      __typename
      ... on PullRequest {
        id
        number
        title
        body
        url
        state
        isDraft
        author {
          login
        }
        repository {
          name
          owner {
            login
          }
        }
        headRefName
        baseRefName
        headRefOid
        reviewDecision
        mergeStateStatus
        labels(first: 20) {
          nodes {
            name
            color
          }
        }
      }
    }
  }
}
"#;

pub(super) const REVIEW_THREADS_QUERY: &str = r#"
query HarborPullRequestReviewThreads($owner: String!, $repo: String!, $number: Int!, $after: String) {
  repository(owner: $owner, name: $repo) {
    pullRequest(number: $number) {
      reviewThreads(first: 100, after: $after) {
        pageInfo {
          hasNextPage
          endCursor
        }
        nodes {
          id
          path
          diffSide
          line
          startLine
          startDiffSide
          originalLine
          isResolved
          isOutdated
          comments(first: 100) {
            nodes {
              id
              body
              author {
                login
                avatarUrl(size: 48)
              }
              createdAt
              updatedAt
              path
              line
              originalLine
              viewerDidAuthor
              viewerCanUpdate
              viewerCanDelete
              viewerCanReact
              reactionGroups {
                content
                viewerHasReacted
                users(first: 1) {
                  totalCount
                }
              }
            }
            pageInfo {
              hasNextPage
              endCursor
            }
          }
        }
      }
    }
  }
}
"#;

pub(super) const REVIEW_THREAD_COMMENTS_QUERY: &str = r#"
query HarborPullRequestReviewThreadComments($threadId: ID!, $after: String) {
  node(id: $threadId) {
    ... on PullRequestReviewThread {
      comments(first: 100, after: $after) {
        pageInfo {
          hasNextPage
          endCursor
        }
        nodes {
          id
          body
          author {
            login
            avatarUrl(size: 48)
          }
          createdAt
          updatedAt
          path
          line
          originalLine
          viewerDidAuthor
          viewerCanUpdate
          viewerCanDelete
          viewerCanReact
          reactionGroups {
            content
            viewerHasReacted
            users(first: 1) {
              totalCount
            }
          }
        }
      }
    }
  }
}
"#;

pub(super) const ADD_PULL_REQUEST_REVIEW_MUTATION: &str = r#"
mutation HarborAddPullRequestReview($input: AddPullRequestReviewInput!) {
  addPullRequestReview(input: $input) {
    pullRequestReview {
      id
      state
    }
  }
}
"#;

pub(super) const ADD_PULL_REQUEST_REVIEW_THREAD_MUTATION: &str = r#"
mutation HarborAddPullRequestReviewThread($input: AddPullRequestReviewThreadInput!) {
  addPullRequestReviewThread(input: $input) {
    thread {
      id
    }
  }
}
"#;

pub(super) const ADD_PULL_REQUEST_REVIEW_THREAD_REPLY_MUTATION: &str = r#"
mutation HarborAddPullRequestReviewThreadReply($input: AddPullRequestReviewThreadReplyInput!) {
  addPullRequestReviewThreadReply(input: $input) {
    comment {
      id
    }
  }
}
"#;

pub(super) const RESOLVE_REVIEW_THREAD_MUTATION: &str = r#"
mutation HarborResolveReviewThread($input: ResolveReviewThreadInput!) {
  resolveReviewThread(input: $input) {
    thread {
      id
      isResolved
    }
  }
}
"#;

pub(super) const UNRESOLVE_REVIEW_THREAD_MUTATION: &str = r#"
mutation HarborUnresolveReviewThread($input: UnresolveReviewThreadInput!) {
  unresolveReviewThread(input: $input) {
    thread {
      id
      isResolved
    }
  }
}
"#;

pub(super) const UPDATE_REVIEW_COMMENT_MUTATION: &str = r#"
mutation HarborUpdateReviewComment($input: UpdatePullRequestReviewCommentInput!) {
  updatePullRequestReviewComment(input: $input) {
    pullRequestReviewComment {
      id
      body
    }
  }
}
"#;

pub(super) const DELETE_REVIEW_COMMENT_MUTATION: &str = r#"
mutation HarborDeleteReviewComment($input: DeletePullRequestReviewCommentInput!) {
  deletePullRequestReviewComment(input: $input) {
    pullRequestReviewComment {
      id
    }
  }
}
"#;

pub(super) const ADD_REACTION_MUTATION: &str = r#"
mutation HarborAddReaction($input: AddReactionInput!) {
  addReaction(input: $input) {
    reaction {
      id
    }
  }
}
"#;

pub(super) const REMOVE_REACTION_MUTATION: &str = r#"
mutation HarborRemoveReaction($input: RemoveReactionInput!) {
  removeReaction(input: $input) {
    reaction {
      id
    }
  }
}
"#;

pub(super) const SUBMIT_PULL_REQUEST_REVIEW_MUTATION: &str = r#"
mutation HarborSubmitPullRequestReview($input: SubmitPullRequestReviewInput!) {
  submitPullRequestReview(input: $input) {
    pullRequestReview {
      id
      state
    }
  }
}
"#;

pub(super) fn rest_review_comment_body(
    head_sha: &str,
    range: &ReviewCommentRange,
    body: &str,
) -> Value {
    let mut payload = Map::new();
    payload.insert("body".to_string(), Value::String(body.to_string()));
    payload.insert("commit_id".to_string(), Value::String(head_sha.to_string()));
    payload.insert("path".to_string(), Value::String(range.path.clone()));
    payload.insert("line".to_string(), json!(range.line));
    payload.insert(
        "side".to_string(),
        Value::String(review_side(range.side).to_string()),
    );

    if let Some(start_line) = range.start_line {
        payload.insert("start_line".to_string(), json!(start_line));
    }

    if let Some(start_side) = range.start_side {
        payload.insert(
            "start_side".to_string(),
            Value::String(review_side(start_side).to_string()),
        );
    }

    Value::Object(payload)
}

pub(super) fn graphql_review_thread_input(
    range: &ReviewCommentRange,
    body: &str,
) -> Map<String, Value> {
    let mut input = Map::new();
    input.insert("body".to_string(), Value::String(body.to_string()));
    input.insert("path".to_string(), Value::String(range.path.clone()));
    input.insert("line".to_string(), json!(range.line));
    input.insert(
        "side".to_string(),
        Value::String(review_side(range.side).to_string()),
    );

    if let Some(start_line) = range.start_line {
        input.insert("startLine".to_string(), json!(start_line));
    }

    if let Some(start_side) = range.start_side {
        input.insert(
            "startSide".to_string(),
            Value::String(review_side(start_side).to_string()),
        );
    }

    input
}

pub(super) fn add_review_thread_reply_input(
    review_thread_node_id: &str,
    pull_request_review_node_id: Option<&str>,
    body: &str,
) -> Map<String, Value> {
    let mut input = Map::new();
    input.insert(
        "pullRequestReviewThreadId".to_string(),
        Value::String(review_thread_node_id.to_string()),
    );
    input.insert("body".to_string(), Value::String(body.to_string()));

    if let Some(pull_request_review_node_id) = pull_request_review_node_id {
        input.insert(
            "pullRequestReviewId".to_string(),
            Value::String(pull_request_review_node_id.to_string()),
        );
    }

    input
}

pub(super) fn submit_pull_request_review_input(
    pull_request_review_node_id: &str,
    event: SubmitPullRequestReviewEvent,
    body: Option<&str>,
) -> Map<String, Value> {
    let mut input = Map::new();
    input.insert(
        "pullRequestReviewId".to_string(),
        Value::String(pull_request_review_node_id.to_string()),
    );
    input.insert(
        "event".to_string(),
        Value::String(submit_pull_request_review_event(event).to_string()),
    );
    if let Some(body) = body.filter(|body| !body.trim().is_empty()) {
        input.insert("body".to_string(), Value::String(body.to_string()));
    }

    input
}

fn review_side(side: ReviewSide) -> &'static str {
    match side {
        ReviewSide::Left => "LEFT",
        ReviewSide::Right => "RIGHT",
    }
}

fn submit_pull_request_review_event(event: SubmitPullRequestReviewEvent) -> &'static str {
    match event {
        SubmitPullRequestReviewEvent::Approve => "APPROVE",
        SubmitPullRequestReviewEvent::Comment => "COMMENT",
        SubmitPullRequestReviewEvent::RequestChanges => "REQUEST_CHANGES",
    }
}

pub(super) fn graphql_string_at(value: Value, pointer: &str, label: &str) -> Result<String> {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| GitHubError::Mapping(format!("missing {label}")))
}

pub(super) fn repository_pull_requests_query(
    repository: &RepoId,
    filter: PullRequestListFilter,
) -> String {
    let mode = match filter {
        PullRequestListFilter::Open => "is:open archived:false",
        PullRequestListFilter::Closed => "is:closed archived:false",
        PullRequestListFilter::NeedsReview => "is:open archived:false review-requested:@me",
    };

    format!(
        "repo:{} is:pr {mode} sort:updated-desc",
        repository.full_name()
    )
}

pub(super) const REPOSITORY_PAGE_SIZE: usize = 100;
pub(super) const REPOSITORY_PAGE_SIZE_QUERY: &str = "100";
pub(super) const REVIEW_COMMENT_PAGE_SIZE: usize = 100;
pub(super) const REVIEW_COMMENT_PAGE_SIZE_QUERY: &str = "100";
