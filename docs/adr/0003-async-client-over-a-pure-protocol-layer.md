# 0003. Async client over a pure protocol layer

Status: accepted, 2026-10-20

## Context

Two clients (async and blocking) must behave identically. Tests should not
need a network. The macOS app and other sync programs want a client
without a runtime of their own.

## Decision

Three layers, all in one crate:

1. `protocol`: `Request` (method, path, bytes), `Response` (status, bytes,
   `Retry-After`), and `parse`, which turns a response into a typed value
   or an `ApiError`. No HTTP client types appear here.
2. `api::endpoints`: one function per API operation returning an
   `Endpoint<T>`: the request plus a `fn` that decodes the response. These
   own all routing and body-shaping decisions.
3. `Client` (async, `reqwest::Client`) and `blocking::Client`
   (`reqwest::blocking::Client`): each has one `call` method that sends a
   `Request` and hands the `Response` to the endpoint's decoder. Resource
   handles are one-line wrappers.

The protocol and endpoint layers are `pub(crate)`. Exposing them for custom
transports is a possible future step; keeping them private now keeps the
public surface small while the API settles.

## Consequences

- Adding an operation touches four places, all small: an endpoint
  function, a fixture, a test, and the two resource wrappers.
- The blocking client is about 150 lines and cannot drift from the async
  one in behaviour, only in signature.
- `reqwest` is a hard dependency in both modes. A `ureq` or custom
  transport would require making layer 1 public; nothing else changes.
