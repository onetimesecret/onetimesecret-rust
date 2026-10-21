//! Secrets: the recipient's side.

use std::time::{Duration, SystemTime};

use secrecy::SecretString;
use serde::Deserialize;
use url::Url;

use super::wire::{flag, seconds, timestamp};
use super::{Receipt, SecretId, SecretState};

/// Everything about a secret except its value.
#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
#[allow(clippy::struct_excessive_bools)] // they mirror the wire format
pub struct SecretMetadata {
    /// The secret's identifier.
    pub identifier: SecretId,
    /// A short prefix of the identifier, for display.
    pub shortid: String,
    /// The secret's state.
    pub state: SecretState,
    /// Whether a passphrase is needed to reveal it.
    #[serde(default, deserialize_with = "flag::lenient")]
    pub has_passphrase: bool,
    /// Whether this secret is an account-verification token rather than
    /// user content.
    #[serde(default, deserialize_with = "flag::lenient")]
    pub verification: bool,
    /// How the value is meant to be displayed, e.g. `"text"`.
    #[serde(default)]
    pub display_type: Option<String>,
    /// The time to live requested at creation.
    #[serde(default, deserialize_with = "seconds::opt")]
    pub secret_ttl: Option<Duration>,
    /// The lifespan granted by the server.
    #[serde(default, deserialize_with = "seconds::opt")]
    pub lifespan: Option<Duration>,
    /// When the secret was created.
    #[serde(deserialize_with = "timestamp::required")]
    pub created: SystemTime,
    /// When the secret last changed.
    #[serde(deserialize_with = "timestamp::required")]
    pub updated: SystemTime,
    /// The link was opened.
    #[serde(default, deserialize_with = "flag::lenient")]
    pub is_previewed: bool,
    /// The value was revealed.
    #[serde(default, deserialize_with = "flag::lenient")]
    pub is_revealed: bool,
}

/// A secret's current state and remaining lifetime.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct SecretStatus {
    /// The secret, while the server holds it. `None` when the server does
    /// not know the identifier: the secret never existed, or it has been
    /// revealed, burned, or expired. The server does not say which.
    pub secret: Option<SecretMetadata>,
    /// How long until the secret expires, when the server reports it.
    pub remaining: Option<Duration>,
}

impl SecretStatus {
    /// `true` while the server still holds the secret.
    pub fn is_known(&self) -> bool {
        self.secret.is_some()
    }

    /// The secret's state, while the server still holds it. `None` once
    /// it is gone: the server does not say whether it was revealed,
    /// burned or expired, so there is no state to report.
    ///
    /// ```
    /// # use onetimesecret::{SecretState, SecretStatus};
    /// # fn check(status: &SecretStatus) {
    /// if status.state() == Some(&SecretState::New) {
    ///     println!("nobody has looked at it yet");
    /// }
    /// # }
    /// ```
    pub fn state(&self) -> Option<&SecretState> {
        self.secret.as_ref().map(|s| &s.state)
    }
}

/// The outcome of a reveal: the value, and the secret as it was.
///
/// The value is gone from the server the moment this is returned.
#[derive(Debug)]
#[non_exhaustive]
pub struct RevealedSecret {
    /// The secret's value.
    pub value: SecretString,
    /// The secret's metadata at the time of the reveal.
    pub secret: SecretMetadata,
}

/// The outcome of concealing or generating a secret.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct CreatedSecret {
    /// The creator's receipt. Keep its identifier to check on or burn the
    /// secret later.
    pub receipt: Receipt,
    /// The secret, without its value.
    pub secret: SecretMetadata,
    /// The domain the link lives on; `None` for the server's own.
    pub share_domain: Option<String>,
    /// The link to hand to the recipient.
    pub share_url: Url,
}

// ---- wire records -------------------------------------------------------

#[derive(Deserialize)]
pub(crate) struct SecretStatusRecord {
    /// Either a full secret record or `{"state": "unknown"}`; see
    /// [`into_status`](Self::into_status).
    pub(crate) record: serde_json::Value,
    #[serde(default)]
    pub(crate) details: SecretStatusDetails,
}

#[derive(Default, Deserialize)]
pub(crate) struct SecretStatusDetails {
    #[serde(default, deserialize_with = "seconds::opt")]
    pub(crate) current_expiration: Option<Duration>,
}

impl SecretStatusRecord {
    /// The server answers `{"record": {"state": "unknown"}}`, with HTTP
    /// 200, for a secret it does not hold. Anything else is a secret.
    pub(crate) fn into_status(self) -> Result<SecretStatus, serde_json::Error> {
        let unknown = self.record.get("identifier").is_none()
            && self.record.get("state").and_then(serde_json::Value::as_str) == Some("unknown");
        let secret = if unknown {
            None
        } else {
            Some(serde_json::from_value(self.record)?)
        };
        Ok(SecretStatus {
            secret,
            remaining: self.details.current_expiration,
        })
    }
}

#[derive(Deserialize)]
pub(crate) struct RevealedSecretRecord {
    #[serde(default)]
    pub(crate) secret_value: Option<SecretString>,
    #[serde(flatten)]
    pub(crate) secret: SecretMetadata,
}

#[derive(Deserialize)]
pub(crate) struct CreatedSecretRecord {
    pub(crate) receipt: Receipt,
    pub(crate) secret: SecretMetadata,
    #[serde(default)]
    pub(crate) share_domain: Option<String>,
}

impl CreatedSecretRecord {
    /// The share link: on the custom domain when the server reports one,
    /// otherwise on the server the client is configured against.
    pub(crate) fn into_created(self, base_url: &Url) -> Result<CreatedSecret, url::ParseError> {
        // `base_url` always ends in `/`, so a relative join keeps any path
        // prefix a self-hosted install may live under.
        let path = format!("secret/{}", self.secret.identifier);
        let share_url = match self.share_domain.as_deref() {
            Some(domain) if !domain.is_empty() => Url::parse(&format!("https://{domain}/{path}"))?,
            _ => base_url.join(&path)?,
        };
        Ok(CreatedSecret {
            receipt: self.receipt,
            secret: self.secret,
            share_domain: self.share_domain,
            share_url,
        })
    }
}
