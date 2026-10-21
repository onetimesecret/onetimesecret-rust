//! The async client and its configuration.

use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use secrecy::{ExposeSecret, SecretString};
use url::Url;
use zeroize::Zeroizing;

use crate::api::endpoints::{Context, Endpoint};
use crate::api::{Receipts, Secrets, System};
use crate::error::{Error, Result};
use crate::protocol::{self, API_PREFIX, Method, Request, Response};

/// Environment variable for the base URL.
pub const ENV_BASE_URL: &str = "ONETIME_BASE_URL";
/// Environment variable for the customer identifier.
pub const ENV_CUSTOMER: &str = "ONETIME_CUSTOMER_EXTID";
/// Environment variable for the API token.
pub const ENV_API_TOKEN: &str = "ONETIME_API_TOKEN";

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Retries for idempotent requests, as in the other official SDKs.
const DEFAULT_MAX_RETRIES: u32 = 2;
/// Base of the exponential backoff: 0.5s, 1s, 2s, …
const BACKOFF_BASE: Duration = Duration::from_millis(500);
/// Never wait longer than this between attempts, whatever `Retry-After`
/// says.
const BACKOFF_CAP: Duration = Duration::from_secs(30);

/// The compiler this SDK was built with, for the client headers.
const RUSTC_VERSION: &str = env!("ONETIMESECRET_RUSTC_VERSION");

/// Where to find the customer identifier, for error messages.
const CUSTOMER_HINT: &str = "Your customer extid is the \"ur…\" identifier at the bottom of the \
    user menu when you are signed in (there is a copy button next to it). Pass it to \
    Credentials::new or set ONETIME_CUSTOMER_EXTID.";

/// The hosted regions. Each is a separate deployment with its own data;
/// an account exists in exactly one of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Region {
    /// `https://us.onetimesecret.com`
    Us,
    /// `https://eu.onetimesecret.com`
    Eu,
    /// `https://ca.onetimesecret.com`
    Ca,
    /// `https://uk.onetimesecret.com`
    Uk,
    /// `https://nz.onetimesecret.com`
    Nz,
}

impl Region {
    /// The region's base URL.
    #[allow(clippy::missing_panics_doc)] // parses a literal
    pub fn base_url(self) -> Url {
        let host = match self {
            Region::Us => "us",
            Region::Eu => "eu",
            Region::Ca => "ca",
            Region::Uk => "uk",
            Region::Nz => "nz",
        };
        Url::parse(&format!("https://{host}.onetimesecret.com/")).expect("static URL")
    }
}

impl fmt::Display for Region {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.base_url().host_str().unwrap_or_default())
    }
}

/// API credentials: who you are and your API token.
///
/// The customer identifier is the **customer extid**: a short, opaque,
/// case-insensitive value beginning with `ur`, shown with a copy button at
/// the bottom of the user menu when signed in. Other identifiers the API
/// uses are not interchangeable with it. The token is the API token from
/// the account page.
///
/// The extid's shape is checked when the client is built, so a value of
/// the wrong kind fails there rather than as an opaque 401 later. This is
/// the same check the other official SDKs make.
#[derive(Clone)]
pub struct Credentials {
    customer: String,
    token: SecretString,
}

impl Credentials {
    /// Credentials for `customer` with `token`.
    pub fn new(customer: impl Into<String>, token: impl Into<SecretString>) -> Self {
        Self {
            customer: customer.into(),
            token: token.into(),
        }
    }

    /// Read [`ENV_CUSTOMER`] and [`ENV_API_TOKEN`]. `Ok(None)` when neither
    /// is set; an error when only one is.
    pub fn from_env() -> Result<Option<Self>> {
        let customer = env_var(ENV_CUSTOMER);
        let token = env_var(ENV_API_TOKEN);
        match (customer, token) {
            (None, None) => Ok(None),
            (Some(customer), Some(token)) => Ok(Some(Self::new(customer, token))),
            (Some(_), None) => Err(Error::Config(format!(
                "{ENV_CUSTOMER} is set but {ENV_API_TOKEN} is not"
            ))),
            (None, Some(_)) => Err(Error::Config(format!(
                "{ENV_API_TOKEN} is set but {ENV_CUSTOMER} is not"
            ))),
        }
    }

    /// The customer identifier.
    pub fn customer(&self) -> &str {
        &self.customer
    }

