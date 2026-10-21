//! The blocking client goes through the same protocol layer. These tests
//! confirm it is wired up: a round trip, the retry sleep, and the error
//! path.

#![cfg(feature = "blocking")]

use std::time::{Duration, Instant};

use onetimesecret::blocking::Client;
use onetimesecret::secrecy::ExposeSecret;
use onetimesecret::{ConcealSecret, Credentials, GenerateSecret};
use tokio::runtime::Runtime;
use wiremock::matchers::{basic_auth, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const RECEIPT: &str = "5e60wem00rn4zj5a3itqbir40s70j4uzdb36u2k00qyz6h01fnt8keb8v05z6h";

fn fixture(name: &str) -> serde_json::Value {
    let text = std::fs::read_to_string(format!(
        "{}/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap_or_else(|e| panic!("fixture {name}: {e}"));
    serde_json::from_str(&text).unwrap()
}

fn respond(status: u16, name: &str) -> ResponseTemplate {
    ResponseTemplate::new(status).set_body_json(fixture(name))
}

/// wiremock is async; the blocking client is not. Drive the server from a
/// runtime the client never sees.
fn server(runtime: &Runtime) -> MockServer {
    runtime.block_on(MockServer::start())
}

fn mount(runtime: &Runtime, server: &MockServer, mock: Mock) {
    runtime.block_on(mock.mount(server));
}

fn anonymous(server: &MockServer) -> Client {
    Client::builder()
        .base_url(server.uri())
        .allow_http(true)
        .build()
        .unwrap()
}

fn authenticated(server: &MockServer) -> Client {
    Client::builder()
        .base_url(server.uri())
        .allow_http(true)
        .credentials(Credentials::new("ur1abc23def", "tok"))
        .build()
        .unwrap()
}

#[test]
fn blocking_client_round_trips() {
    let runtime = Runtime::new().unwrap();
    let server = server(&runtime);
    mount(
        &runtime,
        &server,
        Mock::given(method("POST"))
            .and(path("/api/v2/secret/conceal"))
            .and(basic_auth("ur1abc23def", "tok"))
            .respond_with(respond(200, "v2_conceal_200.json"))
            .expect(1),
    );

    let client = authenticated(&server);
    assert!(client.is_authenticated());

    let created = client
        .secrets()
        .conceal(ConcealSecret::new("psst"))
        .unwrap();
    assert!(created.share_url.path().starts_with("/secret/"));
    assert_eq!(created.receipt.shortid, "5e60wem0");
    drop(server);
}

#[test]
fn generate_claims_the_value() {
    const GENERATED_RECEIPT: &str =
        "ig61b2jx147uvq5mrlauro5spzjylog4iy2nvij08c90rxph8b9mjnvgqdfb9h";
    let runtime = Runtime::new().unwrap();
    let server = server(&runtime);
    mount(
        &runtime,
        &server,
        Mock::given(method("POST"))
            .and(path("/api/v2/guest/secret/generate"))
            .respond_with(respond(200, "v2_generate_200.json"))
            .expect(1),
    );
    mount(
        &runtime,
        &server,
        Mock::given(method("GET"))
            .and(path(format!("/api/v2/guest/receipt/{GENERATED_RECEIPT}")))
            .respond_with(respond(200, "v2_receipt_show_generated_200.json"))
            .expect(1),
    );

    let generated = anonymous(&server)
        .secrets()
        .generate(GenerateSecret::new())
        .unwrap();
    assert_eq!(
        generated.value.as_ref().unwrap().expose_secret(),
        "XQ+'Y*9{'$ed"
    );
    assert!(generated.claim_error.is_none());
    assert!(generated.receipt.secret_value.is_none());
    drop(server);
}

#[test]
fn a_retry_sleeps_for_retry_after_when_it_exceeds_the_backoff() {
    let runtime = Runtime::new().unwrap();
    let server = server(&runtime);
    // One 429 asking for a second, then success. The first backoff step is
    // 500ms, so a wait of at least a second shows the header won; the cap
    // is 30s, so a wait well under that shows nothing else did.
    mount(
        &runtime,
        &server,
        Mock::given(method("GET"))
            .and(path("/api/v2/status"))
            .respond_with(
                ResponseTemplate::new(429)
                    .insert_header("retry-after", "1")
                    .set_body_string("slow down"),
            )
            .up_to_n_times(1)
            .expect(1),
    );
    mount(
        &runtime,
        &server,
        Mock::given(method("GET"))
            .and(path("/api/v2/status"))
            .respond_with(respond(200, "v2_system_status_200.json"))
            .expect(1),
    );

    let started = Instant::now();
    let status = anonymous(&server).system().status().unwrap();
    let waited = started.elapsed();

    assert!(status.is_nominal());
    assert!(waited >= Duration::from_secs(1), "waited {waited:?}");
    assert!(waited < Duration::from_secs(10), "waited {waited:?}");
    drop(server);
}

#[test]
fn account_routes_report_the_servers_401() {
    let runtime = Runtime::new().unwrap();
    let server = server(&runtime);
    mount(
        &runtime,
        &server,
        Mock::given(method("GET"))
            .and(path("/api/v2/receipt/recent"))
            .respond_with(respond(401, "v2_receipt_recent_401.json"))
            .expect(1),
    );
    mount(
        &runtime,
        &server,
        Mock::given(method("PATCH"))
            .and(path(format!("/api/v2/receipt/{RECEIPT}")))
            .respond_with(respond(401, "v2_bad_credentials_401.json"))
            .expect(1),
    );

    // Anonymous: no guest route exists for the receipt list.
    let err = anonymous(&server).receipts().recent().unwrap_err();
    assert!(err.is_unauthorized());
    assert_eq!(err.as_api().unwrap().message, "Authentication Required");

    // Wrong credentials: the memo route exists, the token does not.
    let err = authenticated(&server)
        .receipts()
        .update_memo(RECEIPT, "note")
        .unwrap_err();
    assert!(err.is_unauthorized());
    assert!(
        err.as_api()
            .unwrap()
            .detail
            .as_deref()
            .unwrap()
            .contains("CREDENTIALS_INVALID")
    );
    drop(server);
}
