# 0004. Parse responses liberally

Status: accepted, 2026-10-20

## Context

The server is a moving target: the v2 spec says strings, production sends
numbers; `recipients` has been an array, a comma-separated string, an
empty string and `null`; timestamps are fractional epochs; new fields and
new enum values appear without notice. Self-hosted installs lag the hosted
service by months.

An SDK that fails to parse a response is useless at exactly the moment the
user needs it, and the failure is opaque.

## Decision

- Unknown fields are ignored (serde's default; never `deny_unknown_fields`).
- Scalar fields go through helpers in `types::wire` that accept numbers or
  numeric strings for timestamps and durations, booleans or
  `"true"`/`"false"`/`"1"`/`"0"` for flags, and the four shapes of
  `recipients`.
- Enums (`SecretState`, `ReceiptState`, `SecretKind`) carry an
  `Unknown(String)` variant and are `#[non_exhaustive]`.
- Response structs are `#[non_exhaustive]` so fields can be added.
- Fields the spec marks optional, or that only some responses carry, are
  `Option`. Fields marked required stay required; a missing one is a
  genuine server change and surfaces as `Error::Decode` with the status.
- Garbage still fails: `"ttl": "soon"` or a negative duration is an error,
  not a silent default.

Request bodies are the opposite: typed, exact, and built by hand-written
`Serialize` impls so the wire shape is visible in one place.

## Consequences

- Fixtures captured from a real server are the test oracle; keep them raw.
- A newer server mostly works with an older SDK, which is the right way
  round for a library.
- Users cannot exhaustively match on states; they must handle `Unknown` or
  use `_`.
