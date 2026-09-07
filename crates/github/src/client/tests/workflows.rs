use serde_json::json;

use super::super::{GitHubClient, test_support::RecordingTransport};

#[test]
fn loads_check_runs_after_the_first_page() {
    let transport = RecordingTransport::default();
    let check = |id| json!({"id": id, "name": format!("check-{id}"), "status": "completed", "conclusion": "success"});
    *transport.get_responses.lock().unwrap() = vec![
        json!({"check_runs": (1..=100).map(check).collect::<Vec<_>>()}),
        json!({"check_runs": [check(101)]}),
    ];
    let client = GitHubClient::new(transport.clone());
    let checks = smol::block_on(client.list_check_runs("acme", "app", "head")).unwrap();
    assert_eq!(checks.len(), 101);
    assert_eq!(checks.last().unwrap().id, Some(101));
    assert!(
        transport.gets.lock().unwrap()[1]
            .1
            .contains(&("page".into(), "2".into()))
    );
}

#[test]
fn loads_workflow_runs_after_the_first_page_for_the_same_head() {
    let transport = RecordingTransport::default();
    let run = |id| {
        json!({
            "id": id, "name": format!("workflow-{id}"), "status": "completed", "conclusion": "success",
            "head_sha": "head", "head_branch": "feature", "event": "pull_request",
            "url": format!("https://api.github.com/repos/acme/app/actions/runs/{id}"),
            "html_url": format!("https://github.com/acme/app/actions/runs/{id}"),
            "created_at": "2026-09-07T12:00:00Z", "updated_at": "2026-09-07T12:00:00Z"
        })
    };
    *transport.get_responses.lock().unwrap() = vec![
        json!({"workflow_runs": (1..=100).map(run).collect::<Vec<_>>()}),
        json!({"workflow_runs": [run(101)]}),
    ];
    let client = GitHubClient::new(transport.clone());
    let runs = smol::block_on(client.list_workflow_runs_for_head("acme", "app", "head")).unwrap();
    assert_eq!(runs.len(), 101);
    assert_eq!(runs.last().unwrap().id, 101);
    let gets = transport.gets.lock().unwrap();
    assert!(gets[1].1.contains(&("page".into(), "2".into())));
    assert!(
        gets.iter()
            .all(|(_, query)| query.contains(&("head_sha".into(), "head".into())))
    );
}

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

    let page = smol::block_on(client.list_repository_workflow_run_page("acme", "app", 1)).unwrap();

    let gets = transport
        .gets
        .lock()
        .expect("gets mutex should not be poisoned");
    assert_eq!(gets.len(), 1);
    assert_eq!(gets[0].0, "/repos/acme/app/actions/runs");
    assert_eq!(gets[0].1, vec![("per_page".to_string(), "100".to_string())]);
    assert_eq!(page.next_page, None);
}

#[test]
fn gets_second_repository_workflow_run_page() {
    let transport = RecordingTransport::default();
    *transport
        .get_response
        .lock()
        .expect("get response mutex should not be poisoned") = Some(json!({
        "total_count": 250,
        "workflow_runs": []
    }));
    let client = GitHubClient::new(transport.clone());

    let page = smol::block_on(client.list_repository_workflow_run_page("acme", "app", 2)).unwrap();

    let gets = transport
        .gets
        .lock()
        .expect("gets mutex should not be poisoned");
    assert_eq!(gets.len(), 1);
    assert_eq!(
        gets[0].1,
        vec![
            ("per_page".to_string(), "100".to_string()),
            ("page".to_string(), "2".to_string())
        ]
    );
    assert_eq!(page.total_count, 250);
    assert_eq!(page.next_page, Some(3));
}

#[test]
fn gets_workflow_run_page_for_workflow() {
    let transport = RecordingTransport::default();
    *transport
        .get_response
        .lock()
        .expect("get response mutex should not be poisoned") = Some(json!({
        "total_count": 0,
        "workflow_runs": []
    }));
    let client = GitHubClient::new(transport.clone());

    smol::block_on(client.list_workflow_run_page_for_workflow("acme", "app", 901, 1)).unwrap();

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

    let log = smol::block_on(client.workflow_run_log("acme", "app", 42, None)).unwrap();

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

#[test]
fn loads_jobs_for_the_selected_attempt() {
    let transport = RecordingTransport::default();
    *transport.get_response.lock().unwrap() = Some(json!({"jobs": []}));
    let client = GitHubClient::new(transport.clone());
    smol::block_on(client.list_workflow_jobs_for_run("acme", "app", 42, Some(2))).unwrap();
    assert_eq!(
        transport.gets.lock().unwrap()[0].0,
        "/repos/acme/app/actions/runs/42/attempts/2/jobs"
    );
}
