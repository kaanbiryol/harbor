use async_trait::async_trait;
use harbor_domain::{MergeMethod, ReactionContent, ReviewCommentRange};
use harbor_github::{Result, SubmitPullRequestReviewEvent};

use super::super::{GitHubPullRequestActionApi, GitHubReviewMutationApi, GitHubWorkflowActionApi};
use super::{FakeGitHubApi, queues::pop_result};

#[async_trait]
impl GitHubWorkflowActionApi for FakeGitHubApi {
    async fn dispatch_workflow(
        &self,
        _owner: &str,
        _repo: &str,
        _workflow_id: u64,
        _git_ref: &str,
    ) -> Result<()> {
        self.record_call("dispatch_workflow");
        pop_result(&self.dispatch_workflow_results, "dispatch_workflow")
    }

    async fn rerun_failed_jobs(&self, _owner: &str, _repo: &str, _run_id: u64) -> Result<()> {
        self.record_call("rerun_failed_jobs");
        pop_result(&self.rerun_failed_jobs_results, "rerun_failed_jobs")
    }
}

#[async_trait]
impl GitHubPullRequestActionApi for FakeGitHubApi {
    async fn update_pull_request_body(
        &self,
        _pull_request_node_id: &str,
        _body: &str,
    ) -> Result<()> {
        self.record_call("update_pull_request_body");
        pop_result(
            &self.update_pull_request_body_results,
            "update_pull_request_body",
        )
    }

    async fn request_pull_request_reviewer(
        &self,
        _owner: &str,
        _repo: &str,
        _number: u64,
        _reviewer: &str,
    ) -> Result<()> {
        self.record_call("request_pull_request_reviewer");
        pop_result(
            &self.request_reviewer_results,
            "request_pull_request_reviewer",
        )
    }

    async fn add_pull_request_assignee(
        &self,
        _owner: &str,
        _repo: &str,
        _number: u64,
        _assignee: &str,
    ) -> Result<()> {
        self.record_call("add_pull_request_assignee");
        pop_result(&self.add_assignee_results, "add_pull_request_assignee")
    }

    async fn add_pull_request_label(
        &self,
        _owner: &str,
        _repo: &str,
        _number: u64,
        _label: &str,
    ) -> Result<()> {
        self.record_call("add_pull_request_label");
        pop_result(&self.add_label_results, "add_pull_request_label")
    }

    async fn create_pull_request_comment(
        &self,
        _owner: &str,
        _repo: &str,
        _number: u64,
        _body: &str,
    ) -> Result<()> {
        self.record_call("create_pull_request_comment");
        pop_result(
            &self.create_pull_request_comment_results,
            "create_pull_request_comment",
        )
    }

    async fn approve_pull_request(
        &self,
        _owner: &str,
        _repo: &str,
        _number: u64,
        _body: Option<&str>,
    ) -> Result<()> {
        self.record_call("approve_pull_request");
        pop_result(&self.approve_results, "approve_pull_request")
    }

    async fn request_pull_request_changes(
        &self,
        _owner: &str,
        _repo: &str,
        _number: u64,
        _body: &str,
    ) -> Result<()> {
        self.record_call("request_pull_request_changes");
        pop_result(
            &self.request_changes_results,
            "request_pull_request_changes",
        )
    }

    async fn merge_pull_request(
        &self,
        _owner: &str,
        _repo: &str,
        _number: u64,
        _head_sha: &str,
        _method: MergeMethod,
    ) -> Result<()> {
        self.record_call("merge_pull_request");
        pop_result(&self.merge_results, "merge_pull_request")
    }
}

#[async_trait]
impl GitHubReviewMutationApi for FakeGitHubApi {
    async fn submit_pull_request_review(
        &self,
        _pull_request_review_node_id: &str,
        _event: SubmitPullRequestReviewEvent,
        _body: Option<&str>,
    ) -> Result<()> {
        self.record_call("submit_pull_request_review");
        pop_result(&self.submit_review_results, "submit_pull_request_review")
    }

    async fn create_pull_request_review_comment(
        &self,
        _owner: &str,
        _repo: &str,
        _number: u64,
        _commit_id: &str,
        _range: &ReviewCommentRange,
        _body: &str,
    ) -> Result<()> {
        self.record_call("create_pull_request_review_comment");
        pop_result(
            &self.create_comment_results,
            "create_pull_request_review_comment",
        )
    }

    async fn start_pull_request_review(
        &self,
        _pull_request_node_id: &str,
        _commit_id: &str,
        _range: &ReviewCommentRange,
        _body: &str,
    ) -> Result<String> {
        self.record_call("start_pull_request_review");
        pop_result(&self.start_review_results, "start_pull_request_review")
    }

    async fn add_pending_review_thread(
        &self,
        _pull_request_review_node_id: &str,
        _range: &ReviewCommentRange,
        _body: &str,
    ) -> Result<()> {
        self.record_call("add_pending_review_thread");
        pop_result(&self.pending_thread_results, "add_pending_review_thread")
    }

    async fn add_review_thread_reply(
        &self,
        _thread_id: &str,
        _pull_request_review_node_id: Option<&str>,
        _body: &str,
    ) -> Result<()> {
        self.record_call("add_review_thread_reply");
        pop_result(&self.reply_results, "add_review_thread_reply")
    }

    async fn resolve_review_thread(&self, _thread_id: &str) -> Result<()> {
        self.record_call("resolve_review_thread");
        pop_result(&self.resolve_thread_results, "resolve_review_thread")
    }

    async fn unresolve_review_thread(&self, _thread_id: &str) -> Result<()> {
        self.record_call("unresolve_review_thread");
        pop_result(&self.unresolve_thread_results, "unresolve_review_thread")
    }

    async fn update_review_comment(&self, _comment_id: &str, _body: &str) -> Result<()> {
        self.record_call("update_review_comment");
        pop_result(&self.update_comment_results, "update_review_comment")
    }

    async fn delete_review_comment(&self, _comment_id: &str) -> Result<()> {
        self.record_call("delete_review_comment");
        pop_result(&self.delete_comment_results, "delete_review_comment")
    }

    async fn add_review_comment_reaction(
        &self,
        _comment_id: &str,
        _content: ReactionContent,
    ) -> Result<()> {
        self.record_call("add_review_comment_reaction");
        pop_result(&self.add_reaction_results, "add_review_comment_reaction")
    }

    async fn remove_review_comment_reaction(
        &self,
        _comment_id: &str,
        _content: ReactionContent,
    ) -> Result<()> {
        self.record_call("remove_review_comment_reaction");
        pop_result(
            &self.remove_reaction_results,
            "remove_review_comment_reaction",
        )
    }
}
