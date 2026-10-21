//! Check on a secret, then reveal it. Revealing is the one-time action.
//!
//! ```sh
//! export ONETIME_BASE_URL=https://eu.onetimesecret.com
//! cargo run --example reveal -- https://eu.onetimesecret.com/secret/<id> [passphrase]
//! ```

use onetimesecret::secrecy::ExposeSecret;
use onetimesecret::{Client, SecretId};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let id: SecretId = args
        .next()
        .ok_or("usage: reveal <link-or-id> [passphrase]")?
        .parse()?;
    let passphrase = args.next();

    let client = Client::from_env()?;
    let secrets = client.secrets();

    match secrets.status(&id).await {
        Ok(status) => println!(
            "state: {}, time left: {:?}",
            status.secret.state, status.remaining
        ),
        Err(e) if e.is_not_found() => {
            println!("that secret is gone (revealed, burned, or expired)");
            return Ok(());
        }
        Err(e) => return Err(e.into()),
    }

    let revealed = match passphrase {
        Some(pw) => secrets.reveal_with_passphrase(&id, pw).await?,
        None => secrets.reveal(&id).await?,
    };
    println!("value: {}", revealed.value.expose_secret());
    Ok(())
}
