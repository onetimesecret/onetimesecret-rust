# Changelog

All notable changes to this project are documented here. The format is
based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the
project follows [Semantic Versioning](https://semver.org/).

Each release has a `## [x.y.z] - YYYY-MM-DD` heading that must match
`Cargo.toml`; `scripts/check-release.sh` verifies it, and the Release
workflow uses the section as the GitHub release notes.

## [0.1.0] - unreleased

### Added

- Async `Client` over reqwest with `rustls` (default) or `native-tls`.
- `blocking::Client` behind the `blocking` feature.
- Secrets: `conceal`, `generate`, `show`, `status`, `status_many`,
  `reveal`, `reveal_with_passphrase`.
- Receipts: `show`, `recent`, `burn`, `burn_with_passphrase`, `update_memo`.
- `generate` claims the generated value, which the server discloses only
  on the first fetch of the receipt, and returns it in
  `GeneratedSecret::value`. A failed claim keeps the created secret and
  attaches the error. `GenerateSecret::claim_value(false)` opts out;
  `Receipt::secret_value` then carries the value on that first fetch.
- System: `status`, `version`, `supported_locales`.
- Typed `SecretId` and `ReceiptId` that also parse full links.
- `Error` and `ApiError` with the SDK family's classification
  (`AccountRequired`, `EntitlementRequired`, `RateLimited`, …), the
  server's message, request id and `Retry-After`.
- Retries with exponential backoff for idempotent requests on connection
  failures, timeouts, 429 and 5xx (`max_retries`, default 2).
- Customer extid validation at build time, matching the other SDKs.
- `User-Agent` and `X-Onetime-Client` headers identifying the SDK.
- Configuration from `ONETIME_BASE_URL`, `ONETIME_CUSTOMER_EXTID` and
  `ONETIME_API_TOKEN`.

### Security

- A plain `http://` base URL is accepted without `allow_http(true)` only
  for loopback hosts. The names `localhost` and `*.localhost` are pinned
  to `127.0.0.1` and `::1` in the HTTP client, so a hosts file or
  resolver that points them elsewhere cannot send plaintext off the
  machine.
