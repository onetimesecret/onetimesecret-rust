//! Request builders for creating secrets.

use std::time::Duration;

use secrecy::{ExposeSecret, SecretString};
use serde::Serialize;
use serde::ser::{SerializeStruct, Serializer};

/// Share a value you already have.
///
/// ```
/// use std::time::Duration;
/// use onetimesecret::ConcealSecret;
///
/// let request = ConcealSecret::new("the launch codes")
///     .ttl(Duration::from_secs(3600))
///     .passphrase("hunter2")
///     .recipient("ops@example.com");
/// ```
#[derive(Clone)]
#[must_use]
pub struct ConcealSecret {
    pub(crate) secret: SecretString,
    pub(crate) options: CreateOptions,
}

impl std::fmt::Debug for ConcealSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConcealSecret")
            .field("secret", &"[REDACTED]")
            .field("options", &self.options)
            .finish()
    }
}

impl ConcealSecret {
    /// A request to conceal `secret`.
    pub fn new(secret: impl Into<SecretString>) -> Self {
        Self {
            secret: secret.into(),
            options: CreateOptions::default(),
        }
    }

    /// How long the secret lives before the server destroys it unread.
    /// Rounded down to whole seconds. The server caps this by plan.
    pub fn ttl(mut self, ttl: Duration) -> Self {
        self.options.ttl = Some(ttl);
        self
    }

    /// Require a passphrase to reveal the secret.
    pub fn passphrase(mut self, passphrase: impl Into<SecretString>) -> Self {
        self.options.passphrase = Some(passphrase.into());
        self
    }

    /// Have the server email the share link to this address.
    pub fn recipient(mut self, email: impl Into<String>) -> Self {
        self.options.recipient = Some(email.into());
        self
    }

    /// Put the share link on one of your custom domains instead of the
    /// server's own.
    pub fn share_domain(mut self, domain: impl Into<String>) -> Self {
        self.options.share_domain = Some(domain.into());
        self
    }
}

/// Have the server make up a value.
///
/// ```
/// use onetimesecret::{CharacterSets, GenerateSecret};
///
/// let request = GenerateSecret::new()
///     .length(32)
///     .character_sets(CharacterSets::default().symbols(false));
/// ```
#[derive(Debug, Clone)]
#[must_use]
pub struct GenerateSecret {
    pub(crate) options: CreateOptions,
    pub(crate) length: Option<u32>,
    pub(crate) character_sets: Option<CharacterSets>,
    pub(crate) claim: bool,
}

impl Default for GenerateSecret {
    fn default() -> Self {
        Self {
            options: CreateOptions::default(),
            length: None,
            character_sets: None,
            claim: true,
        }
    }
}

impl GenerateSecret {
    /// A request with the server's defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether `generate` fetches the receipt right after creation to
    /// claim the generated value. On by default.
    ///
    /// The server discloses the value exactly once, on the first fetch of
    /// the receipt, and only for a short time after creation (60 seconds
    /// by default). Pass `false` to make that fetch yourself with
    /// [`Receipts::show`](crate::Receipts::show); its
    /// [`Receipt::secret_value`](crate::Receipt::secret_value) then carries
    /// the value. One request fewer, and the value goes to whoever fetches
    /// first, so be the first.
    pub fn claim_value(mut self, claim: bool) -> Self {
        self.claim = claim;
        self
    }

    /// How long the secret lives before the server destroys it unread.
    pub fn ttl(mut self, ttl: Duration) -> Self {
        self.options.ttl = Some(ttl);
        self
    }

    /// Require a passphrase to reveal the secret.
    pub fn passphrase(mut self, passphrase: impl Into<SecretString>) -> Self {
        self.options.passphrase = Some(passphrase.into());
        self
    }

    /// Have the server email the share link to this address.
    pub fn recipient(mut self, email: impl Into<String>) -> Self {
        self.options.recipient = Some(email.into());
        self
    }

    /// Put the share link on one of your custom domains.
    pub fn share_domain(mut self, domain: impl Into<String>) -> Self {
        self.options.share_domain = Some(domain.into());
        self
    }

    /// Length of the generated value. The server accepts 4 to 128.
    pub fn length(mut self, length: u32) -> Self {
        self.length = Some(length);
        self
    }

    /// Which characters the generated value may use.
    pub fn character_sets(mut self, sets: CharacterSets) -> Self {
        self.character_sets = Some(sets);
        self
    }
}

/// Character classes for [`GenerateSecret`]. Unset fields leave the
/// server's default in place.
#[derive(Debug, Clone, Default, Serialize)]
#[non_exhaustive]
#[must_use]
pub struct CharacterSets {
    #[serde(skip_serializing_if = "Option::is_none")]
    uppercase: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lowercase: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    numbers: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    symbols: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    exclude_ambiguous: Option<bool>,
}

impl CharacterSets {
    /// `A-Z`.
    pub fn uppercase(mut self, on: bool) -> Self {
        self.uppercase = Some(on);
        self
    }