    fn validate(&self) -> Result<()> {
        if is_customer_extid(&self.customer) {
            return Ok(());
        }
        Err(Error::Config(format!(
            "customer {:?} is not a customer extid: extids begin with \"ur\" (e.g. \"ur1abc23def\"). {CUSTOMER_HINT}",
            self.customer
        )))
    }
}

/// `ur` followed by at least one alphanumeric, case-insensitive.
fn is_customer_extid(value: &str) -> bool {
    let rest = value
        .strip_prefix("ur")
        .or_else(|| value.strip_prefix("UR"))
        .or_else(|| value.strip_prefix("Ur"))
        .or_else(|| value.strip_prefix("uR"));
    rest.is_some_and(|rest| !rest.is_empty() && rest.chars().all(|c| c.is_ascii_alphanumeric()))
}

impl fmt::Debug for Credentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Credentials")
            .field("customer", &self.customer)
            .field("token", &"[REDACTED]")
            .finish()
    }
}

fn env_var(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.trim().is_empty())
}

/// Validated configuration shared by the async and blocking clients.
#[derive(Debug, Clone)]
pub(crate) struct Config {
    pub(crate) context: Context,
    pub(crate) credentials: Option<Credentials>,
    pub(crate) user_agent: String,
    pub(crate) timeout: Duration,
    pub(crate) connect_timeout: Duration,
    pub(crate) https_only: bool,
    pub(crate) max_retries: u32,
}

impl Config {
    /// The absolute URL for a request path.
    pub(crate) fn url_for(&self, path: &str) -> Result<Url> {
        self.context
            .base_url
            .join(&format!("{API_PREFIX}{path}"))
            .map_err(|e| Error::Config(format!("could not build request URL: {e}")))
    }

    pub(crate) fn basic_auth(&self) -> Option<(&str, &SecretString)> {
        self.credentials
            .as_ref()
            .map(|c| (c.customer.as_str(), &c.token))
    }

    /// Only idempotent requests are retried, and only this many times.
    pub(crate) fn may_retry(&self, method: Method, attempt: u32) -> bool {
        method == Method::Get && attempt < self.max_retries
    }

    /// How long to wait before attempt `attempt` (1-based): exponential
    /// backoff, or the server's `Retry-After` when it asks for longer,
    /// capped.
    pub(crate) fn backoff(attempt: u32, retry_after: Option<Duration>) -> Duration {
        let exponential = BACKOFF_BASE.saturating_mul(1 << attempt.saturating_sub(1).min(16));
        retry_after
            .unwrap_or(Duration::ZERO)
            .max(exponential)
            .min(BACKOFF_CAP)
    }
}

/// Statuses worth a retry: throttling and transient server failures.
pub(crate) fn retryable_status(status: u16) -> bool {
    matches!(status, 429 | 500 | 502 | 503 | 504)
}

/// Transport failures worth a retry: anything but a client-side builder
/// or body problem. Timeouts and connection failures are included.
pub(crate) fn retryable_transport_error(error: &reqwest::Error) -> bool {
    !(error.is_builder() || error.is_body() || error.is_decode())
}

/// Configures a [`Client`].
///
/// ```no_run
/// use onetimesecret::{Client, Credentials, Region};
///
/// let client = Client::builder()
///     .region(Region::Us)
///     .credentials(Credentials::new("ur1abc23def", "token"))
///     .build()?;
/// # Ok::<(), onetimesecret::Error>(())
/// ```
#[derive(Debug, Clone)]
#[must_use]
pub struct ClientBuilder {
    base_url: Option<String>,
    credentials: Option<Credentials>,
    user_agent: Option<String>,
    timeout: Duration,
    connect_timeout: Duration,
    https_only: bool,
    max_retries: u32,
}

impl Default for ClientBuilder {
    fn default() -> Self {
        Self {
            base_url: None,
            credentials: None,
            user_agent: None,
            timeout: DEFAULT_TIMEOUT,
            connect_timeout: DEFAULT_CONNECT_TIMEOUT,
            https_only: true,
            max_retries: DEFAULT_MAX_RETRIES,
        }
    }
}

impl ClientBuilder {
    /// A builder with no base URL and no credentials.
    pub fn new() -> Self {
        Self::default()
    }

    /// Talk to one of the hosted regions.
    pub fn region(mut self, region: Region) -> Self {
        self.base_url = Some(region.base_url().into());
        self
    }

