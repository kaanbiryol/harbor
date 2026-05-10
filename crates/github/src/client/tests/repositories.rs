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

    smol::block_on(client.list_repositories()).unwrap();

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
            ("per_page".to_string(), "100".to_string()),
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
fn paginates_user_repositories_endpoint() {
    let transport = RecordingTransport::default();
    *transport
        .get_responses
        .lock()
        .expect("get responses mutex should not be poisoned") = vec![
        Value::Array(
            (0..REPOSITORY_PAGE_SIZE)
                .map(|index| {
                    json!({
                        "name": format!("app-{index}"),
                        "owner": { "login": "acme" },
                    })
                })
                .collect(),
        ),
        json!([
            {
                "name": "last",
                "owner": { "login": "acme" },
            }
        ]),
    ];
    let client = GitHubClient::new(transport.clone());

    let repositories = smol::block_on(client.list_repositories()).unwrap();

    assert_eq!(repositories.len(), REPOSITORY_PAGE_SIZE + 1);
    assert_eq!(repositories[REPOSITORY_PAGE_SIZE].full_name(), "acme/last");

    let gets = transport
        .gets
        .lock()
        .expect("gets mutex should not be poisoned");
    assert_eq!(gets.len(), 2);
    assert_eq!(gets[0].0, "/user/repos");
    assert_eq!(
        gets[1].1,
        vec![
            (
                "affiliation".to_string(),
                "owner,collaborator,organization_member".to_string()
            ),
            ("per_page".to_string(), "100".to_string()),
            ("sort".to_string(), "updated".to_string()),
            ("page".to_string(), "2".to_string()),
        ]
    );
}
