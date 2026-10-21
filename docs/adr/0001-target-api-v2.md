# 0001. Target the v2 API

Status: accepted, 2026-10-20

## Context

The server exposes three API versions. v1 is form-encoded and legacy. v2 is
documented as "current stable, recommended for production". v3 powers the
web UI, has cleaner native JSON types, and is labelled alpha, "subject to
change without deprecation".

More decisive than the labels: in the server's route tables, v3's
authenticated secret routes accept session authentication only
(`auth=sessionauth`), while v2's accept HTTP Basic (`auth=basicauth,noauth`).
An API-key client cannot use v3 for the operations that need an account.

The generated OpenAPI spec for v2 describes most scalars as strings
("V2 API returns string values for all fields"), but responses captured
from production on 2026-10-20 carry native numbers, booleans and nulls,
and are the same shape as v3's.

## Decision

Speak v2 only, mounted at `api/v2/`. Do not expose an API-version switch.
Parse scalar fields so that both the documented (stringly) and the observed
(native) representations work (see 0004).

## Consequences

- One code path, one set of fixtures, one thing to test.
- When v3 stabilises and accepts API keys, the move is a path constant and
  a fixture refresh, not a redesign; a test already feeds a v3 response
  through the v2 types as a canary.
- v1-only servers (very old self-hosted installs) are not supported.
