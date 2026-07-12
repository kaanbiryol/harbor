use std::io::{Cursor, Read};

use http::{HeaderMap, StatusCode, header};
use serde_json::Value;
use zip::ZipArchive;

use crate::{GitHubError, GitHubRateLimitStatus, HttpCacheValidator, Result};

pub(super) struct ParsedJsonOutput {
    pub(super) value: Value,
    pub(super) metadata: GitHubResponseMetadata,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct GitHubRateLimitMetadata {
    pub(super) retry_after_seconds: Option<u64>,
    pub(super) reset_epoch_seconds: Option<u64>,
    pub(super) resource: Option<String>,
    pub(super) remaining: Option<u64>,
    pub(super) limit: Option<u64>,
    pub(super) used: Option<u64>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct GitHubResponseMetadata {
    pub(super) status_code: Option<u16>,
    pub(super) validator: Option<HttpCacheValidator>,
    pub(super) rate_limit: GitHubRateLimitMetadata,
}

impl GitHubRateLimitMetadata {
    pub(super) fn into_status(self) -> Option<GitHubRateLimitStatus> {
        (self.retry_after_seconds.is_some()
            || self.reset_epoch_seconds.is_some()
            || self.resource.is_some()
            || self.remaining.is_some()
            || self.limit.is_some())
        .then_some(GitHubRateLimitStatus {
            retry_after_seconds: self.retry_after_seconds,
            reset_epoch_seconds: self.reset_epoch_seconds,
            resource: self.resource,
            remaining: self.remaining,
            limit: self.limit,
            used: self.used,
        })
    }
}

pub(super) fn parse_json_output(stdout: &[u8]) -> Result<ParsedJsonOutput> {
    let metadata = parse_response_metadata(stdout);
    if stdout.is_empty() {
        return Ok(ParsedJsonOutput {
            value: Value::Null,
            metadata,
        });
    }

    let Some(json) = json_body(stdout) else {
        return Ok(ParsedJsonOutput {
            value: Value::Null,
            metadata,
        });
    };
    let json = json.trim_ascii();
    let value = if json.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(json).map_err(|error| GitHubError::Mapping(error.to_string()))?
    };

    Ok(ParsedJsonOutput { value, metadata })
}

pub(super) fn json_body(stdout: &[u8]) -> Option<&[u8]> {
    stdout
        .iter()
        .position(|byte| matches!(byte, b'{' | b'['))
        .map(|index| &stdout[index..])
}

pub(super) fn parse_rate_limit_metadata(stdout: &[u8]) -> GitHubRateLimitMetadata {
    parse_response_metadata(stdout).rate_limit
}

pub(super) fn parse_response_metadata(stdout: &[u8]) -> GitHubResponseMetadata {
    let header_bytes = json_body(stdout)
        .and_then(|json_body| stdout.len().checked_sub(json_body.len()))
        .map_or(stdout, |json_start| &stdout[..json_start]);
    let mut rate_limit = GitHubRateLimitMetadata::default();
    let mut etag = None;
    let mut last_modified = None;
    let mut status_code = None;

    for line in String::from_utf8_lossy(header_bytes).lines() {
        if let Some(status) = http_status_code(line) {
            status_code = Some(status);
            continue;
        }

        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();

        match key.trim().to_ascii_lowercase().as_str() {
            "etag" => etag = Some(value.to_string()),
            "last-modified" => last_modified = Some(value.to_string()),
            "retry-after" => rate_limit.retry_after_seconds = value.parse().ok(),
            "x-ratelimit-reset" => rate_limit.reset_epoch_seconds = value.parse().ok(),
            "x-ratelimit-resource" => rate_limit.resource = Some(value.to_string()),
            "x-ratelimit-remaining" => rate_limit.remaining = value.parse().ok(),
            "x-ratelimit-limit" => rate_limit.limit = value.parse().ok(),
            "x-ratelimit-used" => rate_limit.used = value.parse().ok(),
            _ => {}
        }
    }

    let validator = Some(HttpCacheValidator {
        etag,
        last_modified,
    })
    .filter(|validator| !validator.is_empty());

    GitHubResponseMetadata {
        status_code,
        validator,
        rate_limit,
    }
}

pub(super) fn response_metadata_from_headers(
    status: StatusCode,
    headers: &HeaderMap,
) -> GitHubResponseMetadata {
    let etag = header_string(headers, header::ETAG);
    let last_modified = header_string(headers, header::LAST_MODIFIED);
    let validator = Some(HttpCacheValidator {
        etag,
        last_modified,
    })
    .filter(|validator| !validator.is_empty());

    GitHubResponseMetadata {
        status_code: Some(status.as_u16()),
        validator,
        rate_limit: GitHubRateLimitMetadata {
            retry_after_seconds: header_u64(headers, header::RETRY_AFTER),
            reset_epoch_seconds: header_u64(headers, "x-ratelimit-reset"),
            resource: header_string(headers, "x-ratelimit-resource"),
            remaining: header_u64(headers, "x-ratelimit-remaining"),
            limit: header_u64(headers, "x-ratelimit-limit"),
            used: header_u64(headers, "x-ratelimit-used"),
        },
    }
}

fn header_string<K>(headers: &HeaderMap, key: K) -> Option<String>
where
    K: header::AsHeaderName,
{
    headers
        .get(key)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
}

fn header_u64<K>(headers: &HeaderMap, key: K) -> Option<u64>
where
    K: header::AsHeaderName,
{
    headers.get(key)?.to_str().ok()?.parse().ok()
}

pub(super) fn json_value_from_body(body: &[u8]) -> Result<Value> {
    let body = body.trim_ascii();
    if body.is_empty() {
        return Ok(Value::Null);
    }

    serde_json::from_slice(body).map_err(|error| GitHubError::Mapping(error.to_string()))
}

pub(super) fn workflow_log_text_from_zip(body: &[u8]) -> Result<String> {
    let reader = Cursor::new(body);
    let mut archive =
        ZipArchive::new(reader).map_err(|error| GitHubError::Mapping(error.to_string()))?;
    let mut entries = Vec::new();

    for index in 0..archive.len() {
        let mut file = archive
            .by_index(index)
            .map_err(|error| GitHubError::Mapping(error.to_string()))?;
        if file.is_dir() {
            continue;
        }

        let name = file.name().to_string();
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)
            .map_err(|error| GitHubError::Mapping(error.to_string()))?;
        entries.push((name, bytes));
    }

    entries.sort_by(|left, right| left.0.cmp(&right.0));

    let output_capacity = entries.iter().map(|(_, bytes)| bytes.len() + 1).sum();
    let mut output = String::with_capacity(output_capacity);
    for (_, bytes) in entries {
        if !output.is_empty() && !output.ends_with('\n') {
            output.push('\n');
        }
        output.push_str(&String::from_utf8_lossy(&bytes));
        if !output.ends_with('\n') {
            output.push('\n');
        }
    }

    Ok(output)
}

fn http_status_code(line: &str) -> Option<u16> {
    let mut parts = line.split_whitespace();
    let protocol = parts.next()?;
    if !protocol.starts_with("HTTP/") {
        return None;
    }

    parts.next()?.parse().ok()
}

pub(super) fn graphql_response_cost(value: &Value) -> Option<u64> {
    value
        .pointer("/data/rateLimit/cost")
        .and_then(Value::as_u64)
}
