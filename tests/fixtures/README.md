# Response fixtures

Captured from `https://eu.onetimesecret.com` on 2026-10-20 against the v2
API's guest routes, pretty-printed and otherwise unmodified. The secrets
behind them were throwaways and are long gone; the identifiers are inert.

Two are not raw captures:

- `v2_receipt_recent_200.json` needs an account to capture; it is shaped
  per the OpenAPI spec around a captured receipt record.
- `v2_supported_locales_200.json` is a captured response with the locale
  list shortened.

`v2_secret_status_unknown_200.json` is what `GET /secret/{id}/status`
returns, with HTTP 200, for a secret the server does not hold: one that
never existed, or one already revealed, burned or expired. The server does
not say which.

`v3_conceal_200.json` is kept for comparison only: the SDK speaks v2, and
this shows the two formats are the same shape today.

Re-capture when the server changes. The integration tests in
`../client.rs` serve these through a mock server, so a changed fixture
is the quickest way to see what a server change does to the SDK.
