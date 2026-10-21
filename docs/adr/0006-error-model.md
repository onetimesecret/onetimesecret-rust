# 0006. One error type, classified API errors

Status: accepted, 2026-10-20

## Context

Callers need to tell apart: no response at all; the server said no; the
server said yes in a shape the SDK cannot read; and the client was
misconfigured. Within "the server said no", the useful distinctions are
not-found (which also means "already revealed"), unauthorized, validation
(including wrong passphrase), rate limiting, and server failure.

The server's error bodies are not uniform. Observed shapes include
`{error, error_type, request_id}`, `{error, message, code, code_scope,
timestamp}`, and `{error, error_type, field, request_id}`.

## Decision

- A single `Error` enum: `Config`, `Transport(reqwest::Error)`,
  `Api(Box<ApiError>)`, `Decode { status, message }`. `#[non_exhaustive]`.
- `ApiError` keeps every field the server has been seen to send, all
  optional except `status` and `message`, plus the decoded `body`.
- `kind: ApiErrorKind` is the classification every official SDK uses,
  taken from `onetimesecret-ruby`'s exception hierarchy: the server's
  `error_type` when known (`FormError`, `RequiresAccount`,
  `RecordNotFound`, `GuestRoutesDisabled`, `EntitlementRequired`,
  `LimitExceeded`, `ServerError`), else the status. The account
  requirement is special-cased because the server has reported it as a
  400 `FormError` with field `requires_account`, as an `error_key`, and as
  a code such as `GUEST_CONCEAL_REQUIRES_ACCOUNT`; all of these become
  `AccountRequired`, which `is_unauthorized()` includes.
- `message` follows the Ruby SDK's precedence, `error` then `message`,
  with a per-kind default when both are absent. When the server sends both
  and they differ, the second is kept as `detail` and shown after the
  message, so nothing is lost (`Authentication Required:
  [CREDENTIALS_INVALID] Invalid credentials`).
- `retry_after` comes from the header or, failing that, the body.
- Convenience predicates on both types: `is_not_found`,
  `is_unauthorized`, `is_rate_limited`, and `is_transient` for the kinds a
  retry may clear.
- No distinct `WrongPassphrase` variant: the server signals it with a 422
  and a free-text message, which is not stable enough to match on. It
  arrives as `ApiErrorKind::Invalid`.

## Consequences

- `ApiError` is boxed because it is large; `Error` stays small to return.
- Adding a server field later is a non-breaking change (`#[non_exhaustive]`
  struct, `Option` field).
- Idempotent requests are retried by the client (ADR 0008); everything
  else surfaces at once with `retry_after` for the caller to use.
