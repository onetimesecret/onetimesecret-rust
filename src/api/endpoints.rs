//! Every API operation as a pure value: the request to send and how to
//! read the reply. The async and blocking resource handles are thin
//! wrappers over these.

use secrecy::{ExposeSecret, SecretString};
use serde::Serialize;
use url::Url;

use crate::error::{Error, Result};
use crate::protocol::{self, Record, Records, Request, Response, path_segment};
use crate::types::requests::CreateBody;
use crate::types::{
    ConcealSecret, CreatedSecret, CreatedSecretRecord, GenerateSecret, Receipt, ReceiptId,
    RevealedSecret, RevealedSecretRecord, SecretId, SecretMetadata, SecretStatus,
    SecretStatusRecord, SupportedLocales, SystemStatus, SystemVersion,
};

/// What the operations need to know about the client.
#[derive(Debug, Clone)]
pub(crate) struct Context {
    /// Always ends in `/`.
    pub(crate) base_url: Url,
    /// Use the `/guest/` routes (no credentials configured).
    pub(crate) guest: bool,
}

impl Context {
    /// The host of the base URL, used as the share domain when a request
    /// does not name one.
    fn default_share_domain(&self) -> &str {
        self.base_url.host_str().unwrap_or_default()
    }

    /// `guest/<path>` for anonymous clients, `<path>` otherwise.
    fn route(&self, path: &str) -> String {
        if self.guest {
            format!("guest/{path}")
        } else {
            path.to_owned()
        }
    }
}

/// A request and the function that interprets its response.
pub(crate) struct Endpoint<T> {
    pub(crate) request: Request,
    pub(crate) decode: fn(&Context, &Response) -> Result<T>,
}

impl<T> Endpoint<T> {
    fn new(request: Request, decode: fn(&Context, &Response) -> Result<T>) -> Self {
        Self { request, decode }
    }
}

fn decode_record<T: serde::de::DeserializeOwned>(_: &Context, response: &Response) -> Result<T> {
    protocol::parse::<Record<T>>(response).map(|r| r.record)
}

fn decode_records<T: serde::de::DeserializeOwned>(
    _: &Context,
    response: &Response,
) -> Result<Vec<T>> {
    protocol::parse::<Records<T>>(response).map(|r| r.records)
}

fn decode_plain<T: serde::de::DeserializeOwned>(_: &Context, response: &Response) -> Result<T> {
    protocol::parse(response)
}

fn decode_created(ctx: &Context, response: &Response) -> Result<CreatedSecret> {
    let record: CreatedSecretRecord = decode_record(ctx, response)?;
    record
        .into_created(&ctx.base_url)
        .map_err(|e| Error::Decode {
            status: response.status,
            message: format!("could not build the share link: {e}"),
        })
}

fn decode_revealed(ctx: &Context, response: &Response) -> Result<RevealedSecret> {
    let record: RevealedSecretRecord = decode_record(ctx, response)?;
    let value = record.secret_value.ok_or_else(|| Error::Decode {
        status: response.status,
        message: "the server did not return the secret value".into(),
    })?;
    Ok(RevealedSecret {
        value,
        secret: record.secret,
    })
}

fn decode_status(ctx: &Context, response: &Response) -> Result<SecretStatus> {
    let record: SecretStatusRecord = decode_plain(ctx, response)?;
    record.into_status().map_err(|e| Error::Decode {
        status: response.status,
        message: e.to_string(),
    })
}

// ---- secrets ------------------------------------------------------------

pub(crate) fn conceal(ctx: &Context, req: &ConcealSecret) -> Result<Endpoint<CreatedSecret>> {
    let body = CreateBody {
        kind: "conceal",
        secret: Some(&req.secret),
        options: &req.options,
        length: None,
        character_sets: None,
        default_share_domain: ctx.default_share_domain(),
    };
    Ok(Endpoint::new(
        Request::post(ctx.route("secret/conceal"), &body)?,
        decode_created,
    ))
}

pub(crate) fn generate(ctx: &Context, req: &GenerateSecret) -> Result<Endpoint<CreatedSecret>> {
    let body = CreateBody {
        kind: "generate",
        secret: None,
        options: &req.options,
        length: req.length,
        character_sets: req.character_sets.as_ref(),
        default_share_domain: ctx.default_share_domain(),
    };
    Ok(Endpoint::new(
        Request::post(ctx.route("secret/generate"), &body)?,
        decode_created,
    ))
}

pub(crate) fn show_secret(ctx: &Context, id: &SecretId) -> Endpoint<SecretMetadata> {
    Endpoint::new(
        Request::get(ctx.route(&format!("secret/{}", path_segment(id.as_str())))),
        decode_record,
    )
}

pub(crate) fn secret_status(_: &Context, id: &SecretId) -> Endpoint<SecretStatus> {
    Endpoint::new(
        Request::get(format!("secret/{}/status", path_segment(id.as_str()))),
        decode_status,
    )
}

pub(crate) fn secret_status_many(
    _: &Context,
    ids: &[SecretId],
) -> Result<Endpoint<Vec<SecretMetadata>>> {
    #[derive(Serialize)]
    struct Body<'a> {
        identifiers: &'a [SecretId],
    }
    Ok(Endpoint::new(
        Request::post("secret/status", &Body { identifiers: ids })?,
        decode_records,
    ))
}

