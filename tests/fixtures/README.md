# Response fixtures

Captured from `https://eu.onetimesecret.com` on 2026-10-20 against the v2
API's guest routes, pretty-printed and otherwise unmodified. The secrets
behind them were throwaways and are long gone; the identifiers are inert.

Three are not raw captures:

- `v2_receipt_recent_200.json` needs an account to capture; it is shaped
  per the OpenAPI spec around a captured receipt record.
- `v2_supported_locales_200.json` is a captured response with the locale
  list shortened.
- `v2_system_version_200.json` needs an account too (`GET /version` is
  authenticated: the build version fingerprints an install). It is shaped
  per the OpenAPI spec and the server's handler, which splits a dotted
  version string, so the parts are strings. The numbers are illustrative.

`v2_secret_status_unknown_200.json` is what `GET /secret/{id}/status`
returns, with HTTP 200, for a secret the server does not hold: one that
never existed, or one already revealed, burned or expired. The server does
not say which.

`v3_conceal_200.json` is kept for comparison only: the SDK speaks v2, and
this shows the two formats are the same shape today.

Re-capture when the server changes. The integration tests in
`../client.rs` and `../blocking.rs` serve these through a mock server, so
a changed fixture is the quickest way to see what a server change does to
the SDK.

## Capturing from a self-hosted server

Any server that speaks the v2 API will do, including one running on your
own machine. Point `ONETIME_BASE_URL` at it (the scheme, host and port the
server is actually listening on; there is no default port) and, for the
authenticated routes, give it an account's extid and API token:

```sh
export ONETIME_BASE_URL=http://127.0.0.1:PORT
export ONETIME_CUSTOMER_EXTID=ur1abc23def
export ONETIME_API_TOKEN=...

# Guest routes need no credentials.
curl --silent --show-error --fail-with-body \
  --header 'Accept: application/json' \
  "$ONETIME_BASE_URL/api/v2/status" | jq . > v2_system_status_200.json

# Account routes send the credentials as HTTP basic auth, as the SDK does.
curl --silent --show-error --fail-with-body \
  --header 'Accept: application/json' \
  --user "$ONETIME_CUSTOMER_EXTID:$ONETIME_API_TOKEN" \
  "$ONETIME_BASE_URL/api/v2/version" | jq . > v2_system_version_200.json
```

For an error response drop `--fail-with-body` and add `--write-out
'%{http_code}\n'` so the status is visible; the file name carries it.

Before committing a capture:

- Create throwaway secrets for it; the identifiers end up in the
  repository, so they must not point at anything.
- Replace the extid and token if they appear in the body (they should
  not, but check), and look for anything that names the machine.
- Pretty-print with `jq .` and leave the content alone otherwise. A
  fixture that was edited beyond that is listed at the top of this file
  with what changed.
- Say in this file where it came from: a hosted region, a self-hosted
  server and its version (`GET /version`), or the spec.

The `roundtrip` example (`cargo run --example roundtrip`) uses the same
`ONETIME_BASE_URL` and walks a secret through its whole life against a
real server, which is a quick way to check that a self-hosted server
still matches these fixtures before re-capturing.
