use std::{
    io::{ErrorKind, Write},
    process::{Command, Stdio},
};

use async_trait::async_trait;
use serde_json::Value;

use crate::{GitHubError, Result};

#[async_trait]
pub trait GitHubTransport: Send + Sync {
    async fn rest_get(&self, path: &str, query: &[(&str, &str)]) -> Result<Value>;
    async fn rest_post(&self, path: &str, body: Value) -> Result<Value>;
    async fn rest_put(&self, path: &str, body: Value) -> Result<Value>;
    async fn workflow_run_log(&self, owner: &str, repo: &str, run_id: u64) -> Result<String>;
    async fn graphql(&self, query: &str, variables: Value) -> Result<Value>;
}

#[derive(Clone, Debug, Default)]
pub struct GhCliTransport;

impl GhCliTransport {
    pub async fn preflight(&self) -> Result<()> {
        run_status(vec!["--version".to_string()]).await?;
        run_status(vec!["auth".to_string(), "status".to_string()]).await
    }
}

#[async_trait]
impl GitHubTransport for GhCliTransport {
    async fn rest_get(&self, path: &str, query: &[(&str, &str)]) -> Result<Value> {
        let mut args = vec![
            "api".to_string(),
            "--method".to_string(),
            "GET".to_string(),
            path.to_string(),
        ];

        for (key, value) in query {
            args.push("--raw-field".to_string());
            args.push(format!("{key}={value}"));
        }

        run_json(args, None).await
    }

    async fn rest_post(&self, path: &str, body: Value) -> Result<Value> {
        let args = vec![
            "api".to_string(),
            "--method".to_string(),
            "POST".to_string(),
            path.to_string(),
            "--input".to_string(),
            "-".to_string(),
        ];

        run_json(args, Some(body.to_string())).await
    }

    async fn rest_put(&self, path: &str, body: Value) -> Result<Value> {
        let args = vec![
            "api".to_string(),
            "--method".to_string(),
            "PUT".to_string(),
            path.to_string(),
            "--input".to_string(),
            "-".to_string(),
        ];

        run_json(args, Some(body.to_string())).await
    }

    async fn workflow_run_log(&self, owner: &str, repo: &str, run_id: u64) -> Result<String> {
        let args = vec![
            "run".to_string(),
            "view".to_string(),
            run_id.to_string(),
            "--repo".to_string(),
            format!("{owner}/{repo}"),
            "--log".to_string(),
        ];

        run_text(args).await
    }

    async fn graphql(&self, query: &str, variables: Value) -> Result<Value> {
        if graphql_variables_need_input(&variables) {
            let args = vec![
                "api".to_string(),
                "graphql".to_string(),
                "--input".to_string(),
                "-".to_string(),
            ];
            return run_json(
                args,
                Some(
                    serde_json::json!({
                        "query": query,
                        "variables": variables,
                    })
                    .to_string(),
                ),
            )
            .await;
        }

        let mut args = vec![
            "api".to_string(),
            "graphql".to_string(),
            "--raw-field".to_string(),
            format!("query={query}"),
        ];

        if let Some(variables) = variables.as_object() {
            for (key, value) in variables {
                let (flag, field) = graphql_field_arg(key, value)?;
                args.push(flag);
                args.push(field);
            }
        } else if !variables.is_null() {
            return Err(GitHubError::Transport(
                "graphql variables must be a JSON object".to_string(),
            ));
        }

        run_json(args, None).await
    }
}

async fn run_status(args: Vec<String>) -> Result<()> {
    smol::unblock(move || {
        let output = Command::new("gh")
            .args(args)
            .output()
            .map_err(map_spawn_error)?;

        if output.status.success() {
            Ok(())
        } else {
            Err(map_failed_status(&output.stderr))
        }
    })
    .await
}

async fn run_json(args: Vec<String>, input: Option<String>) -> Result<Value> {
    smol::unblock(move || run_json_blocking(args, input)).await
}

async fn run_text(args: Vec<String>) -> Result<String> {
    smol::unblock(move || {
        let output = Command::new("gh")
            .args(args)
            .output()
            .map_err(map_spawn_error)?;

        if !output.status.success() {
            return Err(map_failed_status(&output.stderr));
        }

        String::from_utf8(output.stdout).map_err(|error| GitHubError::Mapping(error.to_string()))
    })
    .await
}

fn run_json_blocking(args: Vec<String>, input: Option<String>) -> Result<Value> {
    let mut command = Command::new("gh");
    command
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if input.is_some() {
        command.stdin(Stdio::piped());
    }

    let mut child = command.spawn().map_err(map_spawn_error)?;

    if let Some(input) = input
        && let Some(mut stdin) = child.stdin.take()
    {
        stdin
            .write_all(input.as_bytes())
            .map_err(|error| GitHubError::Transport(error.to_string()))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|error| GitHubError::Transport(error.to_string()))?;

    if !output.status.success() {
        return Err(map_failed_status(&output.stderr));
    }

    if output.stdout.is_empty() {
        return Ok(Value::Null);
    }

    serde_json::from_slice(&output.stdout).map_err(|error| GitHubError::Mapping(error.to_string()))
}

fn graphql_field_arg(key: &str, value: &Value) -> Result<(String, String)> {
    let field = match value {
        Value::Null => (String::from("--field"), format!("{key}=null")),
        Value::Bool(value) => (String::from("--field"), format!("{key}={value}")),
        Value::Number(value) => (String::from("--field"), format!("{key}={value}")),
        Value::String(value) => (String::from("--raw-field"), format!("{key}={value}")),
        Value::Array(_) | Value::Object(_) => {
            return Err(GitHubError::Transport(format!(
                "complex graphql variable `{key}` is not supported by GhCliTransport yet"
            )));
        }
    };

    Ok(field)
}

fn graphql_variables_need_input(variables: &Value) -> bool {
    variables
        .as_object()
        .is_some_and(|variables| variables.values().any(value_is_complex))
}

fn value_is_complex(value: &Value) -> bool {
    matches!(value, Value::Array(_) | Value::Object(_))
}

fn map_spawn_error(error: std::io::Error) -> GitHubError {
    if error.kind() == ErrorKind::NotFound {
        GitHubError::MissingCli
    } else {
        GitHubError::Transport(error.to_string())
    }
}

fn map_failed_status(stderr: &[u8]) -> GitHubError {
    let message = String::from_utf8_lossy(stderr).trim().to_string();
    let lower_message = message.to_lowercase();

    if lower_message.contains("not logged")
        || lower_message.contains("authentication")
        || lower_message.contains("gh auth login")
    {
        GitHubError::UnauthenticatedCli
    } else if message.is_empty() {
        GitHubError::Transport("gh command exited with a non-zero status".to_string())
    } else {
        GitHubError::Transport(message)
    }
}
