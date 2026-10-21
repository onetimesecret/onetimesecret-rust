//! End-to-end tests: the async client against a mock server that replays
//! responses captured from the real API (see `fixtures/README.md`).

use std::time::{Duration, UNIX_EPOCH};

use onetimesecret::secrecy::ExposeSecret;
use onetimesecret::{
    ApiErrorKind, CharacterSets, Client, ConcealSecret, Credentials, Error, GenerateSecret,
    ReceiptId, ReceiptState, SecretId, SecretKind, SecretState,
};
use wiremock::matchers::{basic_auth, body_partial_json, header, header_exists, method, path};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

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

const SECRET: &str = "lj85fzddjygwug0bwntphasz91a36hpjc4ut4xanu9oqn0drnh1ab7ify46ib9";
const RECEIPT: &str = "5e60wem00rn4zj5a3itqbir40s70j4uzdb36u2k00qyz6h01fnt8keb8v05z6h";

#[tokio::test]
async fn conceal_anonymously_uses_the_guest_route_and_builds_the_share_link() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/guest/secret/conceal"))
        .and(header("content-type", "application/json"))
        .and(header("accept", "application/json"))
        .and(body_partial_json(serde_json::json!({
            "secret": {"kind": "conceal", "secret": "psst", "ttl": 600}
        })))
        .respond_with(respond(200, "v2_conceal_200.json"))
        .expect(1)
        .mount(&server)
        .await;

    let created = anonymous(&server)
        .secrets()
        .conceal(ConcealSecret::new("psst").ttl(Duration::from_secs(600)))
        .await
        .unwrap();

    assert_eq!(created.secret.identifier.as_str(), SECRET);
    assert_eq!(created.receipt.identifier.as_str(), RECEIPT);
    assert_eq!(
        created.share_url.as_str(),
        format!("{}/secret/{SECRET}", server.uri())
    );
    assert_eq!(created.receipt.state, ReceiptState::New);
    assert_eq!(created.receipt.kind, Some(SecretKind::Conceal));
    assert_eq!(created.receipt.secret_ttl, Duration::from_secs(600));
    assert_eq!(created.receipt.recipients, Vec::<String>::new());
    assert!(created.receipt.is_pending());
    assert!(!created.secret.has_passphrase);
    let created_at = created.secret.created.duration_since(UNIX_EPOCH).unwrap();
    assert_eq!(created_at.as_secs(), 1_792_537_823);
}

#[tokio::test]
async fn the_request_carries_the_sdk_user_agent_and_no_auth_when_anonymous() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/status"))
        .respond_with(respond(200, "v2_system_status_200.json"))
        .mount(&server)
        .await;

    let client = Client::builder()
        .base_url(server.uri())
        .allow_http(true)
        .user_agent("acme-cli/2.0")
        .build()
        .unwrap();
    let status = client.system().status().await.unwrap();
    assert!(status.is_nominal());

    let received: Vec<Request> = server.received_requests().await.unwrap();
    let ua = received[0]
        .headers
        .get("user-agent")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(ua.starts_with("acme-cli/2.0 onetimesecret-rust/"), "{ua}");
    assert!(received[0].headers.get("authorization").is_none());
}

#[tokio::test]
async fn credentials_go_out_as_basic_auth_on_the_regular_routes() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/secret/generate"))
        .and(basic_auth("ur1abc23def", "tok"))
        .and(body_partial_json(serde_json::json!({
            "secret": {
                "kind": "generate",
                "passphrase": "pw123",
                "length": 24,
                "character_sets": {"symbols": false}
            }
        })))
        .respond_with(respond(200, "v2_generate_200.json"))
        .expect(1)
        .mount(&server)
        .await;

    let created = authenticated(&server)
        .secrets()
        .generate(
            GenerateSecret::new()
                .passphrase("pw123")
                .length(24)
                .character_sets(CharacterSets::default().symbols(false)),
        )
        .await
        .unwrap();
    assert_eq!(created.receipt.kind, Some(SecretKind::Generate));
    assert!(created.receipt.has_passphrase);
    assert!(created.secret.has_passphrase);
}

