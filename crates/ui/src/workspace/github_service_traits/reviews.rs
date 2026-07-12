use async_trait::async_trait;
use harbor_domain::{
    PullRequestComment, PullRequestReview, ReactionContent, ReviewCommentRange, ReviewThread,
    SubmitPullRequestReviewEvent,
};
use harbor_github::Result;

#[async_trait]
pub trait GitHubReviewApi: Send + Sync {
    async fn list_pull_request_reviews(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> Result<Vec<PullRequestReview>>;
    async fn list_pull_request_comments(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> Result<Vec<PullRequestComment>>;
    async fn pull_request_review_comment_count(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        review_id: &str,
    ) -> Result<usize>;
    async fn list_review_threads(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> Result<Vec<ReviewThread>>;
}

#[async_trait]
pub trait GitHubReviewMutationApi: Send + Sync {
    async fn submit_pull_request_review(
        &self,
        pull_request_review_node_id: &str,
        event: SubmitPullRequestReviewEvent,
        body: Option<&str>,
    ) -> Result<()>;
    async fn create_pull_request_review_comment(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        commit_id: &str,
        range: &ReviewCommentRange,
        body: &str,
    ) -> Result<()>;
    async fn start_pull_request_review(
        &self,
        pull_request_node_id: &str,
        commit_id: &str,
        range: &ReviewCommentRange,
        body: &str,
    ) -> Result<String>;
    async fn add_pending_review_thread(
        &self,
        pull_request_review_node_id: &str,
        range: &ReviewCommentRange,
        body: &str,
    ) -> Result<()>;
    async fn add_review_thread_reply(
        &self,
        thread_id: &str,
        pull_request_review_node_id: Option<&str>,
        body: &str,
    ) -> Result<()>;
    async fn resolve_review_thread(&self, thread_id: &str) -> Result<()>;
    async fn unresolve_review_thread(&self, thread_id: &str) -> Result<()>;
    async fn update_review_comment(&self, comment_id: &str, body: &str) -> Result<()>;
    async fn delete_review_comment(&self, comment_id: &str) -> Result<()>;
    async fn add_review_comment_reaction(
        &self,
        comment_id: &str,
        content: ReactionContent,
    ) -> Result<()>;
    async fn remove_review_comment_reaction(
        &self,
        comment_id: &str,
        content: ReactionContent,
    ) -> Result<()>;
}
