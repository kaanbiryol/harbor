use harbor_domain::{PullRequestReview, ReactionContent, ReviewCommentRange, ReviewThread};
use serde_json::{Value, json};

use crate::{GitHubError, GitHubTransport, Result, dto};

use super::{
    GitHubClient, SubmitPullRequestReviewEvent,
    requests::{
        ADD_PULL_REQUEST_REVIEW_MUTATION, ADD_PULL_REQUEST_REVIEW_THREAD_MUTATION,
        ADD_PULL_REQUEST_REVIEW_THREAD_REPLY_MUTATION, ADD_REACTION_MUTATION,
        DELETE_REVIEW_COMMENT_MUTATION, REMOVE_REACTION_MUTATION, RESOLVE_REVIEW_THREAD_MUTATION,
        REVIEW_COMMENT_PAGE_SIZE, REVIEW_COMMENT_PAGE_SIZE_QUERY, REVIEW_THREADS_QUERY,
        SUBMIT_PULL_REQUEST_REVIEW_MUTATION, UNRESOLVE_REVIEW_THREAD_MUTATION,
        UPDATE_REVIEW_COMMENT_MUTATION, add_review_thread_reply_input, graphql_review_thread_input,
        graphql_string_at, rest_review_comment_body, submit_pull_request_review_input,
    },
};

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

    pub async fn list_review_threads(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> Result<Vec<ReviewThread>> {
        let mut threads = Vec::new();
        let mut after = None;

        loop {
            let response = self
                .transport
                .graphql(
                    REVIEW_THREADS_QUERY,
                    json!({
                        "owner": owner,
                        "repo": repo,
                        "number": number,
                        "after": after,
                    }),
                )
                .await?;
            let page = dto::review_threads_page_from_graphql_value(response)?;
            threads.extend(page.threads);

            if !page.has_next_page {
                break;
            }

            after = Some(page.end_cursor.ok_or_else(|| {
                GitHubError::Mapping("review threads page was missing an end cursor".to_string())
            })?);
        }

        Ok(threads)
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
