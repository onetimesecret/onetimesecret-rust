//! Conceal a secret and print the one-time link.
//!
//! ```sh
//! export ONETIME_BASE_URL=https://eu.onetimesecret.com
//! export ONETIME_CUSTOMER_EXTID=ur...      # optional: omit both for anonymous use
//! export ONETIME_API_TOKEN=...
//! cargo run --example conceal -- "the launch codes"
//! ```

use std::process::ExitCode;
use std::time::Duration;

use onetimesecret::{Client, ConcealSecret};

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            // A transport failure keeps its reason (connection refused, DNS,
            // TLS) at the end of the source chain.
            match std::iter::successors(e.source(), |&c| c.source()).last() {
                Some(cause) => eprintln!("error: {e} ({cause})"),
                None => eprintln!("error: {e}"),
            }
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let value = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "hello from onetimesecret-rust".to_owned());

    let client = Client::from_env()?;
    let created = client
        .secrets()
        .conceal(ConcealSecret::new(value).ttl(Duration::from_secs(3600)))
        .await?;

    println!("share link : {}", created.share_url);
    println!("receipt id : {}", created.receipt.identifier);
    println!("expires in : {:?}", created.receipt.secret_ttl);
    Ok(())
}
