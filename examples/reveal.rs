//! Check on a secret, then reveal it. Revealing is the one-time action.
//!
//! ```sh
//! export ONETIME_BASE_URL=https://eu.onetimesecret.com
//! cargo run --example reveal -- https://eu.onetimesecret.com/secret/<id> [passphrase]
//! ```

use std::process::ExitCode;

use onetimesecret::secrecy::ExposeSecret;
use onetimesecret::{Client, SecretId};

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let id: SecretId = args
        .next()
        .ok_or("usage: reveal <link-or-id> [passphrase]")?
        .parse()?;
    let passphrase = args.next();

    let client = Client::from_env()?;
    let secrets = client.secrets();

    let status = secrets.status(&id).await?;
    let Some(secret) = &status.secret else {
        println!("that secret is gone (revealed, burned, or expired) or never existed");
        return Ok(());
    };
    println!("state: {}, time left: {:?}", secret.state, status.remaining);

    let revealed = match passphrase {
        Some(pw) => secrets.reveal_with_passphrase(&id, pw).await?,
        None => secrets.reveal(&id).await?,
    };
    println!("value: {}", revealed.value.expose_secret());
    Ok(())
}