#[tokio::test]
async fn a_custom_share_domain_is_reflected_in_the_link() {
    let server = MockServer::start().await;
    let mut body = fixture("v2_conceal_200.json");
    body["record"]["share_domain"] = "secrets.example.com".into();
    Mock::given(method("POST"))
        .and(path("/api/v2/secret/conceal"))
        .and(body_partial_json(serde_json::json!({
            "secret": {"share_domain": "secrets.example.com"}
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(&server)
        .await;

    let created = authenticated(&server)
        .secrets()
        .conceal(ConcealSecret::new("psst").share_domain("secrets.example.com"))
        .await
        .unwrap();
    assert_eq!(
        created.share_url.as_str(),
        format!("https://secrets.example.com/secret/{SECRET}")
    );
    assert_eq!(created.share_domain.as_deref(), Some("secrets.example.com"));
}

#[tokio::test]
async fn status_show_and_reveal_walk_the_lifecycle() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/api/v2/secret/{SECRET}/status")))
        .respond_with(respond(200, "v2_secret_status_200.json"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/api/v2/guest/secret/{SECRET}")))
        .respond_with(respond(200, "v2_secret_show_200.json"))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path(format!("/api/v2/guest/secret/{SECRET}/reveal")))
        .and(body_partial_json(serde_json::json!({"continue": true})))
        .respond_with(respond(200, "v2_secret_reveal_200.json"))
        .mount(&server)
        .await;

    let client = anonymous(&server);
    let secrets = client.secrets();

    // A full link is accepted wherever an id is.
    let link = format!("https://eu.onetimesecret.com/secret/{SECRET}");
    let status = secrets.status(link.as_str()).await.unwrap();
    assert!(status.is_known());
    assert_eq!(status.state(), Some(&SecretState::New));
    assert_eq!(status.secret.unwrap().state, SecretState::New);
    assert_eq!(status.remaining, Some(Duration::from_secs(563)));

    let shown = secrets.show(SecretId::new(SECRET)).await.unwrap();
    assert_eq!(shown.identifier.as_str(), SECRET);
    assert_eq!(shown.lifespan, Some(Duration::from_secs(600)));

    let revealed = secrets.reveal(SECRET).await.unwrap();
    assert_eq!(
        revealed.value.expose_secret(),
        "sdk fixture capture, safe to ignore"
    );
    assert_eq!(revealed.secret.state, SecretState::Revealed);
    assert!(revealed.secret.is_revealed);
    assert!(!format!("{revealed:?}").contains("safe to ignore"));
}

#[tokio::test]
async fn a_secret_the_server_does_not_hold_has_an_unknown_status() {
    // Gone secrets and unknown identifiers alike: HTTP 200, no record.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/secret/doesnotexist1234/status"))
        .respond_with(respond(200, "v2_secret_status_unknown_200.json"))
        .mount(&server)
        .await;

    let status = anonymous(&server)
        .secrets()
        .status("doesnotexist1234")
        .await
        .unwrap();
    assert!(!status.is_known());
    assert_eq!(status.state(), None);
    assert!(status.secret.is_none());
    assert_eq!(status.remaining, None);
}

#[tokio::test]
async fn status_many_posts_the_identifiers() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/secret/status"))
        .and(body_partial_json(serde_json::json!({
            "identifiers": [SECRET, "doesnotexist1234"]
        })))
        .respond_with(respond(200, "v2_secret_status_list_200.json"))
        .mount(&server)
        .await;

    let found = anonymous(&server)
        .secrets()
        .status_many([SECRET, "doesnotexist1234"])
        .await
        .unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].identifier.as_str(), SECRET);
}

#[tokio::test]
async fn receipts_show_recent_burn_and_memo() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/api/v2/receipt/{RECEIPT}")))
        .respond_with(respond(200, "v2_receipt_show_200.json"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v2/receipt/recent"))
        .and(basic_auth("ur1abc23def", "tok"))
        .respond_with(respond(200, "v2_receipt_recent_200.json"))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path(format!("/api/v2/receipt/{RECEIPT}/burn")))
        .and(body_partial_json(
            serde_json::json!({"continue": true, "passphrase": "pw123"}),
        ))
        .respond_with(respond(200, "v2_receipt_burn_200.json"))
        .mount(&server)
        .await;
    Mock::given(method("PATCH"))
        .and(path(format!("/api/v2/receipt/{RECEIPT}")))
        .and(body_partial_json(serde_json::json!({"memo": "for ops"})))
        .respond_with(respond(200, "v2_receipt_show_200.json"))
        .mount(&server)
        .await;

    let client = authenticated(&server);
    let receipts = client.receipts();

    let receipt = receipts.show(ReceiptId::new(RECEIPT)).await.unwrap();
    assert_eq!(
        receipt.secret_identifier.as_ref().map(SecretId::as_str),
        Some(SECRET)
    );
    assert_eq!(receipt.secret_state, Some(SecretState::New));
    assert_eq!(receipt.natural_expiration.as_deref(), Some("10 minutes"));
    assert_eq!(
        receipt.share_url.as_ref().map(url::Url::as_str),
        Some(format!("https://eu.onetimesecret.com/secret/{SECRET}").as_str())
    );
    assert!(receipt.is_previewed);
    assert!(receipt.previewed.is_some());
    assert!(receipt.is_pending());

    let recent = receipts.recent().await.unwrap();
    assert_eq!(recent.len(), 1);

    let burned = receipts
        .burn_with_passphrase(
            format!("https://eu.onetimesecret.com/receipt/{RECEIPT}"),
            "pw123",
        )
        .await
        .unwrap();
    // The burn fixture came from the generated secret of the same capture.
    assert_eq!(
        burned.identifier.as_str(),
        "ig61b2jx147uvq5mrlauro5spzjylog4iy2nvij08c90rxph8b9mjnvgqdfb9h"
    );
    assert_eq!(burned.kind, Some(SecretKind::Generate));

    let updated = receipts.update_memo(RECEIPT, "for ops").await.unwrap();
    assert_eq!(updated.identifier.as_str(), RECEIPT);
}

#[tokio::test]
async fn the_first_fetch_of_a_generated_secrets_receipt_carries_the_value() {
    const GENERATED: &str = "5s2kbpsa1zj8ixfiijeo0cnh3axrbnj6888dq5j03hwtccwdt3sxyk55kmato7";
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/api/v2/guest/receipt/{GENERATED}")))
        .respond_with(respond(200, "v2_receipt_show_generated_200.json"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/api/v2/guest/receipt/{RECEIPT}")))
        .respond_with(respond(200, "v2_receipt_show_200.json"))
        .mount(&server)
        .await;

    let receipts = anonymous(&server);
    let receipts = receipts.receipts();

    let receipt = receipts.show(GENERATED).await.unwrap();
    assert_eq!(receipt.state, ReceiptState::New);
    assert_eq!(
        receipt.secret_value.as_ref().unwrap().expose_secret(),
        "XQ+'Y*9{'$ed"
    );
    // The value never reaches Debug output.
    assert!(!format!("{receipt:?}").contains("XQ+'Y*9"));

    // A concealed secret's receipt has no value to disclose.
    let receipt = receipts.show(RECEIPT).await.unwrap();
    assert!(receipt.secret_value.is_none());
}

#[tokio::test]
async fn a_revealed_receipt_reports_the_secret_as_gone() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/api/v2/guest/receipt/{RECEIPT}")))
        .respond_with(respond(200, "v2_receipt_show_after_reveal_200.json"))
        .mount(&server)
        .await;

    let receipt = anonymous(&server).receipts().show(RECEIPT).await.unwrap();
    assert_eq!(receipt.state, ReceiptState::Revealed);
    assert!(receipt.is_destroyed);
    assert!(!receipt.is_pending());
    assert!(receipt.secret_identifier.is_none());
    assert!(receipt.revealed.is_some());
}

#[tokio::test]
async fn error_bodies_become_typed_api_errors() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(format!("/api/v2/guest/secret/{SECRET}/reveal")))
        .respond_with(respond(422, "v2_secret_reveal_wrong_passphrase_422.json"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v2/guest/receipt/nope123"))
        .respond_with(respond(404, "v2_receipt_show_404.json"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v2/receipt/recent"))
        .respond_with(respond(401, "v2_receipt_recent_401.json"))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/v2/guest/secret/conceal"))
        .respond_with(respond(422, "v2_conceal_422.json"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v2/status"))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("retry-after", "7")
                .set_body_string("slow down"),
        )
        .mount(&server)
        .await;

    let client = anonymous(&server);

    let err = client
        .secrets()
        .reveal_with_passphrase(SECRET, "wrong")
        .await
        .unwrap_err();
    let api = err.as_api().unwrap();
    assert_eq!(api.kind, ApiErrorKind::Invalid);
    assert_eq!(api.message, "Incorrect passphrase");
    assert_eq!(api.request_id.as_deref(), Some("3i0rl31u57g7d"));

    let err = client.receipts().show("nope123").await.unwrap_err();
    assert!(err.is_not_found());
    assert_eq!(
        err.as_api().unwrap().error_type.as_deref(),
        Some("RecordNotFound")
    );
    assert_eq!(err.to_string(), "not found (HTTP 404): identifier: nope123");

    let err = client.receipts().recent().await.unwrap_err();
    assert!(err.is_unauthorized());
    let api = err.as_api().unwrap();
    assert_eq!(api.message, "Authentication Required");
    assert!(
        api.detail
            .as_deref()
            .unwrap()
            .contains("AUTH_HEADER_MISSING")
    );

    let err = client
        .secrets()
        .conceal(ConcealSecret::new(""))
        .await
        .unwrap_err();
    let api = err.as_api().unwrap();
    assert_eq!(api.kind, ApiErrorKind::Invalid);
    assert_eq!(api.field.as_deref(), Some("secret"));

    let no_retries = Client::builder()
        .base_url(server.uri())
        .allow_http(true)
        .max_retries(0)
        .build()
        .unwrap();
    let err = no_retries.system().status().await.unwrap_err();
    assert!(err.is_rate_limited());
    assert!(err.as_api().unwrap().is_transient());
    assert_eq!(
        err.as_api().unwrap().retry_after,
        Some(Duration::from_secs(7))
    );
    assert_eq!(err.as_api().unwrap().message, "Rate limit exceeded");
}

#[tokio::test]
async fn a_success_body_of_the_wrong_shape_is_a_decode_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/api/v2/guest/receipt/{RECEIPT}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"record": {}})))
        .mount(&server)
        .await;

    let err = anonymous(&server)
        .receipts()
        .show(RECEIPT)
        .await
        .unwrap_err();
    assert!(matches!(err, Error::Decode { status: 200, .. }), "{err:?}");
}

