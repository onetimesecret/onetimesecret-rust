# onetimesecret

[![crates.io](https://img.shields.io/crates/v/onetimesecret.svg)](https://crates.io/crates/onetimesecret)
[![docs.rs](https://docs.rs/onetimesecret/badge.svg)](https://docs.rs/onetimesecret)
[![CI](https://github.com/onetimesecret/onetimesecret-rust/actions/workflows/ci.yml/badge.svg)](https://github.com/onetimesecret/onetimesecret-rust/actions/workflows/ci.yml)
![MSRV 1.85](https://img.shields.io/badge/MSRV-1.85-blue)
[![License: MIT](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)

The official Rust SDK for [Onetime Secret](https://onetimesecret.com): share
a secret through a link that works exactly once.

- Async client on [`reqwest`](https://crates.io/crates/reqwest), with an
  optional blocking client behind the `blocking` feature.
- Covers the public v2 API: conceal, generate, reveal, status, and the
  receipts a creator keeps (show, list, burn, memo).
- Typed identifiers, typed errors, typed lifecycle states. Secrets and
  passphrases are [`SecretString`](https://docs.rs/secrecy)s: redacted from
  `Debug`, zeroized on drop.
- Retries idempotent requests on connection failures, timeouts, 429 and
  5xx, with exponential backoff and `Retry-After`. Never retries a create,
  reveal, or burn.
- Works with the hosted regions, self-hosted installs, and custom domains.
- Behaves like the other official SDKs: same environment variables, same
  error classification, same credential checks.

## Install

```sh
cargo add onetimesecret
```

Features: `rustls` (default), `native-tls`, `blocking`.

## Use

```rust
use std::time::Duration;
use onetimesecret::{Client, ConcealSecret, Credentials, Region};

#[tokio::main]
async fn main() -> Result<(), onetimesecret::Error> {
    let client = Client::builder()
        .region(Region::Eu)
        .credentials(Credentials::new("ur1abc23def", "your-api-token"))
        .build()?;

    let created = client
        .secrets()
        .conceal(ConcealSecret::new("the launch codes").ttl(Duration::from_secs(3600)))
        .await?;

    println!("send this, once: {}", created.share_url);
    println!("your receipt:    {}", created.receipt.identifier);
    Ok(())
}
```

Reveal on the other side:

```rust
use onetimesecret::secrecy::ExposeSecret;

let revealed = client.secrets().reveal("https://eu.onetimesecret.com/secret/…").await?;
println!("{}", revealed.value.expose_secret());
```

Or let the environment configure it:

```sh
export ONETIME_BASE_URL=https://eu.onetimesecret.com
export ONETIME_CUSTOMER_EXTID=ur1abc23def   # the "ur…" id in the user menu
export ONETIME_API_TOKEN=…
```

```rust
let client = onetimesecret::Client::from_env()?;
```

Without credentials the client is anonymous and uses the API's `/guest/`
routes; everything except listing receipts and editing memos works.

### Authentication

Authentication is HTTP Basic: the **customer extid** is the username and
the **API token** is the password. The extid is a short, opaque identifier
beginning with `ur`, shown with a copy button at the bottom of the user
menu when you are signed in. Other identifiers the API uses are not
interchangeable with it, so `Credentials` checks the shape when the client
is built:

```text
invalid configuration: customer "acct-123" is not a customer extid: extids
begin with "ur" (e.g. "ur1abc23def"). Your customer extid is the "ur…"
identifier at the bottom of the user menu …
```

> Self-hosted servers older than
> [onetimesecret#3945](https://github.com/onetimesecret/onetimesecret/pull/3945)
> silently accept invalid credentials and create the secret anonymously
> instead of returning 401. Current servers return 401.

The [examples](examples/) cover conceal, reveal, receipts, and the blocking
client. Full reference on [docs.rs](https://docs.rs/onetimesecret).

## Errors

```rust
match client.secrets().reveal(id).await {
    Ok(revealed) => { /* … */ }
    Err(e) if e.is_not_found() => println!("already revealed, burned, or expired"),
    Err(e) => match e.as_api() {
        Some(api) if api.kind == onetimesecret::ApiErrorKind::Invalid => {
            println!("rejected: {} (request {})", api.message, api.request_id.as_deref().unwrap_or("?"));
        }
        _ => return Err(e.into()),
    },
}
```

`Error::Transport` is a failure to get any response; `Error::Api` carries
the status, an `ApiErrorKind` (`Invalid`, `Unauthorized`,
`AccountRequired`, `Forbidden`, `EntitlementRequired`, `NotFound`,
`Conflict`, `RateLimited`, `Server`), the server's message, and its
request id; `Error::Decode` means a success response this SDK could not
read. The classification is the one every official SDK uses, derived from
the server's `error_type` when it sends one.

## Servers

| Target | Builder |
| --- | --- |
| Hosted region | `.region(Region::Us)` (also `Eu`, `Ca`, `Uk`, `Nz`) |
| Self-hosted | `.base_url("https://secrets.example.com")`, path prefixes kept |
| Custom domain | `.base_url("https://your.domain")`, or `.share_domain(…)` per request |
| Local development | `.base_url("http://localhost:3000").allow_http(true)` |

`https://onetimesecret.com` itself is the website, not an API host; the
builder refuses it.

## Compatibility

- Rust 1.85 or newer (the library; some dev-dependencies need newer).
- The v2 API, hosted or self-hosted. Responses are read liberally so that a
  server a little newer or older than the SDK still works.
- Pre-1.0: minor versions may break the API; the changelog says when.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Design decisions are recorded in
[docs/adr](docs/adr/). Maintainers: see
[docs/releasing.md](docs/releasing.md).

## License

MIT. See [LICENSE](LICENSE).
