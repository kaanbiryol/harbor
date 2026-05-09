#[path = "panels/checks.rs"]
mod checks;
#[path = "panels/diff_view.rs"]
mod diff_view;
#[path = "panels/logs.rs"]
mod logs;
#[path = "panels/pull_request.rs"]
mod pull_request;
#[path = "panels/review.rs"]
mod review;
#[path = "panels/review_thread_chrome.rs"]
pub(crate) mod review_thread_chrome;
#[path = "panels/workflows.rs"]
mod workflows;

pub(crate) use checks::*;
pub(crate) use diff_view::*;
pub(crate) use logs::*;
pub(crate) use pull_request::*;
pub(crate) use review::*;
pub(crate) use workflows::*;