#[tokio::test]
async fn transport_failures_are_distinguishable() {
    // Nothing listens here.
    let client = Client::builder()
        .base_url("http://127.0.0.1:9")
        .allow_http(true)
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap();
    let err = client.system().status().await.unwrap_err();
    assert!(matches!(err, Error::Transport(_)), "{err:?}");
    assert!(err.as_api().is_none());
    // The request path can name a secret; transport errors do not repeat it.
    let text = format!("{err} / {err:?}");
    assert!(!text.contains("/api/v2/status"), "{text}");
}

#[tokio::test]
async fn a_localhost_base_url_needs_no_opt_in_and_reaches_the_loopback_server() {
    // `localhost` is accepted by name and pinned to the loopback
    // addresses, so the request lands here regardless of what the system
    // resolver would have said.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/status"))
        .respond_with(respond(200, "v2_system_status_200.json"))
        .expect(1)
        .mount(&server)
        .await;

    let base_url = format!("http://localhost:{}", server.address().port());
    let client = Client::builder().base_url(base_url).build().unwrap();
    assert!(client.system().status().await.unwrap().is_nominal());
}

#[tokio::test]
async fn a_path_prefix_on_the_base_url_is_kept() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/ots/api/v2/supported-locales"))
        .respond_with(respond(200, "v2_supported_locales_200.json"))
        .expect(1)
        .mount(&server)
        .await;

    let client = Client::builder()
        .base_url(format!("{}/ots", server.uri()))
        .allow_http(true)
        .build()
        .unwrap();
    let locales = client.system().supported_locales().await.unwrap();
    assert_eq!(locales.default_locale, "en");
    assert!(locales.locales.iter().any(|l| l == "fr_CA"));
}

