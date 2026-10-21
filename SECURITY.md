# Security

## Reporting a vulnerability

Report vulnerabilities in this SDK privately through
[GitHub security advisories](https://github.com/onetimesecret/onetimesecret-rust/security/advisories/new)
or by email to security@onetimesecret.com. Please do not open a public
issue.

You will get an acknowledgement within three business days and a fix or a
plan within thirty. Credit is given in the advisory unless you prefer
otherwise.

Vulnerabilities in the Onetime Secret service itself are covered by the
[service's security policy](https://github.com/onetimesecret/onetimesecret/blob/main/SECURITY.md).

## Supported versions

The latest minor release receives fixes. Pre-1.0, that means the most
recent `0.x` line.

## What the SDK does and does not promise

- Secrets, passphrases and API tokens are held in
  [`secrecy::SecretString`](https://docs.rs/secrecy): redacted from `Debug`,
  zeroized when dropped. Request bodies are built in a buffer that is
  zeroized after the request. Copies the HTTP stack makes while sending
  are outside the SDK's control.
- TLS is required, with one exception: a plain `http://` base URL naming
  a loopback host (`localhost`, `127.0.0.1`, `::1`) is accepted, since that
  traffic never leaves the machine. Any other `http://` host needs
  `allow_http(true)`.
- Redirects are never followed, so credentials cannot be sent to a host
  other than the configured base URL.
- A base URL with a username or password in it is refused; credentials
  travel only as HTTP Basic auth from `Credentials`, and never into the
  share links the SDK builds.
- Identifiers are percent-encoded into a single path segment, so a value
  that is not an identifier cannot redirect a request within the server.
- Transport errors (`Error::Transport`) omit the request URL, which can
  name a secret. The base URL is available from `Client::base_url`.
- Server error messages are surfaced as-is in `ApiError::message`. The
  server does not echo secrets, but treat logs of these messages as you
  would any API log.
- The SDK does not validate TLS beyond what `reqwest` and the chosen
  backend do, and does not pin certificates.
