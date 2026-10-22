# Contributing

Thanks for helping. This file is short on purpose; the code and its tests
are the reference.

## Setting up

```sh
git clone https://github.com/onetimesecret/onetimesecret-rust
cd onetimesecret-rust
cargo test --all-features
```

Rust 1.85 or newer builds the library; the test suite wants a current
stable toolchain. No server is needed: the integration tests replay
responses captured from the real API (`tests/fixtures/`).

## Running against a server

To run the examples against a real server, set `ONETIME_BASE_URL` and,
for account operations, `ONETIME_CUSTOMER_EXTID` and `ONETIME_API_TOKEN`.
Use a throwaway account or the anonymous guest routes; the examples create
real secrets.

The live suite in `tests/live.rs` checks the same flows against a real
server. It is off unless `ONETIME_LIVE=1` is set, so `cargo test` stays
offline:

```sh
ONETIME_LIVE=1 cargo test --test live
ONETIME_LIVE=1 ONETIME_BASE_URL=http://127.0.0.1:7143 cargo test --test live
```

Without `ONETIME_BASE_URL` it targets `https://eu.onetimesecret.com`, as
the other SDKs' live suites do. The tests that need an account return
early unless both credential variables are set.

Against a local server, start it with `HOST` set to the host and port in
`ONETIME_BASE_URL` (for example `HOST=127.0.0.1:7143`), because the server
builds the links in receipts from `HOST`, which defaults to
`localhost:3000`.

## Before you open a pull request

```sh
cargo fmt --all
cargo clippy --all-features --all-targets -- -D warnings
cargo test --all-features
cargo doc --all-features --no-deps      # with RUSTDOCFLAGS="-D warnings"
```

CI runs these plus an MSRV check, `cargo-deny`, and `cargo-semver-checks`.

## Commits and pull requests

Commit messages and pull request titles use an imperative subject line
under 72 characters, with a body explaining *why* when it is not obvious.
No type prefixes. Squash-merge is the norm, so the PR title is what lands
on `main`; say "breaking" in the title or body when the public API changes.

Anything a user would notice gets a line under `## [Unreleased]` in
`CHANGELOG.md`, in the same pull request, under `Added`, `Changed`,
`Deprecated`, `Removed`, `Fixed` or `Security`. The release process turns
that section into the release notes; see [docs/releasing.md](docs/releasing.md).

## Design

Decisions that shaped the crate are in [docs/adr](docs/adr/). Read them
before proposing a change in the same area; add one when you make a
decision of the same weight.

Some rules of thumb that fall out of them:

- Response types are liberal: unknown fields ignored, scalars accepted in
  the forms the server has used over time, enums with an `Unknown`
  variant. A server change should degrade gracefully, not panic.
- Request types are strict and typed. Do not add `serde_json::Value`
  escape hatches to the public API.
- Secret material is `SecretString` end to end and never appears in
  `Debug` output or error messages.
- Every operation lives in `src/api/endpoints.rs` as a pure value; the
  async and blocking clients only move bytes. A new operation is a new
  endpoint function, a fixture, a test, and two thin wrappers.
- Anything that depends on a server response gets a fixture-backed test.
  Capture fixtures from a real server and say where and when in
  `tests/fixtures/README.md`.
- Behaviour the SDK family shares (error classification, credential
  checks, retry policy, environment variables, client headers) follows
  `onetimesecret-ruby`, the reference implementation. Change it there first, or
  at least in step.

## Reporting security issues

See [SECURITY.md](SECURITY.md). Please do not open public issues for
vulnerabilities.
