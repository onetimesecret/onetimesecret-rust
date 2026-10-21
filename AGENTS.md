# AGENTS.md

Guidance for coding agents (and humans) working in this repository.

## What this is

`onetimesecret`: the official Rust client for the Onetime Secret API v2.
One crate, async by default on `reqwest`, with a blocking client behind the
`blocking` feature. It belongs to a family of SDKs (`onetimesecret-ruby`,
`onetimesecret-py`, `onetimesecret-js`, ...) that share product conventions
(ADR 0008) but is written as an idiomatic Rust library, not a port.

## Commands

```sh
cargo test --all-features
cargo fmt --all
cargo clippy --all-features --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps
scripts/check-release.sh        # Cargo.toml and CHANGELOG.md agree
```

Run all of them before finishing. CI also checks the MSRV (1.85), minimal
dependency versions, `cargo-deny` and `cargo-semver-checks`.

## Rules

- Every API operation lives in `src/api/endpoints.rs` as a pure value. The
  async (`src/api/mod.rs`) and blocking (`src/blocking.rs`) clients only move
  bytes. A new operation is an endpoint function, a fixture, a test, and the
  two thin wrappers.
- Response types are liberal (unknown fields ignored, lenient scalars via
  `src/types/wire.rs`, enums with `Unknown`). Request types are strict; no
  `serde_json::Value` in the public API.
- Secret values, passphrases and API tokens are `SecretString` end to end and
  never appear in `Debug` output or error messages.
- Only idempotent (`GET`) requests are retried. Never retry conceal, reveal
  or burn.
- Tests do not touch the network. The integration tests replay captured
  responses from `tests/fixtures/` through wiremock; say where and when a
  new fixture was captured in `tests/fixtures/README.md`.
- Clippy runs at `pedantic`; `unsafe_code` is forbidden; public items need
  docs. Format with rustfmt (`rustfmt.toml`).
- Add a `CHANGELOG.md` line for user-visible changes.
- Releases are tag-driven; do not bump the version in feature PRs. See
  `docs/releasing.md`.

## Where things are decided

- `docs/adr/`: the design decisions and the reasons for them.
- `docs/releasing.md`: setup and procedure for publishing to crates.io.
