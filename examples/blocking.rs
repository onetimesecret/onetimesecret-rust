//! The same conceal flow without an async runtime.
//!
//! ```sh
//! cargo run --example blocking --features blocking
//! ```

use onetimesecret::blocking::Client;
use onetimesecret::{ConcealSecret, Region};

fn main() -> Result<(), onetimesecret::Error> {
    let client = Client::builder().region(Region::Eu).build()?;
    let created = client
        .secrets()
        .conceal(ConcealSecret::new("hello from the blocking client"))?;
    println!("{}", created.share_url);
    Ok(())
}
