# 0002. Design independently of prior clients

Status: accepted, 2026-10-20

## Context

Two clients already exist: `onetime-ruby`, the Ruby SDK, and `ots-client`,
a small sans-IO crate inside the macOS companion app (`onetimesecret/macos`,
`crates/ots-client` and `crates/transport`). The macOS crate is good code
for its purpose: synchronous, conceal-only, no runtime, every byte of the
secret zeroized, built to be wrapped in an FFI layer. The intent is for the
app to replace it with this crate eventually.

Mirroring either would import constraints that are theirs, not an SDK's:
the Ruby client's version switch and string-keyed responses, the macOS
crate's single operation, hand-rolled base64 and `Box<dyn AuthStrategy>`
indirection that exists for a Swift caller.

## Decision

Design from the API and from how good Rust SDKs look today, not from the
existing implementations. Specifically:

- An async `reqwest` client is the primary interface, as in the crates
  people reach for (`octocrab`, `async-stripe`, the AWS SDK). A blocking
  client is a feature flag, not the default.
- Full coverage of the public API, organised by resource
  (`secrets()`, `receipts()`, `system()`), not a single function.
- Typed identifiers and typed states rather than strings.
- Dependencies are the ordinary ones (`reqwest`, `serde`, `secrecy`,
  `thiserror`, `url`), chosen for maintenance and familiarity over
  minimalism.

What is kept from the macOS crate is an idea rather than code: the layer
that builds requests and reads responses is pure and testable without a
network (0003). Its reasons for wanting that layer (FFI, sync, frugality)
are also why the blocking client exists.

Cross-SDK conventions that belong to the product are kept on purpose;
ADR 0008 lists them.

## Consequences

- The macOS app's migration will mean adopting `blocking::Client` (or the
  async one) and losing its two-host allowlist and page-locked buffers,
  which are app policies and belong in the app.
- This crate owes nothing to the shapes of the other clients; a change in
  one does not imply a change here.