/// The body of reveal and burn: always confirms, carries a passphrase
/// when one was given.
struct Confirm<'a> {
    passphrase: Option<&'a SecretString>,
}

impl Serialize for Confirm<'_> {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("confirm", 2)?;
        s.serialize_field("continue", &true)?;
        if let Some(p) = self.passphrase {
            s.serialize_field("passphrase", p.expose_secret())?;
        }
        s.end()
    }
}

pub(crate) fn reveal(
    ctx: &Context,
    id: &SecretId,
    passphrase: Option<&SecretString>,
) -> Result<Endpoint<RevealedSecret>> {
    Ok(Endpoint::new(
        Request::post(
            ctx.route(&format!("secret/{}/reveal", path_segment(id.as_str()))),
            &Confirm { passphrase },
        )?,
        decode_revealed,
    ))
}

// ---- receipts -----------------------------------------------------------

pub(crate) fn show_receipt(ctx: &Context, id: &ReceiptId) -> Endpoint<Receipt> {
    Endpoint::new(
        Request::get(ctx.route(&format!("receipt/{}", path_segment(id.as_str())))),
        decode_record,
    )
}

pub(crate) fn recent_receipts(_: &Context) -> Endpoint<Vec<Receipt>> {
    Endpoint::new(Request::get("receipt/recent"), decode_records)
}

pub(crate) fn burn(
    ctx: &Context,
    id: &ReceiptId,
    passphrase: Option<&SecretString>,
) -> Result<Endpoint<Receipt>> {
    Ok(Endpoint::new(
        Request::post(
            ctx.route(&format!("receipt/{}/burn", path_segment(id.as_str()))),
            &Confirm { passphrase },
        )?,
        decode_record,
    ))
}

pub(crate) fn update_memo(_: &Context, id: &ReceiptId, memo: &str) -> Result<Endpoint<Receipt>> {
    #[derive(Serialize)]
    struct Body<'a> {
        memo: &'a str,
    }
    Ok(Endpoint::new(
        Request::patch(
            format!("receipt/{}", path_segment(id.as_str())),
            &Body { memo },
        )?,
        decode_record,
    ))
}

// ---- system -------------------------------------------------------------

pub(crate) fn system_status(_: &Context) -> Endpoint<SystemStatus> {
    Endpoint::new(Request::get("status"), decode_plain)
}

pub(crate) fn system_version(_: &Context) -> Endpoint<SystemVersion> {
    Endpoint::new(Request::get("version"), decode_plain)
}

pub(crate) fn supported_locales(_: &Context) -> Endpoint<SupportedLocales> {
    Endpoint::new(Request::get("supported-locales"), decode_plain)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(guest: bool) -> Context {
        Context {
            base_url: Url::parse("https://eu.onetimesecret.com/").unwrap(),
            guest,
        }
    }

    #[test]
    fn anonymous_clients_use_guest_routes_where_they_exist() {
        let id = SecretId::new("k");
        assert_eq!(show_secret(&ctx(true), &id).request.path, "guest/secret/k");
        assert_eq!(show_secret(&ctx(false), &id).request.path, "secret/k");
        // Status has no guest variant.
        assert_eq!(
            secret_status(&ctx(true), &id).request.path,
            "secret/k/status"
        );
        // Neither do the account-only operations.
        assert_eq!(recent_receipts(&ctx(true)).request.path, "receipt/recent");
    }

    #[test]
    fn identifiers_cannot_escape_their_path_segment() {
        // `From<&str>` is lenient, so these reach the endpoint as given.
        let id = SecretId::from("../../x");
        assert_eq!(
            reveal(&ctx(false), &id, None).unwrap().request.path,
            "secret/..%2F..%2Fx/reveal"
        );
        let id = ReceiptId::from("r?burn=1#frag");
        assert_eq!(
            show_receipt(&ctx(false), &id).request.path,
            "receipt/r%3Fburn=1%23frag"
        );
        // Ordinary identifiers are untouched.
        let id = SecretId::from("lj85fzdd");
        assert_eq!(
            secret_status(&ctx(false), &id).request.path,
            "secret/lj85fzdd/status"
        );
    }

    #[test]
    fn conceal_fills_in_the_share_domain_from_the_base_url() {
        let ep = conceal(&ctx(false), &ConcealSecret::new("s")).unwrap();
        let body: serde_json::Value =
            serde_json::from_slice(ep.request.body.as_ref().unwrap()).unwrap();
        assert_eq!(body["secret"]["share_domain"], "eu.onetimesecret.com");
        assert_eq!(body["secret"]["kind"], "conceal");
    }

    #[test]
    fn reveal_always_confirms() {
        let ep = reveal(&ctx(true), &SecretId::new("k"), None).unwrap();
        assert_eq!(ep.request.path, "guest/secret/k/reveal");
        let body: serde_json::Value =
            serde_json::from_slice(ep.request.body.as_ref().unwrap()).unwrap();
        assert_eq!(body, serde_json::json!({"continue": true}));

        let pw = SecretString::from("pw");
        let ep = burn(&ctx(false), &ReceiptId::new("r"), Some(&pw)).unwrap();
        let body: serde_json::Value =
            serde_json::from_slice(ep.request.body.as_ref().unwrap()).unwrap();
        assert_eq!(
            body,
            serde_json::json!({"continue": true, "passphrase": "pw"})
        );
    }
}
