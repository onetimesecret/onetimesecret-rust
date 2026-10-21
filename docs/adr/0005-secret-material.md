# 0005. Secret material handling

Status: accepted, 2026-10-20

## Context

The SDK handles secret values, passphrases and API tokens. The common
failure is not a cryptographic one; it is a secret ending up in a log via
`{:?}`, an error message, or a panic.

## Decision

- Secret values, passphrases and API tokens are `secrecy::SecretString`
  everywhere they are held: request builders, `Credentials`,
  `RevealedSecret::value`. Reading one requires `expose_secret()`.
- `Debug` impls for anything holding secret material print `[REDACTED]`;
  tests assert this.
- Request bodies are serialized straight to a `Zeroizing<Vec<u8>>` with no
  intermediate `serde_json::Value`, and response bodies are read into a
  `Zeroizing<Vec<u8>>`. The copy handed to `reqwest` and the copies the
  HTTP stack makes are not covered, and the docs say so rather than
  implying otherwise.
- Error messages from the server are passed through; they do not echo
  request bodies today, and the SDK never adds request content to an
  error.
- TLS is required unless `allow_http(true)` is called; redirects are
  disabled so Basic credentials cannot follow a redirect to another host.

Not done, deliberately: page locking, core-dump hardening, host
allowlists. Those are process-level policies for an application to set.

## Consequences

- `secrecy` is part of the public API (re-exported as
  `onetimesecret::secrecy`), so a `secrecy` major bump is a major bump
  here.
- Users of `RevealedSecret` must call `expose_secret()`, which is the
  point.
