use std::collections::VecDeque;

use harbor_domain::{
    PullRequestComment, PullRequestReview, ReactionContent, ReviewCommentRange, ReviewThread,
};
use serde_json::{Value, json};

use crate::{GitHubError, GitHubTransport, Result, dto};

use super::{
    GitHubClient, SubmitPullRequestReviewEvent,
    requests::{
        ADD_PULL_REQUEST_REVIEW_MUTATION, ADD_PULL_REQUEST_REVIEW_THREAD_MUTATION,
        ADD_PULL_REQUEST_REVIEW_THREAD_REPLY_MUTATION, ADD_REACTION_MUTATION,
        DELETE_REVIEW_COMMENT_MUTATION, REMOVE_REACTION_MUTATION, RESOLVE_REVIEW_THREAD_MUTATION,
        REVIEW_COMMENT_PAGE_SIZE, REVIEW_COMMENT_PAGE_SIZE_QUERY, REVIEW_THREAD_COMMENTS_QUERY,
        REVIEW_THREADS_QUERY, SUBMIT_PULL_REQUEST_REVIEW_MUTATION,
        UNRESOLVE_REVIEW_THREAD_MUTATION, UPDATE_REVIEW_COMMENT_MUTATION,
        add_review_thread_reply_input, graphql_review_thread_input, graphql_string_at,
        rest_review_comment_body, submit_pull_request_review_input,
    },
};

const REVIEW_THREAD_COMMENT_PAGE_BATCH_SIZE: usize = 8;
const REVIEW_THREAD_COMMENT_EXTRA_PAGE_LIMIT: usize = 8;
const REVIEW_THREAD_PAGE_LIMIT: usize = 20;
const REVIEW_THREAD_PAGE_SIZE: usize = 5;
const REVIEW_THREAD_INITIAL_COMMENT_PAGE_SIZE: usize = 1;
const REVIEW_THREAD_COMMENT_PAGE_SIZE: usize = 50;
const PULL_REQUEST_COMMENT_PAGE_SIZE: usize = 100;

