//! Receipts: the creator's side of a secret.

use std::time::{Duration, SystemTime};

use secrecy::SecretString;
use serde::Deserialize;
use url::Url;

use super::wire::{flag, recipients, seconds, timestamp};
use super::{ReceiptId, ReceiptState, SecretId, SecretKind, SecretState};

/// What the creator keeps after making a secret.
///
/// A receipt reports the secret's state, can burn the secret, and carries
/// the links the web UI would show. Receipts returned alongside a freshly
/// created secret omit the link and expiration fields; fetch the receipt
/// to get them.
///
/// A receipt does not carry the secret's value, with one exception: for a
/// server-generated secret the value is disclosed once, on the first
/// fetch of the receipt, in [`secret_value`](Self::secret_value). See
/// [`Secrets::generate`](crate::Secrets::generate).
#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
#[allow(clippy::struct_excessive_bools)] // they mirror the wire format
pub struct Receipt {
    /// The receipt's identifier.
    pub identifier: ReceiptId,
    /// A short prefix of the identifier, for display.
    pub shortid: String,
    /// The receipt's state.
    pub state: ReceiptState,
    /// Whether the value was supplied or generated.
    #[serde(default)]
    pub kind: Option<SecretKind>,
    /// The secret's identifier, while the secret exists.
    #[serde(default)]
    pub secret_identifier: Option<SecretId>,
    /// A short prefix of the secret's identifier, for display.
    #[serde(default)]
    pub secret_shortid: Option<String>,
    /// The secret's state, when the server reports it alongside.
    #[serde(default)]
    pub secret_state: Option<SecretState>,
    /// The customer who created the secret; `None` for anonymous secrets.
    #[serde(default)]
    pub custid: Option<String>,
    /// The owning account's identifier; `None` for anonymous secrets.
    #[serde(default)]
    pub owner_id: Option<String>,
    /// The secret's time to live from creation.
    #[serde(deserialize_with = "seconds::required")]
    pub secret_ttl: Duration,
    /// The receipt's time to live from creation.
    #[serde(deserialize_with = "seconds::required")]
    pub receipt_ttl: Duration,
    /// The secret's lifespan as granted by the server.
    #[serde(deserialize_with = "seconds::required")]
    pub lifespan: Duration,
    /// The domain the share link lives on; `None` for the server's own.
    #[serde(default)]
    pub share_domain: Option<String>,
    /// Email recipients the server notified.
    #[serde(default, deserialize_with = "recipients::lenient")]
    pub recipients: Vec<String>,
    /// A note the creator attached, visible only to them.
    #[serde(default)]
    pub memo: Option<String>,
    /// Whether a passphrase gates the reveal.
    #[serde(default, deserialize_with = "flag::lenient")]
    pub has_passphrase: bool,
    /// When the receipt was created.
    #[serde(deserialize_with = "timestamp::required")]
    pub created: SystemTime,
    /// When the receipt last changed.
    #[serde(deserialize_with = "timestamp::required")]
    pub updated: SystemTime,
    /// When the link was recorded as shared, if it was.
    #[serde(default, deserialize_with = "timestamp::opt")]
    pub shared: Option<SystemTime>,
    /// When the link was first opened, if it was.
    #[serde(default, deserialize_with = "timestamp::opt")]
    pub previewed: Option<SystemTime>,
    /// When the value was revealed, if it was.
    #[serde(default, deserialize_with = "timestamp::opt")]
    pub revealed: Option<SystemTime>,
    /// When the secret was burned, if it was.
    #[serde(default, deserialize_with = "timestamp::opt")]
    pub burned: Option<SystemTime>,
    /// The link was opened.
    #[serde(default, deserialize_with = "flag::lenient")]
    pub is_previewed: bool,
    /// The value was revealed.
    #[serde(default, deserialize_with = "flag::lenient")]
    pub is_revealed: bool,
    /// The secret was burned.
    #[serde(default, deserialize_with = "flag::lenient")]
    pub is_burned: bool,
    /// The secret is gone, for any reason.
    #[serde(default, deserialize_with = "flag::lenient")]
    pub is_destroyed: bool,
    /// The secret's TTL ran out.
    #[serde(default, deserialize_with = "flag::lenient")]
    pub is_expired: bool,
    /// The secret is gone and the receipt does not know why.
    #[serde(default, deserialize_with = "flag::lenient")]
    pub is_orphaned: bool,
    /// A human phrase for the secret's lifetime, e.g. `"10 minutes"`.
    /// Only on fetched receipts.
    #[serde(default)]
    pub natural_expiration: Option<String>,
    /// When the secret expires. Only on fetched receipts.
    #[serde(default, deserialize_with = "timestamp::opt")]
    pub expiration: Option<SystemTime>,
    /// The share link. Only on fetched receipts, and empty of its
    /// identifier once the secret is gone.
    #[serde(default)]
    pub share_url: Option<Url>,
    /// The receipt's own link. Only on fetched receipts.
    #[serde(default)]
    pub receipt_url: Option<Url>,
    /// The web UI's burn page. Only on fetched receipts.
    #[serde(default)]
    pub burn_url: Option<Url>,
    /// A generated secret's value, disclosed by the server exactly once:
    /// on the first fetch of the receipt, while the secret is still `new`
    /// and within a short window of its creation (60 seconds by
    /// default). `None` on every other receipt and every later fetch.
    #[serde(skip)]
    pub secret_value: Option<SecretString>,
}

// ---- wire records -------------------------------------------------------

/// `GET /receipt/{id}`: the record plus the `details` the server adds for
/// the creator's eyes.
#[derive(Deserialize)]
pub(crate) struct ReceiptEnvelope {
    pub(crate) record: Receipt,
    #[serde(default)]
    pub(crate) details: ReceiptDetails,
}

#[derive(Default, Deserialize)]
pub(crate) struct ReceiptDetails {
    #[serde(default)]
    pub(crate) secret_value: Option<SecretString>,
}

impl Receipt {
    /// `true` while the secret can still be revealed.
    pub fn is_pending(&self) -> bool {
        !self.is_destroyed && !self.is_revealed && !self.is_burned && !self.is_expired
    }
}
