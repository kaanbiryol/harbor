use harbor_domain::RepoId;
use serde_json::{Value, json};

use super::super::{
    GitHubClient,
    test_support::{REPOSITORY_PAGE_SIZE, RecordingTransport},
};

#[test]
fn gets_user_repositories_endpoint() {
    let transport = RecordingTransport::default();
    *transport
        .get_response
        .lock()
        .expect("get response mutex should not be poisoned") = Some(json!([]));
    let client = GitHubClient::new(transport.clone());

    let repositories = smol::block_on(client.list_repositories()).unwrap();

    assert!(!repositories.possibly_limited);
    let gets = transport
        .gets
        .lock()
        .expect("gets mutex should not be poisoned");
    assert_eq!(gets.len(), 1);
    assert_eq!(gets[0].0, "/user/repos");
    assert_eq!(
        gets[0].1,
        vec![
            (
                "affiliation".to_string(),
                "owner,collaborator,organization_member".to_string()
            ),
            ("per_page".to_string(), "10".to_string()),
            ("sort".to_string(), "updated".to_string()),
        ]
    );
}

#[test]
fn gets_current_user_login() {
    let transport = RecordingTransport::default();
    *transport
        .get_response
        .lock()
        .expect("get response mutex should not be poisoned") = Some(json!({ "login": "octocat" }));
    let client = GitHubClient::new(transport.clone());

    let login = smol::block_on(client.current_user()).unwrap();

    assert_eq!(login, "octocat");
    let gets = transport
        .gets
        .lock()
        .expect("gets mutex should not be poisoned");
    assert_eq!(gets[0].0, "/user");
}

#[test]
fn lists_first_repository_page_only() {
    let transport = RecordingTransport::default();
    *transport
        .get_responses
        .lock()
        .expect("get responses mutex should not be poisoned") = vec![Value::Array(
        (0..REPOSITORY_PAGE_SIZE)
            .map(|index| {
                json!({
                    "name": format!("app-{index}"),
                    "owner": { "login": "acme" },
                })
            })
            .collect(),
    )];
    let client = GitHubClient::new(transport.clone());

    let repositories = smol::block_on(client.list_repositories()).unwrap();

    assert_eq!(repositories.repositories.len(), REPOSITORY_PAGE_SIZE);
    assert!(repositories.possibly_limited);

    let gets = transport
        .gets
        .lock()
        .expect("gets mutex should not be poisoned");
    assert_eq!(gets.len(), 1);
    assert_eq!(gets[0].0, "/user/repos");
}

#[test]
fn gets_repository_by_full_name() {
    let transport = RecordingTransport::default();
    *transport
        .get_response
        .lock()
        .expect("get response mutex should not be poisoned") = Some(json!({
        "name": "app",
        "owner": { "login": "acme" },
    }));
    let client = GitHubClient::new(transport.clone());

    let repository = smol::block_on(client.get_repository(&RepoId::new("acme", "app"))).unwrap();

    assert_eq!(repository.full_name(), "acme/app");
    let gets = transport
        .gets
        .lock()
        .expect("gets mutex should not be poisoned");
    assert_eq!(gets.len(), 1);
    assert_eq!(gets[0].0, "/repos/acme/app");
    assert!(gets[0].1.is_empty());
}
