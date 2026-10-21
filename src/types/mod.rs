//! Data types exchanged with the API.
//!
//! Response types are deserialized liberally: unknown fields are ignored,
//! and scalar fields accept the representations the server has used over
//! time (numbers or numeric strings, booleans or `"true"`/`"false"`), so a
//! server a little newer or older than this SDK still parses.

mod ids;
mod receipt;
pub(crate) mod requests;
mod secret;
mod state;
mod system;
pub(crate) mod wire;

pub use ids::{ParseIdError, ReceiptId, SecretId};
pub use receipt::Receipt;
pub(crate) use receipt::ReceiptEnvelope;
pub use requests::{CharacterSets, ConcealSecret, GenerateSecret};
pub use secret::{CreatedSecret, RevealedSecret, SecretMetadata, SecretStatus};
pub use state::{ReceiptState, SecretKind, SecretState};
pub use system::{SupportedLocales, SystemStatus, SystemVersion};

pub(crate) use secret::{CreatedSecretRecord, RevealedSecretRecord, SecretStatusRecord};