#[tokio::test]
async fn version_needs_credentials_and_comes_back_in_parts() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/version"))
        .and(basic_auth("ur1abc23def", "tok"))
        .respond_with(respond(200, "v2_system_version_200.json"))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v2/version"))
        .respond_with(respond(401, "v2_receipt_recent_401.json"))
        .expect(1)
        .mount(&server)
        .await;

    let version = authenticated(&server).system().version().await.unwrap();
    assert_eq!(version.version, vec!["0", "26", "5"]);
    assert_eq!(version.to_dotted(), "0.26.5");
    assert_eq!(version.locale.as_deref(), Some("en"));

    // There is no guest variant: anonymous clients hit the same path and
    // get the server's 401.
    let err = anonymous(&server).system().version().await.unwrap_err();
    assert!(err.is_unauthorized());
}

#[tokio::test]
async fn v3_responses_parse_too() {
    // Not a supported mode, just a canary: the formats are the same shape
    // today, and this fails the day they diverge.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/guest/secret/conceal"))
        .respond_with(respond(200, "v3_conceal_200.json"))
        .mount(&server)
        .await;
    let created = anonymous(&server)
        .secrets()
        .conceal(ConcealSecret::new("psst"))
        .await
        .unwrap();
    assert_eq!(created.receipt.secret_ttl, Duration::from_secs(300));
}

