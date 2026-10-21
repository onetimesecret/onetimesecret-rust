# 0007. Guest routes for anonymous clients

Status: accepted, 2026-10-20

## Context

v2 has two families of routes for the same operations: `/secret/…` and
`/receipt/…`, which accept Basic auth or no auth, and `/guest/secret/…`
and `/guest/receipt/…`, which exist for anonymous use and are subject to
the server's guest-route gating and limits. Status checks and system
endpoints have no guest variant. Listing receipts and editing memos
require an account.

## Decision

A client built without `Credentials` is anonymous and uses the `/guest/`
routes wherever they exist. A client with credentials uses the regular
routes and sends Basic auth on every request. There is no per-call
`guest` flag.

Operations that need an account are not blocked client-side; the server's
401 comes back as `ApiErrorKind::Unauthorized`, with its message intact.

## Consequences

- Anonymous and authenticated use are a build-time choice, visible in
  `Client::is_authenticated()` and in `Debug` output.
- A self-hosted server that disables guest routes rejects anonymous
  clients; that is the operator's decision and the error says so.
- If the server ever drops the guest family, `Context::route` is the one
  place to change.
