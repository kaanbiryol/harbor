use serde_json::json;

use super::super::{GitHubClient, test_support::RecordingTransport};

#[test]
fn gets_repository_workflows() {
    let transport = RecordingTransport::default();
    *transport
        .get_response
        .lock()
        .expect("get response mutex should not be poisoned") = Some(json!({
        "total_count": 0,
        "workflows": []
    }));
    let client = GitHubClient::new(transport.clone());

    smol::block_on(client.list_workflows("acme", "app")).unwrap();

    let gets = transport
        .gets
        .lock()
        .expect("gets mutex should not be poisoned");
    assert_eq!(gets.len(), 1);
    assert_eq!(gets[0].0, "/repos/acme/app/actions/workflows");
    assert_eq!(gets[0].1, vec![("per_page".to_string(), "100".to_string())]);
}

#[test]
fn gets_repository_workflow_runs() {
    let transport = RecordingTransport::default();
    *transport
        .get_response
        .lock()
        .expect("get response mutex should not be poisoned") = Some(json!({
        "total_count": 0,
        "workflow_runs": []
    }));
    let client = GitHubClient::new(transport.clone());

    smol::block_on(client.list_repository_workflow_runs("acme", "app")).unwrap();

    let gets = transport
        .gets
        .lock()
        .expect("gets mutex should not be poisoned");
    assert_eq!(gets.len(), 1);
    assert_eq!(gets[0].0, "/repos/acme/app/actions/runs");
    assert_eq!(gets[0].1, vec![("per_page".to_string(), "100".to_string())]);
}

#[test]
fn gets_workflow_runs_for_workflow() {
    let transport = RecordingTransport::default();
    *transport
        .get_response
        .lock()
        .expect("get response mutex should not be poisoned") = Some(json!({
        "total_count": 0,
        "workflow_runs": []
    }));
    let client = GitHubClient::new(transport.clone());

    smol::block_on(client.list_workflow_runs_for_workflow("acme", "app", 901)).unwrap();

    let gets = transport
        .gets
        .lock()
        .expect("gets mutex should not be poisoned");
    assert_eq!(gets.len(), 1);
    assert_eq!(gets[0].0, "/repos/acme/app/actions/workflows/901/runs");
    assert_eq!(gets[0].1, vec![("per_page".to_string(), "100".to_string())]);
}

#[test]
fn posts_rerun_failed_jobs_endpoint() {
    let transport = RecordingTransport::default();
    let client = GitHubClient::new(transport.clone());

    smol::block_on(client.rerun_failed_jobs("acme", "app", 42)).unwrap();

    let posts = transport
        .posts
        .lock()
        .expect("posts mutex should not be poisoned");
    assert_eq!(posts.len(), 1);
    assert_eq!(
        posts[0].0,
        "/repos/acme/app/actions/runs/42/rerun-failed-jobs"
    );
    assert_eq!(posts[0].1, json!({}));
}

#[test]
fn posts_workflow_dispatch_ref() {
    let transport = RecordingTransport::default();
    let client = GitHubClient::new(transport.clone());

    smol::block_on(client.dispatch_workflow("acme", "app", 9, "feature/build")).unwrap();

    let posts = transport
        .posts
        .lock()
        .expect("posts mutex should not be poisoned");
    assert_eq!(posts.len(), 1);
    assert_eq!(posts[0].0, "/repos/acme/app/actions/workflows/9/dispatches");
    assert_eq!(posts[0].1, json!({ "ref": "feature/build" }));
}

#[test]
fn delegates_workflow_run_log() {
    let transport = RecordingTransport::default();
    let client = GitHubClient::new(transport.clone());

    let log = smol::block_on(client.workflow_run_log("acme", "app", 42)).unwrap();

    assert_eq!(log, "acme/app#42");
    assert_eq!(
        transport
            .log
            .lock()
            .expect("log mutex should not be poisoned")
            .as_deref(),
        Some("acme/app#42")
    );
}