    /// Talk to this server: a hosted region, a self-hosted install, or a
    /// custom domain. A path prefix is kept.
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = Some(url.into());
        self
    }

    /// Authenticate every request. Without credentials the client is
    /// anonymous and uses the `/guest/` routes.
    pub fn credentials(mut self, credentials: Credentials) -> Self {
        self.credentials = Some(credentials);
        self
    }

    /// Prefix the `User-Agent` with your application's name and version,
    /// e.g. `my-app/1.2.0`. The SDK's own identification follows it.
    pub fn user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = Some(user_agent.into());
        self
    }

    /// Deadline for a whole request, response body included. Default 30s.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Deadline for establishing the connection. Default 10s.
    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    /// How many times to retry an idempotent (`GET`) request that fails
    /// with a connection error, a timeout, a 429, or a 5xx, with
    /// exponential backoff from 0.5s and respecting `Retry-After`. Default
    /// 2; `0` disables retries. Requests that create, reveal or burn are
    /// never retried.
    pub fn max_retries(mut self, retries: u32) -> Self {
        self.max_retries = retries;
        self
    }

    /// Allow a plain `http://` base URL. Only for local development
    /// against a server on the same machine: credentials and secrets
    /// travel in the clear.
    pub fn allow_http(mut self, allow: bool) -> Self {
        self.https_only = !allow;
        self
    }

    /// Fill unset values from the environment: [`ENV_BASE_URL`],
    /// [`ENV_CUSTOMER`] and [`ENV_API_TOKEN`]. Explicit settings win.
    pub fn from_env(mut self) -> Result<Self> {
        if self.base_url.is_none() {
            self.base_url = env_var(ENV_BASE_URL);
        }
        if self.credentials.is_none() {
            self.credentials = Credentials::from_env()?;
        }
        Ok(self)
    }

    pub(crate) fn config(self) -> Result<Config> {
        let raw = self.base_url.ok_or_else(|| {
            Error::Config(format!(
                "no base URL: call region(), base_url(), or set {ENV_BASE_URL}"
            ))
        })?;
        let base_url = parse_base_url(&raw, self.https_only)?;
        if let Some(credentials) = &self.credentials {
            credentials.validate()?;
        }
        let user_agent = match self.user_agent {
            Some(prefix) => format!("{prefix} {}", default_user_agent()),
            None => default_user_agent(),
        };
        Ok(Config {
            context: Context {
                base_url,
                guest: self.credentials.is_none(),
            },
            credentials: self.credentials,
            user_agent,
            timeout: self.timeout,
            connect_timeout: self.connect_timeout,
            https_only: self.https_only,
            max_retries: self.max_retries,
        })
    }

    /// Build the client.
    ///
    /// Fails when no base URL was given, when the URL is not usable
    /// (`onetimesecret.com` itself is the website, not an API host), or
    /// when the HTTP client cannot be set up.
    pub fn build(self) -> Result<Client> {
        let config = self.config()?;
        let http = reqwest::Client::builder()
            .user_agent(&config.user_agent)
            .timeout(config.timeout)
            .connect_timeout(config.connect_timeout)
            .https_only(config.https_only)
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        Ok(Client {
            inner: Arc::new(Inner { config, http }),
        })
    }
}

/// Names the SDK repository, not the crate: `onetimesecret-rust` tells the
/// service which of the per-language clients is calling.
fn default_user_agent() -> String {
    format!(
        "onetimesecret-rust/{} (Rust/{RUSTC_VERSION})",
        crate::VERSION
    )
}

/// The `X-Onetime-Client` header every official SDK sends:
/// `<language>:<runtime version>/<sdk version>`.
pub(crate) fn client_header() -> String {
    format!("rust:{RUSTC_VERSION}/{}", crate::VERSION)
}

/// Header name for [`client_header`].
pub(crate) const CLIENT_HEADER: &str = "X-Onetime-Client";

/// Hosts that serve the website, not an API.
const APEX_HOSTS: &[&str] = &["onetimesecret.com", "www.onetimesecret.com"];

