use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

use futures::channel::oneshot;
use harbor_github::{GitHubError, Result};

pub(super) type FakeQueue<T> = Arc<Mutex<VecDeque<FakeResponse<T>>>>;

pub(super) enum FakeResponse<T> {
    Ready(Result<T>),
    Pending(oneshot::Receiver<Result<T>>),
}

pub(super) fn push_result<T>(queue: &FakeQueue<T>, result: Result<T>) {
    queue
        .lock()
        .expect("fake GitHub API queue mutex should not be poisoned")
        .push_back(FakeResponse::Ready(result));
}

pub(super) fn push_pending_result<T>(queue: &FakeQueue<T>) -> oneshot::Sender<Result<T>> {
    let (sender, receiver) = oneshot::channel();
    queue
        .lock()
        .expect("fake response queue")
        .push_back(FakeResponse::Pending(receiver));
    sender
}

pub(super) async fn pop_result<T>(queue: &FakeQueue<T>, name: &str) -> Result<T> {
    let response = queue
        .lock()
        .expect("fake GitHub API queue mutex should not be poisoned")
        .pop_front();
    match response {
        Some(FakeResponse::Ready(result)) => result,
        Some(FakeResponse::Pending(receiver)) => receiver
            .await
            .map_err(|error| GitHubError::Transport(error.to_string()))?,
        None => Err(GitHubError::Transport(format!(
            "missing fake {name} result"
        ))),
    }
}
