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

To run the examples against a real server, set `ONETIME_BASE_URL` and,
for account operations, `ONETIME_CUSTOMER_EXTID` and `ONETIME_API_TOKEN`.
Use a throwaway account or the anonymous guest routes; the examples create
real secrets.

## Before you open a pull request

```sh
cargo fmt --all
cargo clippy --all-features --all-targets -- -D warnings
cargo test --all-features
cargo doc --all-features --no-deps      # with RUSTDOCFLAGS="-D warnings"
```

CI runs these plus an MSRV check, `cargo-deny`, and `cargo-semver-checks`.

## Commits and pull requests

Pull request titles follow [Conventional Commits](https://www.conventionalcommits.org/)
(`feat:`, `fix:`, `docs:`, `chore:`; `feat!:` for a breaking change).
Squash-merge is the norm, so the title is what lands on `main`.

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
