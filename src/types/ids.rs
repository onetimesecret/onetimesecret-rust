//! Typed identifiers.
//!
//! A secret and its receipt have identifiers of the same shape, and passing
//! one where the other is expected is the easiest mistake to make with this
//! API. Distinct types keep the compiler on your side.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// An identifier was empty or contained characters an identifier never has.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid identifier: {0:?}")]
pub struct ParseIdError(String);

fn extract_segment<'a>(input: &'a str, markers: &[&str]) -> &'a str {
    let input = input.trim();
    for marker in markers {
        if let Some(index) = input.find(marker) {
            let rest = &input[index + marker.len()..];
            let end = rest
                .find(|c: char| !c.is_ascii_alphanumeric())
                .unwrap_or(rest.len());
            return &rest[..end];
        }
    }
    input
}

fn validate(id: &str) -> Result<(), ParseIdError> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err(ParseIdError(id.to_owned()));
    }
    Ok(())
}

macro_rules! id_type {
    (
        $(#[$meta:meta])*
        $name:ident, markers: [$($marker:literal),+]
    ) => {
        $(#[$meta])*
        #[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            /// Wrap an identifier as given. For something that may be a
            /// full link, use [`parse`](Self::parse) or `From<&str>`.
            pub fn new(id: impl Into<String>) -> Self {
                Self(id.into())
            }

            /// Parse an identifier from a bare id or from a link that
            /// contains one, rejecting anything that is not an identifier.
            pub fn parse(input: &str) -> Result<Self, ParseIdError> {
                let id = extract_segment(input, &[$($marker),+]);
                validate(id)?;
                Ok(Self(id.to_owned()))
            }

            /// The identifier as a string slice.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl FromStr for $name {
            type Err = ParseIdError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Self::parse(s)
            }
        }

        /// Lenient: a link is reduced to its identifier, anything else is
        /// taken as given and left for the server to judge.
        impl From<&str> for $name {
            fn from(input: &str) -> Self {
                Self(extract_segment(input, &[$($marker),+]).to_owned())
            }
        }

        impl From<String> for $name {
            fn from(input: String) -> Self {
                Self::from(input.as_str())
            }
        }

        impl From<&$name> for $name {
            fn from(id: &$name) -> Self {
                id.clone()
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({:?})", stringify!($name), self.0)
            }
        }
    };
}

id_type! {
    /// Identifies a secret: the part of a share link after `/secret/`.
    ///
    /// Anyone holding this can reveal the secret, once.
    SecretId, markers: ["/secret/"]
}

id_type! {
    /// Identifies a receipt: the creator's handle on a secret, the part of
    /// a receipt link after `/receipt/`.
    ///
    /// A receipt can show a secret's state and burn it; it cannot reveal
    /// the value.
    ReceiptId, markers: ["/receipt/", "/private/"]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_ids_pass_through() {
        assert_eq!(SecretId::from("abc123").as_str(), "abc123");
        assert_eq!(ReceiptId::parse("abc123").unwrap().as_str(), "abc123");
    }

    #[test]
    fn links_are_reduced_to_their_id() {
        let id = SecretId::parse("https://eu.onetimesecret.com/secret/lj85fzdd?x=1").unwrap();
        assert_eq!(id.as_str(), "lj85fzdd");
        let id: ReceiptId = "https://eu.onetimesecret.com/receipt/5e60wem0/burn".into();
        assert_eq!(id.as_str(), "5e60wem0");
        let id: ReceiptId = "https://example.com/private/5e60wem0".into();
        assert_eq!(id.as_str(), "5e60wem0");
    }

    #[test]
    fn a_receipt_link_is_not_a_secret_id() {
        // The marker is wrong, so the whole string stays and validation
        // rejects it.
        assert!(SecretId::parse("https://eu.onetimesecret.com/receipt/5e60wem0").is_err());
        assert!(SecretId::parse("").is_err());
        assert!(SecretId::parse("has space").is_err());
    }

    #[test]
    fn debug_names_the_type() {
        assert_eq!(format!("{:?}", SecretId::new("k")), "SecretId(\"k\")");
    }

    #[test]
    fn serde_is_transparent() {
        let id: SecretId = serde_json::from_str("\"k3y\"").unwrap();
        assert_eq!(id.as_str(), "k3y");
        assert_eq!(serde_json::to_string(&id).unwrap(), "\"k3y\"");
    }
}
