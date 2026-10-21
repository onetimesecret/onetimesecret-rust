//! Error types.

use std::fmt;
use std::time::Duration;

/// A specialised `Result` for this crate.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Anything that can go wrong while talking to Onetime Secret.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The client was misconfigured (bad base URL, incomplete credentials).
    #[error("invalid configuration: {0}")]
    Config(String),

    /// The request never produced an HTTP response: DNS, connection,
    /// TLS, or timeout failures.
    #[error("request failed: {0}")]
    Transport(#[from] reqwest::Error),

    /// The server answered with an error status.
    #[error(transparent)]
    Api(Box<ApiError>),

    /// The server answered with a success status but a body this SDK could
    /// not interpret. Usually means the server is newer or older than the
    /// SDK; please report it.
    #[error("could not decode the response (HTTP {status}): {message}")]
    Decode {
        /// The HTTP status of the response.
        status: u16,
        /// What went wrong while decoding.
        message: String,
    },
}

impl From<ApiError> for Error {
    fn from(e: ApiError) -> Self {
        Error::Api(Box::new(e))
    }
}

impl Error {
    /// The same error with every message from the server or the parser
    /// replaced by a fixed one, keeping what classifies it: the variant,
    /// the status, the kind, and the server's request id and
    /// `Retry-After`.
    ///
    /// For errors from a response that may contain a secret: an API error
    /// echoes the body, and a serde message quotes the input it rejected.
    pub(crate) fn scrubbed(self) -> Self {
        const MESSAGE: &str = "the response was not kept; see the error kind and status";
        match self {
            Error::Config(_) => Error::Config(MESSAGE.into()),
            // reqwest's errors never carry the body; the URL was already
            // stripped when the request was sent.
            Error::Transport(e) => Error::Transport(e),
            Error::Api(e) => Error::Api(Box::new(ApiError {
                status: e.status,
                kind: e.kind,
                message: MESSAGE.into(),
                detail: None,
                error_type: None,
                code: None,
                field: None,
                error_key: None,
                entitlement: None,
                request_id: e.request_id,
                retry_after: e.retry_after,
                body: None,
            })),
            Error::Decode { status, .. } => Error::Decode {
                status,
                message: MESSAGE.into(),
            },
        }
    }

    /// The [`ApiError`] inside, if this is an API error.
    pub fn as_api(&self) -> Option<&ApiError> {
        match self {
            Error::Api(e) => Some(e),
            _ => None,
        }
    }

    /// `true` when the server said the resource does not exist (or no
    /// longer exists: revealed and burned secrets are gone for good).
    pub fn is_not_found(&self) -> bool {
        self.as_api().is_some_and(ApiError::is_not_found)
    }

    /// `true` when the server rejected the credentials, or the operation
    /// needs credentials the client does not have.
    pub fn is_unauthorized(&self) -> bool {
        self.as_api().is_some_and(ApiError::is_unauthorized)
    }

    /// `true` when the request was throttled. See [`ApiError::retry_after`].
    pub fn is_rate_limited(&self) -> bool {
        self.as_api().is_some_and(ApiError::is_rate_limited)
    }
}

/// A classification of an [`ApiError`].
///
/// Derived from the server's `error_type` when it sends one, otherwise from
/// the HTTP status. The mapping is shared with the other official SDKs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ApiErrorKind {
    /// 400 or 422 (`FormError`): the request was rejected by validation.
    /// The message says why and [`ApiError::field`] may name the field.
    /// An incorrect passphrase on reveal or burn arrives this way.
    Invalid,
    /// 401: missing or invalid credentials.
    Unauthorized,
    /// The operation needs an authenticated account and the client has
    /// none. The server reports this inside a 400 body
    /// (`RequiresAccount`, or a `requires_account` field or code) rather
    /// than with a status of its own, so it is classified here instead of
    /// as [`Invalid`](Self::Invalid).
    AccountRequired,
    /// 403 (`Forbidden`, `GuestRoutesDisabled`): not allowed to do this.
    Forbidden,
    /// 403 `EntitlementRequired`: the account's plan does not include
    /// this. [`ApiError::entitlement`] names what is missing.
    EntitlementRequired,
    /// 404 (`RecordNotFound`): the secret or receipt does not exist, or
    /// has already been revealed, burned, or expired.
    NotFound,
    /// 409: the request conflicts with the current state.
    Conflict,
    /// 429 (`LimitExceeded`): too many requests. See
    /// [`ApiError::retry_after`].
    RateLimited,
    /// 5xx (`ServerError`): the server failed.
    Server,
    /// Any other error status.
    Other,
}