impl<T> GitHubClient<T>
where
    T: GitHubTransport,
{
    pub async fn list_pull_request_reviews(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> Result<Vec<PullRequestReview>> {
        let path = format!("/repos/{owner}/{repo}/pulls/{number}/reviews");
        let response = self
            .transport
            .rest_get(&path, &[("per_page", "100")])
            .await?;

        dto::pull_request_reviews_from_value(response)
    }

    pub async fn pull_request_review_comment_count(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        review_id: &str,
    ) -> Result<usize> {
        let path = format!("/repos/{owner}/{repo}/pulls/{number}/reviews/{review_id}/comments");
        let mut count = 0;
        let mut page = 1;

        loop {
            let page_string = page.to_string();
            let response = self
                .transport
                .rest_get(
                    &path,
                    &[
                        ("per_page", REVIEW_COMMENT_PAGE_SIZE_QUERY),
                        ("page", page_string.as_str()),
                    ],
                )
                .await?;
            let comments = response.as_array().ok_or_else(|| {
                GitHubError::Mapping(
                    "pull request review comments response was not an array".to_string(),
                )
            })?;
            let page_count = comments.len();
            count += page_count;

            if page_count < REVIEW_COMMENT_PAGE_SIZE {
                break;
            }

            page += 1;
        }

        Ok(count)
    }

    pub async fn list_pull_request_comments(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> Result<Vec<PullRequestComment>> {
        let path = format!("/repos/{owner}/{repo}/issues/{number}/comments");
        let mut comments = Vec::new();
        let mut page = 1;

        loop {
            let page_string = page.to_string();
            let response = self
                .transport
                .rest_get(
                    &path,
                    &[("per_page", "100"), ("page", page_string.as_str())],
                )
                .await?;
            let page_comments = dto::pull_request_comments_from_value(response)?;
            let page_count = page_comments.len();
            comments.extend(page_comments);

            if page_count < PULL_REQUEST_COMMENT_PAGE_SIZE {
                break;
            }

            page += 1;
        }

        Ok(comments)
    }

    pub async fn list_review_threads(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> Result<Vec<ReviewThread>> {
        let mut threads = Vec::new();
        let mut after = None;
        let mut extra_comment_pages_loaded = 0;
        let mut thread_pages_loaded = 0;

        loop {
            if thread_pages_loaded >= REVIEW_THREAD_PAGE_LIMIT {
                return Err(GitHubError::RequestBudget(format!(
                    "stopped loading review threads after {REVIEW_THREAD_PAGE_LIMIT} pages"
                )));
            }
            thread_pages_loaded += 1;

            let response = self
                .transport
                .graphql(
                    REVIEW_THREADS_QUERY,
                    json!({
                        "owner": owner,
                        "repo": repo,
                        "number": number,
                        "after": after,
                        "threadPageSize": REVIEW_THREAD_PAGE_SIZE,
                        "commentPageSize": REVIEW_THREAD_INITIAL_COMMENT_PAGE_SIZE,
                    }),
                )
                .await?;
            let page = dto::review_threads_page_from_graphql_value(response)?;
            threads.extend(page.threads);
            self.append_review_thread_comment_pages(
                &mut threads,
                page.comment_cursors,
                &mut extra_comment_pages_loaded,
            )
            .await?;

            if !page.has_next_page {
                break;
            }

            after = Some(page.end_cursor.ok_or_else(|| {
                GitHubError::Mapping("review threads page was missing an end cursor".to_string())
            })?);
        }

        Ok(threads)
    }

    async fn append_review_thread_comment_pages(
        &self,
        threads: &mut [ReviewThread],
        cursors: Vec<dto::ReviewThreadCommentCursor>,
        loaded_pages: &mut usize,
    ) -> Result<()> {
        let mut pending = cursors.into_iter().collect::<VecDeque<_>>();

        while !pending.is_empty() {
            let batch_size = pending.len().min(REVIEW_THREAD_COMMENT_PAGE_BATCH_SIZE);
            for _ in 0..batch_size {
                if *loaded_pages >= REVIEW_THREAD_COMMENT_EXTRA_PAGE_LIMIT {
                    tracing::warn!(
                        pending_comment_pages = pending.len(),
                        loaded_comment_pages = *loaded_pages,
                        "stopped paginating review thread comments; rendering partial review threads"
                    );
                    return Ok(());
                }

                *loaded_pages += 1;
                let mut cursor = pending.pop_front().ok_or_else(|| {
                    GitHubError::Mapping("review thread comment cursor queue was empty".into())
                })?;
                let after = cursor.after.clone().ok_or_else(|| {
                    GitHubError::Mapping(
                        "review thread comments page was missing an end cursor".into(),
                    )
                })?;
                let response = self
                    .transport
                    .graphql(
                        REVIEW_THREAD_COMMENTS_QUERY,
                        json!({
                            "threadId": cursor.thread_id.clone(),
                            "after": after,
                            "commentPageSize": REVIEW_THREAD_COMMENT_PAGE_SIZE,
                        }),
                    )
                    .await?;
                let page = dto::review_thread_comments_page_from_graphql_value(response, &cursor)?;
                let thread = threads
                    .iter_mut()
                    .find(|thread| thread.id == cursor.thread_id)
                    .ok_or_else(|| {
                        GitHubError::Mapping(format!(
                            "review thread {} was missing from its GraphQL page",
                            cursor.thread_id
                        ))
                    })?;
                thread.comments.extend(page.comments);

                if page.has_next_page {
                    cursor.after = Some(page.end_cursor.ok_or_else(|| {
                        GitHubError::Mapping(
                            "review thread comments page was missing an end cursor".into(),
                        )
                    })?);
                    pending.push_back(cursor);
                }
            }
        }

        Ok(())
    }

    pub async fn create_pull_request_review_comment(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        head_sha: &str,
        range: &ReviewCommentRange,
        body: &str,
    ) -> Result<()> {
        let path = format!("/repos/{owner}/{repo}/pulls/{number}/comments");
        self.transport
            .rest_post(&path, rest_review_comment_body(head_sha, range, body))
            .await?;

        Ok(())
    }

    pub async fn start_pull_request_review(
        &self,
        pull_request_node_id: &str,
        head_sha: &str,
        range: &ReviewCommentRange,
        body: &str,
    ) -> Result<String> {
        let response = self
            .transport
            .graphql(
                ADD_PULL_REQUEST_REVIEW_MUTATION,
                json!({
                    "input": {
                        "pullRequestId": pull_request_node_id,
                        "commitOID": head_sha,
                        "threads": [graphql_review_thread_input(range, body)]
                    }
                }),
            )
            .await?;

        graphql_string_at(
            response,
            "/data/addPullRequestReview/pullRequestReview/id",
            "created pull request review id",
        )
    }

    pub async fn add_pending_review_thread(
        &self,
        pull_request_review_node_id: &str,
        range: &ReviewCommentRange,
        body: &str,
    ) -> Result<()> {
        let mut input = graphql_review_thread_input(range, body);
        input.insert(
            "pullRequestReviewId".to_string(),
            Value::String(pull_request_review_node_id.to_string()),
        );

        self.transport
            .graphql(
                ADD_PULL_REQUEST_REVIEW_THREAD_MUTATION,
                json!({ "input": input }),
            )
            .await?;

        Ok(())
    }

    pub async fn add_review_thread_reply(
        &self,
        review_thread_node_id: &str,
        pull_request_review_node_id: Option<&str>,
        body: &str,
    ) -> Result<()> {
        let input =
            add_review_thread_reply_input(review_thread_node_id, pull_request_review_node_id, body);

        self.transport
            .graphql(
                ADD_PULL_REQUEST_REVIEW_THREAD_REPLY_MUTATION,
                json!({ "input": input }),
            )
            .await?;

        Ok(())
    }

    pub async fn resolve_review_thread(&self, review_thread_node_id: &str) -> Result<()> {
        self.transport
            .graphql(
                RESOLVE_REVIEW_THREAD_MUTATION,
                json!({
                    "input": {
                        "threadId": review_thread_node_id,
                    }
                }),
            )
            .await?;

        Ok(())
    }

    pub async fn unresolve_review_thread(&self, review_thread_node_id: &str) -> Result<()> {
        self.transport
            .graphql(
                UNRESOLVE_REVIEW_THREAD_MUTATION,
                json!({
                    "input": {
                        "threadId": review_thread_node_id,
                    }
                }),
            )
            .await?;

        Ok(())
    }

    pub async fn update_review_comment(
        &self,
        review_comment_node_id: &str,
        body: &str,
    ) -> Result<()> {
        self.transport
            .graphql(
                UPDATE_REVIEW_COMMENT_MUTATION,
                json!({
                    "input": {
                        "pullRequestReviewCommentId": review_comment_node_id,
                        "body": body,
                    }
                }),
            )
            .await?;

        Ok(())
    }

    pub async fn delete_review_comment(&self, review_comment_node_id: &str) -> Result<()> {
        self.transport
            .graphql(
                DELETE_REVIEW_COMMENT_MUTATION,
                json!({
                    "input": {
                        "id": review_comment_node_id,
                    }
                }),
            )
            .await?;

        Ok(())
    }

    pub async fn add_review_comment_reaction(
        &self,
        review_comment_node_id: &str,
        content: ReactionContent,
    ) -> Result<()> {
        self.transport
            .graphql(
                ADD_REACTION_MUTATION,
                json!({
                    "input": {
                        "subjectId": review_comment_node_id,
                        "content": content.graphql_name(),
                    }
                }),
            )
            .await?;

        Ok(())
    }

    pub async fn remove_review_comment_reaction(
        &self,
        review_comment_node_id: &str,
        content: ReactionContent,
    ) -> Result<()> {
        self.transport
            .graphql(
                REMOVE_REACTION_MUTATION,
                json!({
                    "input": {
                        "subjectId": review_comment_node_id,
                        "content": content.graphql_name(),
                    }
                }),
            )
            .await?;

        Ok(())
    }

    pub async fn submit_pull_request_review(
        &self,
        pull_request_review_node_id: &str,
        event: SubmitPullRequestReviewEvent,
        body: Option<&str>,
    ) -> Result<()> {
        let input = submit_pull_request_review_input(pull_request_review_node_id, event, body);

        self.transport
            .graphql(
                SUBMIT_PULL_REQUEST_REVIEW_MUTATION,
                json!({ "input": input }),
            )
            .await?;

        Ok(())
    }

    pub async fn approve_pull_request(&self, owner: &str, repo: &str, number: u64) -> Result<()> {
        let path = format!("/repos/{owner}/{repo}/pulls/{number}/reviews");
        self.transport
            .rest_post(&path, json!({ "event": "APPROVE" }))
            .await?;

        Ok(())
    }

    pub async fn request_pull_request_changes(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        body: &str,
    ) -> Result<()> {
        let path = format!("/repos/{owner}/{repo}/pulls/{number}/reviews");
        self.transport
            .rest_post(&path, json!({ "event": "REQUEST_CHANGES", "body": body }))
            .await?;

        Ok(())
    }
}
