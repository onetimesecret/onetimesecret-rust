<!--
Title as an imperative sentence; it becomes the squash commit subject:
  Add receipts.recent
  Keep the base URL path prefix in the client
  Rename SecretKey to SecretId (breaking)
-->

## What

## Why

## Checklist

- [ ] `cargo test --all-features` passes
- [ ] `cargo clippy --all-features --all-targets -- -D warnings` is clean
- [ ] Public API changes have docs and, if breaking, say so in the title
- [ ] Behaviour that depends on a server response has a fixture-backed test
- [ ] User-visible changes have a line under `## [Unreleased]` in CHANGELOG.md
