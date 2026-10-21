//! Lifecycle states.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

macro_rules! string_enum {
    (
        $(#[$meta:meta])*
        $name:ident { $( $(#[$vmeta:meta])* $variant:ident => $text:literal ),+ $(,)? }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        #[non_exhaustive]
        pub enum $name {
            $( $(#[$vmeta])* $variant, )+
            /// A value this SDK does not know. The server is newer than the
            /// SDK; the raw text is kept so nothing is lost.
            Unknown(String),
        }

        impl $name {
            /// The wire representation.
            pub fn as_str(&self) -> &str {
                match self {
                    $( Self::$variant => $text, )+
                    Self::Unknown(s) => s,
                }
            }

            fn from_text(s: &str) -> Self {
                match s {
                    $( $text => Self::$variant, )+
                    other => Self::Unknown(other.to_owned()),
                }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let s = String::deserialize(deserializer)?;
                Ok(Self::from_text(&s))
            }
        }
    };
}

string_enum! {
    /// Where a secret is in its life.
    SecretState {
        /// Created; nobody has looked at it.
        New => "new",
        /// Someone opened the link but has not confirmed the reveal.
        Previewed => "previewed",
        /// The value was revealed. It is gone from the server.
        Revealed => "revealed",
        /// The creator burned it before anyone revealed it. Gone.
        Burned => "burned",
    }
}

string_enum! {
    /// Where a receipt is in its life. Mirrors the secret's state and adds
    /// what the receipt alone can know.
    ReceiptState {
        /// Created; the secret has not been looked at.
        New => "new",
        /// The link has been handed out (recorded by the web UI).
        Shared => "shared",
        /// The link was opened but not yet revealed.
        Previewed => "previewed",
        /// The secret was revealed.
        Revealed => "revealed",
        /// The secret was burned.
        Burned => "burned",
        /// The secret's TTL ran out.
        Expired => "expired",
        /// The secret is gone but the receipt does not know why.
        Orphaned => "orphaned",
    }
}

string_enum! {
    /// How a secret's value came to be.
    SecretKind {
        /// The creator supplied the value.
        Conceal => "conceal",
        /// The server generated the value.
        Generate => "generate",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_and_unknown_round_trip() {
        let s: SecretState = serde_json::from_str("\"revealed\"").unwrap();
        assert_eq!(s, SecretState::Revealed);
        assert_eq!(serde_json::to_string(&s).unwrap(), "\"revealed\"");

        let s: ReceiptState = serde_json::from_str("\"vaporised\"").unwrap();
        assert_eq!(s, ReceiptState::Unknown("vaporised".into()));
        assert_eq!(s.to_string(), "vaporised");
        assert_eq!(serde_json::to_string(&s).unwrap(), "\"vaporised\"");
    }
}
