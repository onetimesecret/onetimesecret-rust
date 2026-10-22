//! List your recent receipts and burn one. Needs credentials.
//!
//! ```sh
//! export ONETIME_BASE_URL=https://eu.onetimesecret.com
//! export ONETIME_CUSTOMER_EXTID=ur...
//! export ONETIME_API_TOKEN=...
//! cargo run --example receipts            # list
//! cargo run --example receipts -- <id>    # burn
//! ```

use std::process::ExitCode;

use onetimesecret::{Client, ReceiptId};

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            match e.downcast_ref::<onetimesecret::Error>() {
                Some(e) if e.is_unauthorized() => eprintln!(
                    "error: the server rejected ONETIME_CUSTOMER_EXTID and ONETIME_API_TOKEN ({e})"
                ),
                _ => eprintln!("error: {e}"),
            }
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::from_env()?;
    if !client.is_authenticated() {
        return Err(
            "receipts belong to an account: set ONETIME_CUSTOMER_EXTID and ONETIME_API_TOKEN"
                .into(),
        );
    }
    let receipts = client.receipts();

    if let Some(id) = std::env::args().nth(1) {
        let burned = receipts.burn(ReceiptId::parse(&id)?).await?;
        println!("burned {} (state now {})", burned.identifier, burned.state);
        return Ok(());
    }

    for receipt in receipts.recent().await? {
        println!(
            "{:<10} {:<9} {:<8} {}",
            receipt.shortid,
            receipt.state,
            receipt.kind.as_ref().map_or("?", |k| k.as_str()),
            receipt.memo.as_deref().unwrap_or("")
        );
    }
    Ok(())
}
