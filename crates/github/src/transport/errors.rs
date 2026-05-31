use std::io::ErrorKind;

use http::{HeaderMap, StatusCode};
use serde_json::Value;

use crate::{GitHubError, GitHubRateLimit};

use super::response::{
    GitHubRateLimitMetadata, json_body, parse_rate_limit_metadata, response_metadata_from_headers,
};

pub(super) fn graphql_rate_limit_error(
    value: &Value,
    metadata: &GitHubRateLimitMetadata,
) -> Option<GitHubError> {
    let errors = value.get("errors")?.as_array()?;

    for error in errors {
        let message = error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("GitHub GraphQL request was rate limited");
        let error_type = error
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let lower_message = message.to_ascii_lowercase();

        if error_type.eq_ignore_ascii_case("RATE_LIMITED") || lower_message.contains("rate limit") {
            return Some(rate_limit_error(message.to_string(), metadata.clone()));
        }
    }

    None
}

fn rate_limit_error(message: String, metadata: GitHubRateLimitMetadata) -> GitHubError {
    let lower_message = message.to_ascii_lowercase();
    let limit = GitHubRateLimit {
        message,
        retry_after_seconds: metadata.retry_after_seconds,
        reset_epoch_seconds: metadata.reset_epoch_seconds,
        resource: metadata.resource,
        remaining: metadata.remaining,
        limit: metadata.limit,
        used: metadata.used,
    };

    if lower_message.contains("secondary") || lower_message.contains("abuse") {
        GitHubError::SecondaryRateLimited(Box::new(limit))
    } else {
        GitHubError::RateLimited(Box::new(limit))
    }
}

pub(super) fn map_spawn_error(error: std::io::Error) -> GitHubError {
    if error.kind() == ErrorKind::NotFound {
        GitHubError::MissingCli
    } else {
        GitHubError::Transport(error.to_string())
    }
}

pub(super) fn map_octocrab_error(error: octocrab::Error) -> GitHubError {
    match &error {
        octocrab::Error::GitHub { source, .. }
            if source.status_code == StatusCode::UNAUTHORIZED =>
        {
            GitHubError::Unauthenticated
        }
        octocrab::Error::GitHub { source, .. } => {
            let metadata = GitHubRateLimitMetadata::default();
            let message = source.message.clone();
            if message.to_ascii_lowercase().contains("rate limit") {
                rate_limit_error(message, metadata)
            } else {
                GitHubError::Transport(source.message.clone())
            }
        }
        _ => GitHubError::Transport(error.to_string()),
    }
}

pub(super) fn map_http_failure(
    status: StatusCode,
    headers: &HeaderMap,
    body: &[u8],
) -> GitHubError {
    let metadata = response_metadata_from_headers(status, headers).rate_limit;
    let mut message = failure_message_from_body(body);
    if message.is_empty() && metadata.remaining == Some(0) {
        message = "GitHub rate limit exceeded".to_string();
    }
    let lower_message = message.to_ascii_lowercase();

    if status == StatusCode::UNAUTHORIZED
        || status == StatusCode::FORBIDDEN && lower_message.contains("bad credentials")
    {
        GitHubError::Unauthenticated
    } else if lower_message.contains("rate limit")
        || lower_message.contains("too many requests")
        || metadata.remaining == Some(0)
    {
        rate_limit_error(message, metadata)
    } else if message.is_empty() {
        GitHubError::Transport(format!("github request failed with HTTP {status}"))
    } else {
        GitHubError::Transport(message)
    }
}

fn failure_message_from_body(body: &[u8]) -> String {
    serde_json::from_slice::<Value>(body.trim_ascii())
        .ok()
        .and_then(|value| {
            value
                .get("message")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_default()
}

pub(super) fn map_failed_status(stdout: &[u8], stderr: &[u8]) -> GitHubError {
    let metadata = parse_rate_limit_metadata(stdout);
    let mut message = failure_message(stdout, stderr);
    if message.is_empty() && metadata.remaining == Some(0) {
        message = "GitHub rate limit exceeded".to_string();
    }
    let lower_message = message.to_lowercase();

    if lower_message.contains("not logged")
        || lower_message.contains("authentication")
        || lower_message.contains("gh auth login")
    {
        GitHubError::UnauthenticatedCli
    } else if lower_message.contains("rate limit")
        || lower_message.contains("too many requests")
        || metadata.remaining == Some(0)
    {
        rate_limit_error(message, metadata)
    } else if message.is_empty() {
        GitHubError::Transport("gh command exited with a non-zero status".to_string())
    } else {
        GitHubError::Transport(message)
    }
}

fn failure_message(stdout: &[u8], stderr: &[u8]) -> String {
    let stderr_message = String::from_utf8_lossy(stderr).trim().to_string();
    if !stderr_message.is_empty() {
        return stderr_message;
    }

    json_body(stdout)
        .and_then(|body| serde_json::from_slice::<Value>(body.trim_ascii()).ok())
        .and_then(|value| {
            value
                .get("message")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_default()
}