impl ApiErrorKind {
    pub(crate) fn from_status(status: u16) -> Self {
        match status {
            400 | 422 => ApiErrorKind::Invalid,
            401 => ApiErrorKind::Unauthorized,
            403 => ApiErrorKind::Forbidden,
            404 => ApiErrorKind::NotFound,
            409 => ApiErrorKind::Conflict,
            429 => ApiErrorKind::RateLimited,
            500..=599 => ApiErrorKind::Server,
            _ => ApiErrorKind::Other,
        }
    }

    /// The server's machine-readable error class, when it is one this SDK
    /// knows.
    pub(crate) fn from_error_type(error_type: &str) -> Option<Self> {
        Some(match error_type {
            "FormError" => ApiErrorKind::Invalid,
            "RequiresAccount" | "AccountRequired" => ApiErrorKind::AccountRequired,
            "RecordNotFound" | "NotFound" => ApiErrorKind::NotFound,
            "Forbidden" | "GuestRoutesDisabled" => ApiErrorKind::Forbidden,
            "EntitlementRequired" => ApiErrorKind::EntitlementRequired,
            "LimitExceeded" => ApiErrorKind::RateLimited,
            "ServerError" => ApiErrorKind::Server,
            _ => return None,
        })
    }

    /// The message used when the server sends none.
    pub(crate) fn default_message(self, status: u16) -> String {
        match self {
            ApiErrorKind::Invalid => "Bad request".into(),
            ApiErrorKind::Unauthorized => "Authentication failed".into(),
            ApiErrorKind::AccountRequired => {
                "This operation requires an authenticated account: configure \
                 Credentials with your customer extid and API token"
                    .into()
            }
            ApiErrorKind::Forbidden | ApiErrorKind::EntitlementRequired => "Forbidden".into(),
            ApiErrorKind::NotFound => "Not found".into(),
            ApiErrorKind::Conflict => "Conflict".into(),
            ApiErrorKind::RateLimited => "Rate limit exceeded".into(),
            ApiErrorKind::Server => format!("Server error ({status})"),
            ApiErrorKind::Other => format!("Request failed ({status})"),
        }
    }
}

impl fmt::Display for ApiErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            ApiErrorKind::Invalid => "invalid request",
            ApiErrorKind::Unauthorized => "unauthorized",
            ApiErrorKind::AccountRequired => "account required",
            ApiErrorKind::Forbidden => "forbidden",
            ApiErrorKind::EntitlementRequired => "entitlement required",
            ApiErrorKind::NotFound => "not found",
            ApiErrorKind::Conflict => "conflict",
            ApiErrorKind::RateLimited => "rate limited",
            ApiErrorKind::Server => "server error",
            ApiErrorKind::Other => "error",
        })
    }
}

/// An error response from the API.
///
/// The server's error bodies are not uniform; every field the server has
/// been seen to send is kept, [`message`](Self::message) always has
/// something readable, and [`body`](Self::body) has the rest.
#[derive(Debug, Clone, thiserror::Error)]
#[non_exhaustive]
pub struct ApiError {
    /// The HTTP status code.
    pub status: u16,
    /// A classification of the error.
    pub kind: ApiErrorKind,
    /// The server's description (its `error`, else `message`), or a
    /// default for the status when the body carried none.
    pub message: String,
    /// The server's `message` when it also sent a distinct `error`,
    /// e.g. `[CREDENTIALS_INVALID] Invalid credentials` alongside
    /// `Authentication Required`.
    pub detail: Option<String>,
    /// The server's error class, e.g. `RecordNotFound`, when present.
    pub error_type: Option<String>,
    /// A machine-readable code, e.g. `api_key_invalid`, when present.
    pub code: Option<String>,
    /// The request field that failed validation, when present.
    pub field: Option<String>,
    /// An i18n key for the message, when present.
    pub error_key: Option<String>,
    /// The entitlement the account lacks, for
    /// [`EntitlementRequired`](ApiErrorKind::EntitlementRequired).
    pub entitlement: Option<String>,
    /// The server's request identifier; quote it in support requests.
    pub request_id: Option<String>,
    /// How long to wait before retrying, from the `Retry-After` header or
    /// the body.
    pub retry_after: Option<Duration>,
    /// The decoded body, when it was JSON.
    pub body: Option<serde_json::Value>,
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (HTTP {}): {}", self.kind, self.status, self.message)?;
        if let Some(detail) = &self.detail {
            write!(f, ": {detail}")?;
        }
        Ok(())
    }
}