    /// `a-z`.
    pub fn lowercase(mut self, on: bool) -> Self {
        self.lowercase = Some(on);
        self
    }

    /// `0-9`.
    pub fn numbers(mut self, on: bool) -> Self {
        self.numbers = Some(on);
        self
    }

    /// Punctuation.
    pub fn symbols(mut self, on: bool) -> Self {
        self.symbols = Some(on);
        self
    }

    /// Leave out look-alikes such as `0`/`O` and `1`/`l`.
    pub fn exclude_ambiguous(mut self, on: bool) -> Self {
        self.exclude_ambiguous = Some(on);
        self
    }
}

/// Options shared by conceal and generate.
#[derive(Clone, Default)]
pub(crate) struct CreateOptions {
    pub(crate) ttl: Option<Duration>,
    pub(crate) passphrase: Option<SecretString>,
    pub(crate) recipient: Option<String>,
    pub(crate) share_domain: Option<String>,
}

impl std::fmt::Debug for CreateOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CreateOptions")
            .field("ttl", &self.ttl)
            .field(
                "passphrase",
                &self.passphrase.as_ref().map(|_| "[REDACTED]"),
            )
            .field("recipient", &self.recipient)
            .field("share_domain", &self.share_domain)
            .finish()
    }
}

/// The body of a create request, as the v2 API wants it: nested under
/// `secret`, with `kind` and a mandatory `share_domain`.
pub(crate) struct CreateBody<'a> {
    pub(crate) kind: &'static str,
    pub(crate) secret: Option<&'a SecretString>,
    pub(crate) options: &'a CreateOptions,
    pub(crate) length: Option<u32>,
    pub(crate) character_sets: Option<&'a CharacterSets>,
    /// Used when the request names no share domain.
    pub(crate) default_share_domain: &'a str,
}

impl Serialize for CreateBody<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        struct Inner<'a>(&'a CreateBody<'a>);

        impl Serialize for Inner<'_> {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                let body = self.0;
                let opts = body.options;
                let mut s = serializer.serialize_struct("secret", 8)?;
                s.serialize_field("kind", body.kind)?;
                s.serialize_field(
                    "share_domain",
                    opts.share_domain
                        .as_deref()
                        .unwrap_or(body.default_share_domain),
                )?;
                if let Some(secret) = body.secret {
                    s.serialize_field("secret", secret.expose_secret())?;
                }
                if let Some(ttl) = opts.ttl {
                    s.serialize_field("ttl", &ttl.as_secs())?;
                }
                if let Some(passphrase) = &opts.passphrase {
                    s.serialize_field("passphrase", passphrase.expose_secret())?;
                }
                if let Some(recipient) = &opts.recipient {
                    s.serialize_field("recipient", recipient)?;
                }
                if let Some(length) = body.length {
                    s.serialize_field("length", &length)?;
                }
                if let Some(sets) = body.character_sets {
                    s.serialize_field("character_sets", sets)?;
                }
                s.end()
            }
        }

        let mut outer = serializer.serialize_struct("body", 1)?;
        outer.serialize_field("secret", &Inner(self))?;
        outer.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conceal_serializes_to_the_v2_shape() {
        let req = ConcealSecret::new("hunter2")
            .ttl(Duration::from_secs(3600))
            .passphrase("pw")
            .recipient("a@example.com");
        let body = CreateBody {
            kind: "conceal",
            secret: Some(&req.secret),
            options: &req.options,
            length: None,
            character_sets: None,
            default_share_domain: "eu.onetimesecret.com",
        };
        let json = serde_json::to_value(&body).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "secret": {
                    "kind": "conceal",
                    "share_domain": "eu.onetimesecret.com",
                    "secret": "hunter2",
                    "ttl": 3600,
                    "passphrase": "pw",
                    "recipient": "a@example.com",
                }
            })
        );
    }

    #[test]
    fn generate_omits_what_is_unset_and_honours_share_domain() {
        let req = GenerateSecret::new()
            .share_domain("secrets.example.com")
            .length(16)
            .character_sets(CharacterSets::default().symbols(false));
        let body = CreateBody {
            kind: "generate",
            secret: None,
            options: &req.options,
            length: req.length,
            character_sets: req.character_sets.as_ref(),
            default_share_domain: "eu.onetimesecret.com",
        };
        let json = serde_json::to_value(&body).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "secret": {
                    "kind": "generate",
                    "share_domain": "secrets.example.com",
                    "length": 16,
                    "character_sets": { "symbols": false },
                }
            })
        );
    }

    #[test]
    fn debug_never_prints_secrets() {
        let req = ConcealSecret::new("hunter2").passphrase("pw");
        let text = format!("{req:?}");
        assert!(!text.contains("hunter2"));
        assert!(!text.contains("pw\""));
        assert!(text.contains("REDACTED"));
    }
}
