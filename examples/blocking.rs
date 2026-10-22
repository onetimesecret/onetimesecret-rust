//! The same conceal flow without an async runtime.
//!
//! ```sh
//! export ONETIME_BASE_URL=https://eu.onetimesecret.com
//! export ONETIME_CUSTOMER_EXTID=ur...      # optional: omit both for anonymous use
//! export ONETIME_API_TOKEN=...
//! cargo run --example blocking --features blocking -- "the launch codes"
//! ```

use std::process::ExitCode;
use std::time::Duration;

use onetimesecret::ConcealSecret;
use onetimesecret::blocking::Client;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), onetimesecret::Error> {
    let value = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "hello from the blocking client".to_owned());

    let client = Client::from_env()?;
    let created = client
        .secrets()
        .conceal(ConcealSecret::new(value).ttl(Duration::from_secs(3600)))?;

    println!("share link : {}", created.share_url);
    println!("receipt id : {}", created.receipt.identifier);
    println!("expires in : {:?}", created.receipt.secret_ttl);
    Ok(())
}