impl ApiError {
    /// See [`ApiErrorKind::NotFound`].
    pub fn is_not_found(&self) -> bool {
        self.kind == ApiErrorKind::NotFound
    }

    /// `true` for [`Unauthorized`](ApiErrorKind::Unauthorized) and
    /// [`AccountRequired`](ApiErrorKind::AccountRequired): either way,
    /// valid credentials would have helped.
    pub fn is_unauthorized(&self) -> bool {
        matches!(
            self.kind,
            ApiErrorKind::Unauthorized | ApiErrorKind::AccountRequired
        )
    }

    /// See [`ApiErrorKind::RateLimited`].
    pub fn is_rate_limited(&self) -> bool {
        self.kind == ApiErrorKind::RateLimited
    }

    /// `true` for errors that a later retry may clear: rate limits and
    /// server failures.
    pub fn is_transient(&self) -> bool {
        matches!(self.kind, ApiErrorKind::RateLimited | ApiErrorKind::Server)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scrubbing_keeps_the_classification_and_drops_the_words() {
        let api = ApiError {
            status: 429,
            kind: ApiErrorKind::RateLimited,
            message: "slow down, hunter2".into(),
            detail: Some("hunter2".into()),
            error_type: Some("LimitExceeded".into()),
            code: None,
            field: None,
            error_key: None,
            entitlement: None,
            request_id: Some("req1".into()),
            retry_after: Some(Duration::from_secs(3)),
            body: Some(serde_json::json!({"secret_value": "hunter2"})),
        };
        let scrubbed = Error::from(api).scrubbed();
        let api = scrubbed.as_api().unwrap();
        assert_eq!(api.status, 429);
        assert!(api.is_rate_limited());
        assert_eq!(api.request_id.as_deref(), Some("req1"));
        assert_eq!(api.retry_after, Some(Duration::from_secs(3)));
        assert!(api.body.is_none());
        assert!(!format!("{scrubbed:?}").contains("hunter2"));
        assert!(!scrubbed.to_string().contains("hunter2"));

        let scrubbed = Error::Decode {
            status: 200,
            message: "invalid type: string \"hunter2\"".into(),
        }
        .scrubbed();
        assert!(!scrubbed.to_string().contains("hunter2"));
    }

    #[test]
    fn kinds_follow_status() {
        assert_eq!(ApiErrorKind::from_status(422), ApiErrorKind::Invalid);
        assert_eq!(ApiErrorKind::from_status(401), ApiErrorKind::Unauthorized);
        assert_eq!(ApiErrorKind::from_status(404), ApiErrorKind::NotFound);
        assert_eq!(ApiErrorKind::from_status(429), ApiErrorKind::RateLimited);
        assert_eq!(ApiErrorKind::from_status(503), ApiErrorKind::Server);
        assert_eq!(ApiErrorKind::from_status(409), ApiErrorKind::Conflict);
        assert_eq!(
            ApiErrorKind::from_error_type("GuestRoutesDisabled"),
            Some(ApiErrorKind::Forbidden)
        );
        assert_eq!(ApiErrorKind::from_error_type("Novel"), None);
        assert_eq!(ApiErrorKind::from_status(418), ApiErrorKind::Other);
    }

    #[test]
    fn display_reads_well() {
        let err = ApiError {
            status: 404,
            kind: ApiErrorKind::NotFound,
            message: "identifier: nope".into(),
            detail: None,
            error_type: None,
            code: None,
            field: None,
            error_key: None,
            entitlement: None,
            request_id: None,
            retry_after: None,
            body: None,
        };
        assert_eq!(err.to_string(), "not found (HTTP 404): identifier: nope");
        let err = Error::from(err);
        assert!(err.is_not_found());
        assert!(!err.is_unauthorized());
    }
}
