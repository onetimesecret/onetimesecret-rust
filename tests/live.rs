//! Live tests against a real Onetime Secret server.
//!
//! Opt-in: unless `ONETIME_LIVE=1` is set every test returns at once, so a
//! plain `cargo test` never touches the network.
//!
//! ```sh
//! ONETIME_LIVE=1 cargo test --test live
//! ONETIME_LIVE=1 ONETIME_BASE_URL=http://127.0.0.1:7143 cargo test --test live
//! ```
//!
//! Configuration:
//!
//! - `ONETIME_LIVE`: set to `1` to run these tests.
//! - `ONETIME_BASE_URL`: the server. `Client::from_env` has no default and
//!   fails without it; these tests fall back to the hosted EU region,
//!   `https://eu.onetimesecret.com`, the same default as the JS and Python
//!   SDKs' live suites.
//! - `ONETIME_CUSTOMER_EXTID` and `ONETIME_API_TOKEN`: optional. Without
//!   them the client is anonymous and the tests that need an account
//!   return early.
//!
//! Each test creates real secrets with a short TTL and clearly marked test
//! content. Assertions check round trips and states, not values the server
//! chooses.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use onetimesecret::secrecy::ExposeSecret;
use onetimesecret::{
    ApiErrorKind, Client, ConcealSecret, ENV_BASE_URL, GenerateSecret, ReceiptState, SecretState,
};

const DEFAULT_BASE_URL: &str = "https://eu.onetimesecret.com";
const TTL: Duration = Duration::from_secs(60);

/// The server to test against, or `None` when live tests are off.
fn base_url() -> Option<String> {
    if std::env::var("ONETIME_LIVE").as_deref() != Ok("1") {
        return None;
    }
    let url = std::env::var(ENV_BASE_URL)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_BASE_URL.to_owned());
    Some(url)
}

/// The client under test, or `None` when live tests are off.
fn live() -> Option<Client> {
    let client = Client::builder()
        .base_url(base_url()?)
        .from_env()
        .and_then(onetimesecret::ClientBuilder::build)
        .expect("a client from ONETIME_BASE_URL, ONETIME_CUSTOMER_EXTID, ONETIME_API_TOKEN");
    Some(client)
}

/// `false`, with a note, when the client has no credentials.
fn has_account(client: &Client) -> bool {
    if !client.is_authenticated() {
        eprintln!("skipped: needs ONETIME_CUSTOMER_EXTID and ONETIME_API_TOKEN");
    }
    client.is_authenticated()
}

fn unique(label: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!(
        "onetimesecret-rust live test {label} {nanos}-{}",
        std::process::id()
    )
}

#[tokio::test]
async fn status_is_nominal() {
    let Some(client) = live() else { return };
    let status = client.system().status().await.unwrap();
    assert!(status.is_nominal(), "status: {}", status.status);
}

#[tokio::test]
async fn supported_locales_are_listed() {
    let Some(client) = live() else { return };
    let locales = client.system().supported_locales().await.unwrap();
    assert!(!locales.locales.is_empty());
    assert!(!locales.default_locale.is_empty());
}

#[tokio::test]
async fn version_is_reported_to_an_account() {
    let Some(client) = live() else { return };
    if !has_account(&client) {
        return;
    }
    let version = client.system().version().await.unwrap();
    assert!(!version.version.is_empty());
}

#[tokio::test]
async fn conceal_reveal_walks_the_lifecycle() {
    let Some(client) = live() else { return };
    let value = unique("conceal-reveal");

    let created = client
        .secrets()
        .conceal(ConcealSecret::new(value.clone()).ttl(TTL))
        .await
        .unwrap();
    let secret_id = created.secret.identifier.clone();
    let receipt_id = created.receipt.identifier.clone();

    let receipt = client.receipts().show(&receipt_id).await.unwrap();
    assert_eq!(receipt.identifier, receipt_id);
    // A local server must run with HOST set to the address in ONETIME_BASE_URL.
    assert_eq!(receipt.share_url.as_ref(), Some(&created.share_url));
    assert_eq!(receipt.state, ReceiptState::New);
    assert!(receipt.is_pending());

    let status = client.secrets().status(&secret_id).await.unwrap();
    assert_eq!(status.state(), Some(&SecretState::New));

    let revealed = client.secrets().reveal(&secret_id).await.unwrap();
    assert_eq!(revealed.value.expose_secret(), value);

    let again = client.secrets().reveal(&secret_id).await.unwrap_err();
    assert!(again.is_not_found(), "second reveal: {again}");

    let receipt = client.receipts().show(&receipt_id).await.unwrap();
    assert_eq!(receipt.state, ReceiptState::Revealed);
    assert!(!receipt.is_pending());
}

