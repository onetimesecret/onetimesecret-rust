//! Official Rust SDK for the [Onetime Secret](https://onetimesecret.com) API.
//!
//! Onetime Secret shares a secret through a link that works exactly once.
//! This crate covers the public v2 API: concealing and generating secrets,
//! revealing them, checking their status, and working with the receipts a
//! creator keeps.
//!
//! # Quick start
//!
//! ```no_run
//! use std::time::Duration;
//!
//! use onetimesecret::{Client, ConcealSecret, Credentials, Region};
//!
//! # async fn run() -> Result<(), onetimesecret::Error> {
//! let client = Client::builder()
//!     .region(Region::Eu)
//!     .credentials(Credentials::new("ur1abc23def", "your-api-token"))
//!     .build()?;
//!
//! let created = client
//!     .secrets()
//!     .conceal(
//!         ConcealSecret::new("the launch codes")
//!             .ttl(Duration::from_secs(3600))
//!             .passphrase("hunter2"),
//!     )
//!     .await?;
//!
//! println!("share this link once: {}", created.share_url);
//! println!("keep this receipt: {}", created.receipt.identifier);
//! # Ok(())
//! # }
//! ```
//!
//! The client can also be configured from the environment with
//! [`Client::from_env`], which reads `ONETIME_BASE_URL`,
//! `ONETIME_CUSTOMER_EXTID` and `ONETIME_API_TOKEN`.
//!
//! # Anonymous use
//!
//! A client built without [`Credentials`] talks to the `/guest/` routes of
//! the API, which exist for exactly this purpose. Operations that need an
//! account (listing receipts, updating a memo) return an
//! [`ApiError`] with [`ApiErrorKind::Unauthorized`].
//!
//! # Secret material
//!
//! Secret values and passphrases are carried as [`secrecy::SecretString`]:
//! they are redacted from `Debug` output and zeroized when dropped. Call
//! [`secrecy::ExposeSecret::expose_secret`] when you need the plaintext.
//! Copies made by the HTTP stack are outside the SDK's control.
//!
//! # Features
//!
//! - `rustls` (default): TLS via rustls.
//! - `native-tls`: TLS via the platform's native library instead.
//! - `blocking`: a synchronous client at [`blocking`].

#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc(html_logo_url = "https://onetimesecret.com/favicon.ico")]

mod api;
mod client;
mod error;
mod protocol;
mod types;

#[cfg(feature = "blocking")]
#[cfg_attr(docsrs, doc(cfg(feature = "blocking")))]
pub mod blocking;

pub use secrecy;

pub use api::{Receipts, Secrets, System};
pub use client::{
    Client, ClientBuilder, Credentials, ENV_API_TOKEN, ENV_BASE_URL, ENV_CUSTOMER, Region,
};
pub use error::{ApiError, ApiErrorKind, Error, Result};
pub use types::{
    CharacterSets, ConcealSecret, CreatedSecret, GenerateSecret, GeneratedSecret, ParseIdError,
    Receipt, ReceiptId, ReceiptState, RevealedSecret, SecretId, SecretKind, SecretMetadata,
    SecretState, SecretStatus, SupportedLocales, SystemStatus, SystemVersion,
};

/// The crate version, sent in the `User-Agent` header.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
