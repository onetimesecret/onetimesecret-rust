<!--
Title as a conventional commit:
  feat: add receipts.recent
  fix(client): keep the base URL path prefix
  feat!: rename SecretKey to SecretId     (breaking)
-->

## What

## Why

## Checklist

- [ ] `cargo test --all-features` passes
- [ ] `cargo clippy --all-features --all-targets -- -D warnings` is clean
- [ ] Public API changes have docs and, if breaking, a `!` in the title
- [ ] Behaviour that depends on a server response has a fixture-backed test
- [ ] User-visible changes have a line under `## [Unreleased]` in CHANGELOG.md
