//! The wire protocol, free of any HTTP client.
//!
//! [`Request`] describes what to send and [`Response`] is what came back;
//! [`parse`] turns a response into a typed value or an [`Error`]. The
//! async and blocking clients differ only in how they move bytes.

use std::time::Duration;

use serde::Deserialize;
use serde::de::DeserializeOwned;
use zeroize::Zeroizing;

use crate::error::{ApiError, ApiErrorKind, Error};

/// The mount point of the API this SDK speaks.
pub(crate) const API_PREFIX: &str = "api/v2/";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Method {
    Get,
    Post,
    Patch,
}

/// An HTTP request to the API, relative to the base URL.
pub(crate) struct Request {
    pub(crate) method: Method,
    /// Relative to `API_PREFIX`, without a leading slash.
    pub(crate) path: String,
    /// A JSON body; wiped on drop because it may carry secret material.
    pub(crate) body: Option<Zeroizing<Vec<u8>>>,
}

impl Request {
    pub(crate) fn get(path: impl Into<String>) -> Self {
        Self {
            method: Method::Get,
            path: path.into(),
            body: None,
        }
    }

    pub(crate) fn post(
        path: impl Into<String>,
        body: &impl serde::Serialize,
    ) -> Result<Self, Error> {
        Self::with_body(Method::Post, path, body)
    }

    pub(crate) fn patch(
        path: impl Into<String>,
        body: &impl serde::Serialize,
    ) -> Result<Self, Error> {
        Self::with_body(Method::Patch, path, body)
    }

    fn with_body(
        method: Method,
        path: impl Into<String>,
        body: &impl serde::Serialize,
    ) -> Result<Self, Error> {
        // Straight to bytes: an intermediate `serde_json::Value` would hold
        // an unwiped copy of the secret.
        let mut bytes = Zeroizing::new(Vec::new());
        serde_json::to_writer(&mut *bytes, body).map_err(|e| Error::Config(e.to_string()))?;
        Ok(Self {
            method,
            path: path.into(),
            body: Some(bytes),
        })
    }
}

/// The parts of an HTTP response the protocol cares about.
pub(crate) struct Response {
    pub(crate) status: u16,
    pub(crate) body: Zeroizing<Vec<u8>>,
    /// From the `Retry-After` header, when present.
    pub(crate) retry_after: Option<Duration>,
}

/// Interpret a response: success bodies become `T`, error bodies become
/// [`ApiError`], and a success body that is not a `T` is [`Error::Decode`].
pub(crate) fn parse<T: DeserializeOwned>(response: &Response) -> Result<T, Error> {
    if !(200..300).contains(&response.status) {
        return Err(api_error(response).into());
    }
    serde_json::from_slice(&response.body).map_err(|e| Error::Decode {
        status: response.status,
        message: e.to_string(),
    })
}

/// Successful responses that wrap their payload: `{"record": …}`.
#[derive(Debug, Deserialize)]
pub(crate) struct Record<T> {
    pub(crate) record: T,
}

/// Successful responses that carry a list: `{"records": […]}`.
#[derive(Deserialize)]
pub(crate) struct Records<T> {
    #[serde(default = "Vec::new")]
    pub(crate) records: Vec<T>,
}

/// The fields error bodies have been seen to carry, all optional.
#[derive(Default, Deserialize)]
struct ErrorBody {
    error: Option<String>,
    message: Option<String>,
    error_type: Option<String>,
    code: Option<String>,
    field: Option<String>,
    error_key: Option<String>,
    entitlement: Option<String>,
    request_id: Option<String>,
    #[serde(default, deserialize_with = "crate::types::wire::seconds::opt")]
    retry_after: Option<Duration>,
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|v| !v.trim().is_empty())
}

