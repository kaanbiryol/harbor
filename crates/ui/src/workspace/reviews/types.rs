use harbor_domain::ReactionContent;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReviewCommentSubmission {
    SingleComment,
    StartReview,
    AddToReview,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReviewThreadUiError {
    pub(crate) thread_id: String,
    pub(crate) message: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReviewCommentUiError {
    pub(crate) comment_id: String,
    pub(crate) message: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReviewReactionAction {
    pub(crate) comment_id: String,
    pub(crate) content: ReactionContent,
}
