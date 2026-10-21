//! The whole lifecycle in one run: conceal a secret, look at it from both
//! sides, reveal it, and confirm that the server now reports it gone.
//!
//! ```sh
//! export ONETIME_BASE_URL=https://eu.onetimesecret.com
//! export ONETIME_CUSTOMER_EXTID=ur...      # optional: omit both for anonymous use
//! export ONETIME_API_TOKEN=...
//! cargo run --example roundtrip
//! ```
//!
//! Exits non-zero if any step fails or the revealed value or the final
//! state is not what it should be. This creates one real secret and
//! consumes it immediately.

use std::time::Duration;

use onetimesecret::secrecy::ExposeSecret;
use onetimesecret::{Client, ConcealSecret, ReceiptState, SecretState};

const VALUE: &str = "roundtrip example: safe to ignore";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::from_env()?;
    println!(
        "server     : {} ({})",
        client.base_url(),
        if client.is_authenticated() {
            "authenticated"
        } else {
            "anonymous"
        }
    );

    let status = client.system().status().await?;
    println!("status     : {}", status.status);

    // 1. Conceal: the creator's side.
    let created = client
        .secrets()
        .conceal(ConcealSecret::new(VALUE).ttl(Duration::from_secs(600)))
        .await?;
    let secret_id = created.secret.identifier.clone();
    let receipt_id = created.receipt.identifier.clone();
    println!("share link : {}", created.share_url);
    println!("receipt id : {receipt_id}");

    // 2. The receipt and the secret's status, before anyone looks.
    let receipt = client.receipts().show(&receipt_id).await?;
    println!(
        "receipt    : state={}, pending={}",
        receipt.state,
        receipt.is_pending()
    );
    assert_eq!(receipt.state, ReceiptState::New);

    let before = client.secrets().status(&secret_id).await?;
    let secret = before.secret.as_ref().expect("a fresh secret is known");
    println!(
        "secret     : state={}, time left={:?}",
        secret.state, before.remaining
    );
    assert_eq!(secret.state, SecretState::New);

    // 3. Reveal: the recipient's side, and the one-time action.
    let revealed = client.secrets().reveal(&secret_id).await?;
    println!("revealed   : {:?}", revealed.value.expose_secret());
    assert_eq!(revealed.value.expose_secret(), VALUE);
    assert_eq!(revealed.secret.state, SecretState::Revealed);

    // 4. Afterwards: the receipt says revealed, the secret is gone.
    let receipt = client.receipts().show(&receipt_id).await?;
    println!(
        "receipt    : state={}, pending={}, destroyed={}",
        receipt.state,
        receipt.is_pending(),
        receipt.is_destroyed
    );
    assert_eq!(receipt.state, ReceiptState::Revealed);
    assert!(!receipt.is_pending());

    let after = client.secrets().status(&secret_id).await?;
    println!("secret     : known={}", after.is_known());
    assert!(!after.is_known(), "a revealed secret should be gone");

    let again = client.secrets().reveal(&secret_id).await;
    println!(
        "reveal #2  : {}",
        match &again {
            Ok(_) => "succeeded (it should not have)".to_owned(),
            Err(e) => e.to_string(),
        }
    );
    assert!(again.is_err(), "a secret can only be revealed once");

    println!("ok");
    Ok(())
}