/// The account requirement arrives in any of several slots depending on
/// the endpoint and server version: `error_type`, the `field` of a
/// `FormError`, the i18n `error_key`, or a `code` such as
/// `GUEST_CONCEAL_REQUIRES_ACCOUNT`.
fn mentions_account_required(value: Option<&str>) -> bool {
    let Some(value) = value else {
        return false;
    };
    let lower = value.to_ascii_lowercase();
    [
        "requires_account",
        "requires-account",
        "requiresaccount",
        "account_required",
        "account-required",
        "accountrequired",
    ]
    .iter()
    .any(|needle| {
        lower.match_indices(needle).any(|(index, _)| {
            let before = lower[..index].chars().next_back();
            let after = lower[index + needle.len()..].chars().next();
            !before.is_some_and(|c| c.is_ascii_alphanumeric())
                && !after.is_some_and(|c| c.is_ascii_alphanumeric())
        })
    })
}

fn api_error(response: &Response) -> ApiError {
    let json: Option<serde_json::Value> = serde_json::from_slice(&response.body).ok();
    let body: ErrorBody = json
        .clone()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();

    let account_required = [
        body.error_type.as_deref(),
        body.field.as_deref(),
        body.error_key.as_deref(),
        body.code.as_deref(),
    ]
    .into_iter()
    .any(mentions_account_required);

    let kind = if account_required {
        ApiErrorKind::AccountRequired
    } else {
        body.error_type
            .as_deref()
            .and_then(ApiErrorKind::from_error_type)
            .unwrap_or_else(|| ApiErrorKind::from_status(response.status))
    };

    let error = non_empty(body.error);
    let server_message = non_empty(body.message);
    let (message, detail) = match (error, server_message) {
        (Some(error), Some(message)) if error != message => (error, Some(message)),
        (Some(error), _) => (error, None),
        (None, Some(message)) => (message, None),
        (None, None) => (kind.default_message(response.status), None),
    };

    ApiError {
        status: response.status,
        kind,
        message,
        detail,
        error_type: body.error_type,
        code: body.code,
        field: body.field,
        error_key: body.error_key,
        entitlement: body.entitlement,
        request_id: body.request_id,
        retry_after: response.retry_after.or(body.retry_after),
        body: json,
    }
}

