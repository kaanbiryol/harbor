use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

use harbor_github::{GitHubError, Result};

pub(super) type FakeQueue<T> = Arc<Mutex<VecDeque<Result<T>>>>;

pub(super) fn push_result<T>(queue: &FakeQueue<T>, result: Result<T>) {
    queue
        .lock()
        .expect("fake GitHub API queue mutex should not be poisoned")
        .push_back(result);
}

pub(super) fn pop_result<T>(queue: &FakeQueue<T>, name: &str) -> Result<T> {
    queue
        .lock()
        .expect("fake GitHub API queue mutex should not be poisoned")
        .pop_front()
        .unwrap_or_else(|| {
            Err(GitHubError::Transport(format!(
                "missing fake {name} result"
            )))
        })
}
