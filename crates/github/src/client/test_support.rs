use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde_json::{Value, json};

pub(super) use super::requests::{REPOSITORY_PAGE_SIZE, REVIEW_COMMENT_PAGE_SIZE};
use crate::{ConditionalFetch, GitHubError, GitHubTransport, HttpCacheValidator, Result};

pub(super) type RecordedGet = (String, Vec<(String, String)>);
pub(super) type RecordedConditionalGet =
    (String, Vec<(String, String)>, Option<HttpCacheValidator>);
const FIXTURE_REVIEW_COMMENT_CREATED_AT: &str = "2026-05-01T10:00:00Z";

#[derive(Clone, Default)]
pub(super) struct RecordingTransport {
    pub(super) gets: Arc<Mutex<Vec<RecordedGet>>>,
    pub(super) conditional_gets: Arc<Mutex<Vec<RecordedConditionalGet>>>,
    pub(super) get_response: Arc<Mutex<Option<Value>>>,
    pub(super) conditional_get_response: Arc<Mutex<Option<ConditionalFetch<Value>>>>,
    pub(super) get_responses: Arc<Mutex<Vec<Value>>>,
    pub(super) posts: Arc<Mutex<Vec<(String, Value)>>>,
    pub(super) puts: Arc<Mutex<Vec<(String, Value)>>>,
    pub(super) graphql_calls: Arc<Mutex<Vec<(String, Value)>>>,
    pub(super) graphql_responses: Arc<Mutex<Vec<Value>>>,
    pub(super) graphql_response: Arc<Mutex<Option<Value>>>,
    pub(super) log: Arc<Mutex<Option<String>>>,
}

#[async_trait]
impl GitHubTransport for RecordingTransport {
    async fn rest_get(&self, path: &str, query: &[(&str, &str)]) -> Result<Value> {
        self.gets
            .lock()
            .expect("gets mutex should not be poisoned")
            .push((
                path.to_string(),
                query
                    .iter()
                    .map(|(key, value)| (key.to_string(), value.to_string()))
                    .collect(),
            ));

        {
            let mut responses = self
                .get_responses
                .lock()
                .expect("get responses mutex should not be poisoned");
            if !responses.is_empty() {
                return Ok(responses.remove(0));
            }
        }

        self.get_response
            .lock()
            .expect("get response mutex should not be poisoned")
            .clone()
            .ok_or_else(|| GitHubError::Transport("missing GET response".to_string()))
    }

    async fn rest_get_conditional(
        &self,
        path: &str,
        query: &[(&str, &str)],
        validator: Option<&HttpCacheValidator>,
    ) -> Result<ConditionalFetch<Value>> {
        self.conditional_gets
            .lock()
            .expect("conditional gets mutex should not be poisoned")
            .push((
                path.to_string(),
                query
                    .iter()
                    .map(|(key, value)| (key.to_string(), value.to_string()))
                    .collect(),
                validator.cloned(),
            ));

        if let Some(response) = self
            .conditional_get_response
            .lock()
            .expect("conditional get response mutex should not be poisoned")
            .clone()
        {
            return Ok(response);
        }

        self.rest_get(path, query)
            .await
            .map(|value| ConditionalFetch::Modified {
                value,
                validator: None,
            })
    }

    async fn rest_post(&self, path: &str, body: Value) -> Result<Value> {
        self.posts
            .lock()
            .expect("posts mutex should not be poisoned")
            .push((path.to_string(), body));
        Ok(Value::Null)
    }

    async fn rest_put(&self, path: &str, body: Value) -> Result<Value> {
        self.puts
            .lock()
            .expect("puts mutex should not be poisoned")
            .push((path.to_string(), body));
        Ok(Value::Null)
    }

    async fn workflow_run_log(&self, owner: &str, repo: &str, run_id: u64) -> Result<String> {
        let log = format!("{owner}/{repo}#{run_id}");
        *self.log.lock().expect("log mutex should not be poisoned") = Some(log.clone());
        Ok(log)
    }

    async fn graphql(&self, query: &str, variables: Value) -> Result<Value> {
        self.graphql_calls
            .lock()
            .expect("graphql calls mutex should not be poisoned")
            .push((query.to_string(), variables));
        let mut responses = self
            .graphql_responses
            .lock()
            .expect("graphql responses mutex should not be poisoned");
        if !responses.is_empty() {
            return Ok(responses.remove(0));
        }

        self.graphql_response
            .lock()
            .expect("graphql response mutex should not be poisoned")
            .clone()
            .ok_or_else(|| GitHubError::Transport("missing GraphQL response".to_string()))
    }
}

pub(super) fn review_thread_comment_json(id: &str, body: &str) -> Value {
    json!({
        "id": id,
        "body": body,
        "author": {
            "login": "reviewer",
            "avatarUrl": null
        },
        "createdAt": FIXTURE_REVIEW_COMMENT_CREATED_AT,
        "updatedAt": null,
        "path": "src/app.rs",
        "line": 42,
        "originalLine": 42,
        "viewerDidAuthor": false,
        "viewerCanUpdate": false,
        "viewerCanDelete": false,
        "viewerCanReact": true,
        "reactionGroups": []
    })
}