/// Parse a `Retry-After` header value given in seconds. HTTP-date forms are
/// not interpreted.
pub(crate) fn retry_after(value: &str) -> Option<Duration> {
    value.trim().parse::<u64>().ok().map(Duration::from_secs)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(status: u16, body: &str) -> Response {
        Response {
            status,
            body: Zeroizing::new(body.as_bytes().to_vec()),
            retry_after: None,
        }
    }

    #[test]
    fn success_parses_the_record() {
        let r = response(200, r#"{"record":{"x":1},"details":{}}"#);
        let parsed: Record<serde_json::Value> = parse(&r).unwrap();
        assert_eq!(parsed.record["x"], 1);
    }

    #[test]
    fn a_wrong_shape_is_a_decode_error_with_the_status() {
        let r = response(200, r#"{"records":[]}"#);
        match parse::<Record<serde_json::Value>>(&r).unwrap_err() {
            Error::Decode { status: 200, .. } => {}
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn errors_prefer_error_then_message_then_default() {
        let r = response(
            401,
            r#"{"error":"Authentication Required","message":"[CREDENTIALS_INVALID] Invalid credentials","code":"api_key_invalid"}"#,
        );
        let e = match parse::<()>(&r).unwrap_err() {
            Error::Api(e) => e,
            other => panic!("unexpected {other:?}"),
        };
        assert_eq!(e.kind, ApiErrorKind::Unauthorized);
        assert_eq!(e.message, "Authentication Required");
        assert_eq!(
            e.detail.as_deref(),
            Some("[CREDENTIALS_INVALID] Invalid credentials")
        );
        assert_eq!(e.code.as_deref(), Some("api_key_invalid"));
        assert_eq!(
            e.to_string(),
            "unauthorized (HTTP 401): Authentication Required: [CREDENTIALS_INVALID] Invalid credentials"
        );

        let r = response(
            404,
            r#"{"error":"identifier: nope","error_type":"RecordNotFound","request_id":"abc"}"#,
        );
        let e = parse::<()>(&r).unwrap_err();
        assert!(e.is_not_found());
        let e = e.as_api().unwrap();
        assert_eq!(e.message, "identifier: nope");
        assert_eq!(e.request_id.as_deref(), Some("abc"));

        let r = response(404, r#"{"error_type":"RecordNotFound"}"#);
        assert_eq!(
            parse::<()>(&r).unwrap_err().as_api().unwrap().message,
            "Not found"
        );
    }

    #[test]
    fn error_type_outranks_status_and_account_required_outranks_both() {
        let r = response(
            403,
            r#"{"error":"nope","error_type":"EntitlementRequired","entitlement":"custom_domains"}"#,
        );
        let e = parse::<()>(&r).unwrap_err();
        let e = e.as_api().unwrap();
        assert_eq!(e.kind, ApiErrorKind::EntitlementRequired);
        assert_eq!(e.entitlement.as_deref(), Some("custom_domains"));

        let r = response(
            403,
            r#"{"error":"nope","error_type":"GuestRoutesDisabled"}"#,
        );
        assert_eq!(
            parse::<()>(&r).unwrap_err().as_api().unwrap().kind,
            ApiErrorKind::Forbidden
        );

        // Historically a 400 FormError with field "requires_account".
        let r = response(
            400,
            r#"{"error":"Sign in","error_type":"FormError","field":"requires_account"}"#,
        );
        let e = parse::<()>(&r).unwrap_err();
        assert_eq!(e.as_api().unwrap().kind, ApiErrorKind::AccountRequired);
        assert!(e.is_unauthorized());
        assert_eq!(
            e.as_api().unwrap().field.as_deref(),
            Some("requires_account")
        );

        let r = response(400, r#"{"code":"GUEST_CONCEAL_REQUIRES_ACCOUNT"}"#);
        let e = parse::<()>(&r).unwrap_err();
        assert_eq!(e.as_api().unwrap().kind, ApiErrorKind::AccountRequired);
        assert!(
            e.as_api()
                .unwrap()
                .message
                .contains("authenticated account")
        );

        let r = response(400, r#"{"field":"accountable"}"#);
        assert_eq!(
            parse::<()>(&r).unwrap_err().as_api().unwrap().kind,
            ApiErrorKind::Invalid
        );
    }

    #[test]
    fn retry_after_in_the_body_is_used_when_the_header_is_absent() {
        let r = response(
            429,
            r#"{"error":"slow down","error_type":"LimitExceeded","retry_after":12}"#,
        );
        let e = parse::<()>(&r).unwrap_err();
        let e = e.as_api().unwrap();
        assert!(e.is_rate_limited());
        assert_eq!(e.retry_after, Some(Duration::from_secs(12)));
        assert_eq!(e.body.as_ref().unwrap()["error_type"], "LimitExceeded");
    }

    #[test]
    fn non_json_error_bodies_still_produce_an_error() {
        let r = response(502, "<html>bad gateway</html>");
        let e = parse::<()>(&r).unwrap_err();
        let e = e.as_api().unwrap();
        assert_eq!(e.kind, ApiErrorKind::Server);
        assert_eq!(e.message, "Server error (502)");
        assert!(e.body.is_none());
    }

    #[test]
    fn retry_after_seconds() {
        assert_eq!(retry_after("30"), Some(Duration::from_secs(30)));
        assert_eq!(retry_after("Wed, 21 Oct 2026 07:28:00 GMT"), None);
    }

    #[test]
    fn post_bodies_are_serialized_once() {
        let req = Request::post("secret/conceal", &serde_json::json!({"a": 1})).unwrap();
        assert_eq!(req.method, Method::Post);
        assert_eq!(
            req.body.as_deref().map(Vec::as_slice),
            Some(&b"{\"a\":1}"[..])
        );
    }
}