#[tokio::test]
async fn idempotent_requests_are_retried_on_transient_failures() {
    let server = MockServer::start().await;
    // Two failures, then success: within the default of two retries.
    Mock::given(method("GET"))
        .and(path("/api/v2/status"))
        .respond_with(ResponseTemplate::new(503))
        .up_to_n_times(2)
        .expect(2)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v2/status"))
        .respond_with(respond(200, "v2_system_status_200.json"))
        .expect(1)
        .mount(&server)
        .await;

    let status = anonymous(&server).system().status().await.unwrap();
    assert!(status.is_nominal());
}

#[tokio::test]
async fn non_idempotent_requests_are_never_retried() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/guest/secret/conceal"))
        .respond_with(ResponseTemplate::new(503).set_body_string("down"))
        .expect(1)
        .mount(&server)
        .await;

    let err = anonymous(&server)
        .secrets()
        .conceal(ConcealSecret::new("psst"))
        .await
        .unwrap_err();
    assert_eq!(err.as_api().unwrap().kind, ApiErrorKind::Server);
}

#[tokio::test]
async fn retries_give_up_after_max_retries() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/status"))
        .respond_with(ResponseTemplate::new(502))
        .expect(2)
        .mount(&server)
        .await;

    let client = Client::builder()
        .base_url(server.uri())
        .allow_http(true)
        .max_retries(1)
        .build()
        .unwrap();
    let err = client.system().status().await.unwrap_err();
    assert_eq!(err.as_api().unwrap().status, 502);
}

#[tokio::test]
async fn every_request_identifies_the_client() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/status"))
        .and(header_exists("x-onetime-client"))
        .respond_with(respond(200, "v2_system_status_200.json"))
        .expect(1)
        .mount(&server)
        .await;
    anonymous(&server).system().status().await.unwrap();
    let received: Vec<Request> = server.received_requests().await.unwrap();
    let value = received[0]
        .headers
        .get("x-onetime-client")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(value.starts_with("rust:"), "{value}");
}

#[tokio::test]
async fn a_malformed_customer_id_fails_at_build_time() {
    let err = Client::builder()
        .region(onetimesecret::Region::Eu)
        .credentials(Credentials::new("user@example.com", "tok"))
        .build()
        .unwrap_err();
    assert!(matches!(err, Error::Config(_)), "{err:?}");
    assert!(err.to_string().contains("not a customer extid"));
}