/// Parse and normalise a base URL so that it always ends in `/`.
pub(crate) fn parse_base_url(raw: &str, https_only: bool) -> Result<Url> {
    let raw = raw.trim();
    let mut url =
        Url::parse(raw).map_err(|e| Error::Config(format!("invalid base URL {raw:?}: {e}")))?;
    match url.scheme() {
        "https" => {}
        "http" if !https_only => {}
        "http" => {
            return Err(Error::Config(format!(
                "base URL {raw:?} is not https; call allow_http(true) for local development only"
            )));
        }
        other => {
            return Err(Error::Config(format!(
                "base URL {raw:?} has scheme {other:?}; expected https"
            )));
        }
    }
    let host = url
        .host_str()
        .ok_or_else(|| Error::Config(format!("base URL {raw:?} has no host")))?
        .to_ascii_lowercase();
    if APEX_HOSTS.contains(&host.as_str()) {
        return Err(Error::Config(format!(
            "{host} is the Onetime Secret website, not an API host; use a region such as \
             https://us.onetimesecret.com, your self-hosted server, or your custom domain"
        )));
    }
    if url.query().is_some() || url.fragment().is_some() {
        return Err(Error::Config(format!(
            "base URL {raw:?} must not have a query or fragment"
        )));
    }
    if !url.path().ends_with('/') {
        let path = format!("{}/", url.path());
        url.set_path(&path);
    }
    Ok(url)
}

struct Inner {
    config: Config,
    http: reqwest::Client,
}

/// An async client for the Onetime Secret API.
///
/// Cheap to clone; clones share one connection pool. Build one with
/// [`Client::builder`] or [`Client::from_env`], then reach the API through
/// [`secrets`](Self::secrets), [`receipts`](Self::receipts) and
/// [`system`](Self::system).
#[derive(Clone)]
pub struct Client {
    inner: Arc<Inner>,
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("base_url", &self.base_url().as_str())
            .field("authenticated", &self.is_authenticated())
            .finish_non_exhaustive()
    }
}

impl Client {
    /// Start configuring a client.
    pub fn builder() -> ClientBuilder {
        ClientBuilder::new()
    }

    /// A client configured entirely from the environment. See
    /// [`ClientBuilder::from_env`].
    pub fn from_env() -> Result<Self> {
        ClientBuilder::new().from_env()?.build()
    }

    /// The server this client talks to.
    pub fn base_url(&self) -> &Url {
        &self.inner.config.context.base_url
    }

    /// `true` when credentials were configured.
    pub fn is_authenticated(&self) -> bool {
        self.inner.config.credentials.is_some()
    }

    /// Operations on secrets: conceal, generate, reveal, status.
    pub fn secrets(&self) -> Secrets<'_> {
        Secrets::new(self)
    }

    /// Operations on receipts: show, list, burn, memo.
    pub fn receipts(&self) -> Receipts<'_> {
        Receipts::new(self)
    }

    /// Server status, version and locales.
    pub fn system(&self) -> System<'_> {
        System::new(self)
    }

    pub(crate) fn context(&self) -> &Context {
        &self.inner.config.context
    }

    pub(crate) async fn call<T>(&self, endpoint: Endpoint<T>) -> Result<T> {
        let response = self.send(&endpoint.request).await?;
        (endpoint.decode)(self.context(), &response)
    }

    async fn send(&self, request: &Request) -> Result<Response> {
        let config = &self.inner.config;
        let url = config.url_for(&request.path)?;
        let mut attempt = 0;
        loop {
            let outcome = self.send_once(request, url.clone()).await;
            let retry_after = match &outcome {
                Ok(response) if retryable_status(response.status) => Some(response.retry_after),
                Err(error) if retryable_transport_error(error) => Some(None),
                _ => None,
            };
            match retry_after {
                Some(retry_after) if config.may_retry(request.method, attempt) => {
                    attempt += 1;
                    tokio::time::sleep(Config::backoff(attempt, retry_after)).await;
                }
                _ => return outcome.map_err(Error::from),
            }
        }
    }

    async fn send_once(&self, request: &Request, url: Url) -> reqwest::Result<Response> {
        let config = &self.inner.config;
        let mut builder = self
            .inner
            .http
            .request(request.method.into(), url)
            .header(reqwest::header::ACCEPT, "application/json")
            .header(CLIENT_HEADER, client_header());
        if let Some((user, token)) = config.basic_auth() {
            builder = builder.basic_auth(user, Some(token.expose_secret()));
        }
        if let Some(body) = &request.body {
            builder = builder
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(body.to_vec());
        }
        let response = builder.send().await?;
        let status = response.status().as_u16();
        let retry_after = response
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|v| v.to_str().ok())
            .and_then(protocol::retry_after);
        let body = Zeroizing::new(response.bytes().await?.to_vec());
        Ok(Response {
            status,
            body,
            retry_after,
        })
    }
}

