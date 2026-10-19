use harbor_domain::{MergeMethod, PullRequestState, RepoId, StackMergeAction, StackMergeOutcome};
use serde_json::{Value, json};

use super::super::{GitHubClient, test_support::RecordingTransport};

fn member(number: u64, state: &str) -> Value {
    json!({
        "id": format!("PR_{number}"), "number": number, "title": format!("Layer {number}"),
        "url": format!("https://github.com/acme/app/pull/{number}"), "state": state,
        "headRefName": format!("layer-{number}"), "baseRefName": "main", "headRefOid": "abc123",
        "repository": { "name": "app", "owner": {"login": "acme"}, "viewerPermission": "WRITE" },
        "mergeStateStatus": "CLEAN", "isMergeQueueEnabled": true
    })
}

#[test]
fn loads_minimal_rest_members_and_batches_their_details_in_stack_order() {
    let transport = RecordingTransport::default();
    *transport.get_response.lock().unwrap() = Some(json!({
        "number": 17, "base": {"ref": "release"}, "pull_requests": [{"number": 8}, {"number": 2}]
    }));
    *transport.graphql_response.lock().unwrap() = Some(json!({"data": {"repository": {
        "pr2": member(2, "OPEN"), "pr8": member(8, "MERGED")
    }}}));
    let stack = smol::block_on(
        GitHubClient::new(transport.clone())
            .get_pull_request_stack(&RepoId::new("acme", "app"), 17),
    )
    .unwrap();
    let mut cached = serde_json::to_value(&stack.pull_requests[0]).unwrap();
    cached.as_object_mut().unwrap().remove("stack");
    assert_eq!(
        serde_json::from_value::<harbor_domain::PullRequest>(cached)
            .unwrap()
            .stack,
        None
    );
    assert_eq!(stack.base_ref, "release");
    assert_eq!(
        stack
            .pull_requests
            .iter()
            .map(|member| member.number)
            .collect::<Vec<_>>(),
        [8, 2]
    );
    assert_eq!(stack.pull_requests[0].state, PullRequestState::Merged);
    assert_eq!(stack.pull_requests[1].stack.as_ref().unwrap().position, 2);
    assert_eq!(stack.pull_requests[1].stack.as_ref().unwrap().size, 2);
    assert_eq!(
        transport.gets.lock().unwrap()[0].0,
        "/repos/acme/app/stacks/17"
    );
    let calls = transport.graphql_calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert!(calls[0].0.contains("pr8: pullRequest(number: 8)"));
    assert!(calls[0].0.contains("pr2: pullRequest(number: 2)"));
}

#[test]
fn rejects_partial_stack_details_and_graphql_errors() {
    let transport = RecordingTransport::default();
    *transport.get_response.lock().unwrap() = Some(json!({
        "number": 17, "base": {"ref": "main"}, "pull_requests": [{"number": 8}, {"number": 2}]
    }));
    for response in [
        json!({"data": {"repository": {"pr8": member(8, "OPEN")}}}),
        json!({"errors": [{"message": "unavailable"}]}),
    ] {
        *transport.graphql_response.lock().unwrap() = Some(response);
        assert!(
            smol::block_on(
                GitHubClient::new(transport.clone())
                    .get_pull_request_stack(&RepoId::new("acme", "app"), 17)
            )
            .is_err()
        );
    }
}

#[test]
fn enqueue_stack_polls_acceptance_until_github_confirms_it_entered_the_queue() {
    let transport = RecordingTransport::default();
    *transport.put_response.lock().unwrap() =
        Some(json!({"status": "pending", "details": {"uuid": "abc-123"}}));
    *transport.get_response.lock().unwrap() =
        Some(json!({"status": "enqueued", "details": {"message": "Added to queue"}}));
    let result = smol::block_on(
        GitHubClient::new(transport.clone()).merge_pull_request_stack(
            &RepoId::new("acme", "app"),
            8,
            "abc123",
            StackMergeAction::Enqueue,
        ),
    )
    .unwrap();
    assert_eq!(result, StackMergeOutcome::Enqueued);
    assert_eq!(
        transport.puts.lock().unwrap().as_slice(),
        [(
            "/repos/acme/app/pulls/8/merge-async".into(),
            json!({"sha": "abc123", "merge_action": "merge_queue"})
        )]
    );
    assert_eq!(
        transport.gets.lock().unwrap()[0].0,
        "/repos/acme/app/pulls/8/merge-async/abc-123"
    );
    assert!(transport.graphql_calls.lock().unwrap().is_empty());
}

#[test]
fn propagates_async_failures_instead_of_reporting_the_stack_was_enqueued() {
    let transport = RecordingTransport::default();
    *transport.put_response.lock().unwrap() =
        Some(json!({"status": "pending", "details": {"uuid": "abc-123"}}));
    *transport.get_response.lock().unwrap() =
        Some(json!({"status": "failed", "details": {"message": "PR #7 requires approval"}}));
    let error = smol::block_on(GitHubClient::new(transport).merge_pull_request_stack(
        &RepoId::new("acme", "app"),
        8,
        "abc123",
        StackMergeAction::Enqueue,
    ))
    .unwrap_err();
    assert!(error.to_string().contains("requires approval"));
}

#[test]
fn direct_stack_merges_include_the_method_and_expected_head_sha() {
    let transport = RecordingTransport::default();
    *transport.put_response.lock().unwrap() = Some(json!({"status": "merged", "details": {}}));
    let result = smol::block_on(
        GitHubClient::new(transport.clone()).merge_pull_request_stack(
            &RepoId::new("acme", "app"),
            8,
            "abc123",
            StackMergeAction::Merge(MergeMethod::Squash),
        ),
    )
    .unwrap();
    assert_eq!(result, StackMergeOutcome::Merged);
    assert_eq!(
        transport.puts.lock().unwrap()[0].1,
        json!({"sha": "abc123", "merge_action": "direct_merge", "merge_method": "squash"})
    );
    assert!(transport.gets.lock().unwrap().is_empty());
}
