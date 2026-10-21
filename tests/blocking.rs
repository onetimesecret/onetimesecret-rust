//! The blocking client goes through the same protocol layer; one test
//! confirms it is wired up.

#![cfg(feature = "blocking")]

use onetimesecret::blocking::Client;
use onetimesecret::{ConcealSecret, Credentials};
use wiremock::matchers::{basic_auth, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[test]
fn blocking_client_round_trips() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let server = runtime.block_on(async {
        let server = MockServer::start().await;
        let body: serde_json::Value =
            serde_json::from_str(include_str!("fixtures/v2_conceal_200.json")).unwrap();
        Mock::given(method("POST"))
            .and(path("/api/v2/secret/conceal"))
            .and(basic_auth("ur1abc23def", "tok"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .expect(1)
            .mount(&server)
            .await;
        server
    });

    let client = Client::builder()
        .base_url(server.uri())
        .allow_http(true)
        .credentials(Credentials::new("ur1abc23def", "tok"))
        .build()
        .unwrap();
    assert!(client.is_authenticated());

    let created = client
        .secrets()
        .conceal(ConcealSecret::new("psst"))
        .unwrap();
    assert!(created.share_url.path().starts_with("/secret/"));
    assert_eq!(created.receipt.shortid, "5e60wem0");
    drop(server);
}