#[tokio::test]
async fn a_passphrase_guards_the_secret() {
    let Some(client) = live() else { return };
    let value = unique("passphrase");
    let created = client
        .secrets()
        .conceal(
            ConcealSecret::new(value.clone())
                .ttl(TTL)
                .passphrase("correct horse"),
        )
        .await
        .unwrap();
    let id = created.secret.identifier.clone();
    assert!(created.secret.has_passphrase);

    let wrong = client
        .secrets()
        .reveal_with_passphrase(&id, "battery staple")
        .await
        .unwrap_err();
    assert_eq!(
        wrong.as_api().map(|e| e.kind),
        Some(ApiErrorKind::Invalid),
        "wrong passphrase: {wrong}"
    );
    let status = client.secrets().status(&id).await.unwrap();
    assert!(status.is_known(), "a wrong passphrase consumed the secret");

    let revealed = client
        .secrets()
        .reveal_with_passphrase(&id, "correct horse")
        .await
        .unwrap();
    assert_eq!(revealed.value.expose_secret(), value);
}

#[tokio::test]
async fn generate_claims_the_value_once() {
    let Some(client) = live() else { return };
    let generated = client
        .secrets()
        .generate(GenerateSecret::new().ttl(TTL).length(20))
        .await
        .unwrap();
    assert!(
        generated.claim_error.is_none(),
        "claim failed: {:?}",
        generated.claim_error
    );
    let value = generated.value.as_ref().expect("the claimed value");
    assert_eq!(value.expose_secret().chars().count(), 20);

    let receipt = client
        .receipts()
        .show(&generated.receipt.identifier)
        .await
        .unwrap();
    assert!(
        receipt.secret_value.is_none(),
        "the value is disclosed only on the first fetch"
    );
}

#[tokio::test]
async fn generate_without_a_claim_leaves_the_value_on_the_receipt() {
    let Some(client) = live() else { return };
    let generated = client
        .secrets()
        .generate(GenerateSecret::new().ttl(TTL).claim_value(false))
        .await
        .unwrap();
    assert!(!generated.is_claimed());
    assert!(generated.claim_error.is_none());

    let receipt = client
        .receipts()
        .show(&generated.receipt.identifier)
        .await
        .unwrap();
    assert!(
        receipt.secret_value.is_some(),
        "the first fetch should carry the unclaimed value"
    );
}

#[tokio::test]
async fn burning_destroys_the_secret() {
    let Some(client) = live() else { return };
    let created = client
        .secrets()
        .conceal(ConcealSecret::new(unique("burn")).ttl(TTL))
        .await
        .unwrap();
    let receipt_id = created.receipt.identifier.clone();

    // The burn response echoes the receipt as it was before the burn;
    // the stored receipt is what counts.
    client.receipts().burn(&receipt_id).await.unwrap();
    let receipt = client.receipts().show(&receipt_id).await.unwrap();
    assert_eq!(receipt.state, ReceiptState::Burned);
    assert!(receipt.is_burned);

    let err = client
        .secrets()
        .reveal(&created.secret.identifier)
        .await
        .unwrap_err();
    assert!(err.is_not_found(), "reveal after burn: {err}");
}

#[tokio::test]
async fn an_unknown_secret_is_not_found() {
    let Some(client) = live() else { return };
    let err = client
        .secrets()
        .reveal("doesnotexist000000000000000000000")
        .await
        .unwrap_err();
    assert!(err.is_not_found(), "{err}");
}

#[tokio::test]
async fn recent_receipts_are_listed_for_an_account() {
    let Some(client) = live() else { return };
    if !has_account(&client) {
        return;
    }
    client.receipts().recent().await.unwrap();
}

#[cfg(feature = "blocking")]
#[test]
fn the_blocking_client_reaches_the_server() {
    let Some(base_url) = base_url() else { return };
    let client = onetimesecret::blocking::Client::builder()
        .base_url(base_url)
        .from_env()
        .and_then(onetimesecret::blocking::Builder::build)
        .unwrap();
    assert!(client.system().status().unwrap().is_nominal());
}