impl From<Method> for reqwest::Method {
    fn from(method: Method) -> Self {
        match method {
            Method::Get => reqwest::Method::GET,
            Method::Post => reqwest::Method::POST,
            Method::Patch => reqwest::Method::PATCH,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_urls_are_normalised_with_a_trailing_slash() {
        let url = parse_base_url("https://eu.onetimesecret.com", true).unwrap();
        assert_eq!(url.as_str(), "https://eu.onetimesecret.com/");
        let url = parse_base_url("https://secrets.example.com/ots", true).unwrap();
        assert_eq!(url.as_str(), "https://secrets.example.com/ots/");
        assert_eq!(
            url.join("api/v2/secret/conceal").unwrap().as_str(),
            "https://secrets.example.com/ots/api/v2/secret/conceal"
        );
    }

    #[test]
    fn the_website_is_not_an_api_host() {
        let err = parse_base_url("https://onetimesecret.com", true).unwrap_err();
        assert!(matches!(err, Error::Config(msg) if msg.contains("website")));
        assert!(parse_base_url("https://WWW.onetimesecret.com/", true).is_err());
    }

    #[test]
    fn http_needs_opting_in() {
        assert!(parse_base_url("http://localhost:3000", true).is_err());
        assert!(parse_base_url("http://localhost:3000", false).is_ok());
        assert!(parse_base_url("ftp://x.example", false).is_err());
        assert!(parse_base_url("https://x.example/?a=1", true).is_err());
    }

    #[test]
    fn builder_requires_a_base_url() {
        let err = ClientBuilder::new().build().unwrap_err();
        assert!(matches!(err, Error::Config(msg) if msg.contains("base URL")));
    }

    #[test]
    fn regions_resolve() {
        assert_eq!(
            Region::Nz.base_url().as_str(),
            "https://nz.onetimesecret.com/"
        );
        assert_eq!(Region::Eu.to_string(), "eu.onetimesecret.com");
    }

    #[test]
    fn credentials_must_look_like_an_extid() {
        assert!(is_customer_extid("ur1abc23def"));
        assert!(is_customer_extid("UR1ABC"));
        assert!(!is_customer_extid("ur"));
        assert!(!is_customer_extid("acct-123"));
        assert!(!is_customer_extid("user@example.com"));
        let err = ClientBuilder::new()
            .region(Region::Eu)
            .credentials(Credentials::new("acct-123", "tok"))
            .config()
            .unwrap_err();
        let text = err.to_string();
        assert!(text.contains("not a customer extid"), "{text}");
        assert!(text.contains("ONETIME_CUSTOMER_EXTID"), "{text}");
    }

    #[test]
    fn backoff_grows_and_honours_retry_after_within_the_cap() {
        assert_eq!(Config::backoff(1, None), Duration::from_millis(500));
        assert_eq!(Config::backoff(2, None), Duration::from_secs(1));
        assert_eq!(Config::backoff(3, None), Duration::from_secs(2));
        assert_eq!(
            Config::backoff(1, Some(Duration::from_secs(5))),
            Duration::from_secs(5)
        );
        assert_eq!(
            Config::backoff(1, Some(Duration::from_secs(600))),
            BACKOFF_CAP
        );
        assert_eq!(Config::backoff(40, None), BACKOFF_CAP);
        assert!(retryable_status(503));
        assert!(!retryable_status(404));
    }

    #[test]
    fn headers_identify_the_sdk() {
        assert!(default_user_agent().starts_with("onetimesecret-rust/"));
        assert!(default_user_agent().contains("(Rust/"));
        assert!(client_header().starts_with("rust:"));
        assert!(client_header().ends_with(crate::VERSION));
    }

    #[test]
    fn credentials_are_redacted_in_debug() {
        let c = Credentials::new("ur1abc", "tok-secret");
        let text = format!("{c:?}");
        assert!(text.contains("ur1abc"));
        assert!(!text.contains("tok-secret"));
        let client = Client::builder()
            .region(Region::Eu)
            .credentials(c)
            .build()
            .unwrap();
        let text = format!("{client:?}");
        assert!(text.contains("authenticated: true"));
        assert!(!text.contains("tok-secret"));
    }

    #[test]
    fn user_agent_is_prefixed_by_the_application() {
        let config = ClientBuilder::new()
            .region(Region::Eu)
            .user_agent("my-app/1.0")
            .config()
            .unwrap();
        assert!(
            config
                .user_agent
                .starts_with("my-app/1.0 onetimesecret-rust/")
        );
    }
}
