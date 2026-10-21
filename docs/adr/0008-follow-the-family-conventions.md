# 0008. Follow the SDK family's conventions

Status: accepted, 2026-10-21

## Context

Onetime Secret maintains official SDKs in several languages, with
`onetimesecret-ruby` as the reference. ADR 0002 says this crate is designed from
the API, not from the other clients' code. That leaves a question: which
behaviours are product conventions that every SDK should share, and which
are implementation details each language decides for itself?

The first draft of this crate decided a few of the shared ones on its own
(message precedence, no retries, release-plz for releases) and got them
different from the family.

## Decision

Behaviour that a user or operator would notice across SDKs follows
`onetimesecret-ruby`, and changes there first or in step:

- Environment variables: `ONETIME_BASE_URL`, `ONETIME_CUSTOMER_EXTID`,
  `ONETIME_API_TOKEN`.
- Credentials: the customer extid (`ur…`) is validated at construction,
  with the same hint about where to find it. The server also accepts an
  email address; the SDKs deliberately do not, so a wrong-kind identifier
  fails loudly instead of as a 401.
- Error classification and message precedence (ADR 0006).
- Retries: idempotent (`GET`) requests only, on connection failures,
  timeouts, 429, 500, 502, 503 and 504; exponential backoff from 0.5s;
  two retries by default. This crate additionally honours `Retry-After`
  and caps the wait at 30s.
- Headers: `User-Agent: onetimesecret-rust/<version> (Rust/<rustc>)`
  names the repository, and `X-Onetime-Client: rust:<rustc>/<version>`
  is the cross-SDK client header.
- Refusing `onetimesecret.com` as a base URL, and accepting a full link
  wherever an identifier is expected.
- Releases: the version lives in the repository, a `v*` tag publishes it,
  CI verifies the tag matches, the registry trusts the workflow through
  OIDC, and the GitHub Release is made from the changelog
  (`docs/releasing.md`). `CHANGELOG.md` is written by hand in Keep a
  Changelog form.

How those are implemented stays this crate's business: typed errors
rather than an exception hierarchy, `SecretString` for secret material,
an async client, a pure protocol layer.

## Consequences

- A maintainer who knows one official SDK can predict this one.
- release-plz, which would have automated version bumps from commit
  messages, was dropped for the family's tag-driven flow. The price is a
  hand-edited `Cargo.toml` and changelog per release; `scripts/check-release.sh`
  keeps them in agreement and CI enforces it.
- When the Ruby SDK changes a shared behaviour, this crate should follow
  in its next release and say so in the changelog.
