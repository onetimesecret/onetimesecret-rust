//! A synchronous client, for programs without an async runtime.
//!
//! Mirrors the async API method for method. Enabled with the `blocking`
//! feature.
//!
//! ```no_run
//! use onetimesecret::blocking::Client;
//! use onetimesecret::{ConcealSecret, Region};
//!
//! let client = Client::builder().region(Region::Eu).build()?;
//! let created = client.secrets().conceal(ConcealSecret::new("psst"))?;
//! println!("{}", created.share_url);
//! # Ok::<(), onetimesecret::Error>(())
//! ```
//!
//! Do not call this client from inside an async runtime; it blocks the
//! thread. Use the async [`Client`](crate::Client) there.

use std::fmt;
use std::sync::Arc;

use secrecy::{ExposeSecret, SecretString};
use url::Url;
use zeroize::Zeroizing;

use crate::api::endpoints::{self, Context, Endpoint};
use crate::client::{
    CLIENT_HEADER, ClientBuilder, Config, LOOPBACK_ADDRS, client_header, http_client_setup_error,
    loopback_pin, retryable_status, retryable_transport_error,
};
use crate::error::Result;
use crate::protocol::{self, Request, Response};
use crate::types::{
    ConcealSecret, CreatedSecret, GenerateSecret, Receipt, ReceiptId, RevealedSecret, SecretId,
    SecretMetadata, SecretStatus, SupportedLocales, SystemStatus, SystemVersion,
};

impl ClientBuilder {
    /// Build a blocking client from the same configuration.
    pub fn build_blocking(self) -> Result<Client> {
        let config = self.config()?;
        let mut http = reqwest::blocking::Client::builder()
            .user_agent(&config.user_agent)
            .timeout(config.timeout)
            .connect_timeout(config.connect_timeout)
            .https_only(config.https_only)
            .redirect(reqwest::redirect::Policy::none());
        if let Some(host) = loopback_pin(&config.context.base_url) {
            http = http.resolve_to_addrs(host, &LOOPBACK_ADDRS);
        }
        let http = http.build().map_err(|e| http_client_setup_error(&e))?;
        Ok(Client {
            inner: Arc::new(Inner { config, http }),
        })
    }
}

struct Inner {
    config: Config,
    http: reqwest::blocking::Client,
}

/// A blocking client for the Onetime Secret API. See the
/// [module docs](self).
#[derive(Clone)]
pub struct Client {
    inner: Arc<Inner>,
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("blocking::Client")
            .field("base_url", &self.base_url().as_str())
            .field("authenticated", &self.is_authenticated())
            .finish_non_exhaustive()
    }
}

impl Client {
    /// Start configuring a client. Finish with
    /// [`ClientBuilder::build_blocking`].
    pub fn builder() -> Builder {
        Builder(ClientBuilder::new())
    }

    /// A client configured entirely from the environment. See
    /// [`ClientBuilder::from_env`].
    pub fn from_env() -> Result<Self> {
        ClientBuilder::new().from_env()?.build_blocking()
    }

    /// The server this client talks to.
    pub fn base_url(&self) -> &Url {
        &self.inner.config.context.base_url
    }

    /// `true` when credentials were configured.
    pub fn is_authenticated(&self) -> bool {
        self.inner.config.credentials.is_some()
    }

    /// Operations on secrets.
    pub fn secrets(&self) -> Secrets<'_> {
        Secrets { client: self }
    }

    /// Operations on receipts.
    pub fn receipts(&self) -> Receipts<'_> {
        Receipts { client: self }
    }

    /// Server status, version and locales.
    pub fn system(&self) -> System<'_> {
        System { client: self }
    }

    fn context(&self) -> &Context {
        &self.inner.config.context
    }

    fn call<T>(&self, endpoint: &Endpoint<T>) -> Result<T> {
        let response = self.send(&endpoint.request)?;
        (endpoint.decode)(self.context(), &response)
    }

    fn send(&self, request: &Request) -> Result<Response> {
        let config = &self.inner.config;
        let url = config.url_for(&request.path)?;
        let mut attempt = 0;
        loop {
            let outcome = self.send_once(request, url.clone());
            let retry_after = match &outcome {
                Ok(response) if retryable_status(response.status) => Some(response.retry_after),
                Err(error) if retryable_transport_error(error) => Some(None),
                _ => None,
            };
            match retry_after {
                Some(retry_after) if config.may_retry(request.method, attempt) => {
                    attempt += 1;
                    std::thread::sleep(Config::backoff(attempt, retry_after));
                }
                // Without the URL: a request path can name a secret.
                _ => return outcome.map_err(|e| crate::Error::Transport(e.without_url())),
            }
        }
    }

    fn send_once(&self, request: &Request, url: Url) -> reqwest::Result<Response> {
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
        let response = builder.send()?;
        let status = response.status().as_u16();
        let retry_after = response
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|v| v.to_str().ok())
            .and_then(protocol::retry_after);
        let body = Zeroizing::new(response.bytes()?.to_vec());
        Ok(Response {
            status,
            body,
            retry_after,
        })
    }
}

/// A [`ClientBuilder`] that finishes with a blocking client.
#[derive(Debug, Clone, Default)]
#[must_use]
pub struct Builder(ClientBuilder);

impl Builder {
    /// See [`ClientBuilder::region`].
    pub fn region(self, region: crate::Region) -> Self {
        Self(self.0.region(region))
    }

    /// See [`ClientBuilder::base_url`].
    pub fn base_url(self, url: impl Into<String>) -> Self {
        Self(self.0.base_url(url))
    }

    /// See [`ClientBuilder::credentials`].
    pub fn credentials(self, credentials: crate::Credentials) -> Self {
        Self(self.0.credentials(credentials))
    }

    /// See [`ClientBuilder::user_agent`].
    pub fn user_agent(self, user_agent: impl Into<String>) -> Self {
        Self(self.0.user_agent(user_agent))
    }

    /// See [`ClientBuilder::timeout`].
    pub fn timeout(self, timeout: std::time::Duration) -> Self {
        Self(self.0.timeout(timeout))
    }

    /// See [`ClientBuilder::connect_timeout`].
    pub fn connect_timeout(self, timeout: std::time::Duration) -> Self {
        Self(self.0.connect_timeout(timeout))
    }

    /// See [`ClientBuilder::max_retries`].
    pub fn max_retries(self, retries: u32) -> Self {
        Self(self.0.max_retries(retries))
    }

    /// See [`ClientBuilder::allow_http`].
    pub fn allow_http(self, allow: bool) -> Self {
        Self(self.0.allow_http(allow))
    }

    /// See [`ClientBuilder::from_env`].
    pub fn from_env(self) -> Result<Self> {
        Ok(Self(self.0.from_env()?))
    }

    /// Build the blocking client.
    pub fn build(self) -> Result<Client> {
        self.0.build_blocking()
    }
}

/// Operations on secrets. See [`crate::Secrets`] for details.
#[derive(Debug, Clone, Copy)]
pub struct Secrets<'a> {
    client: &'a Client,
}

impl Secrets<'_> {
    /// See [`crate::Secrets::conceal`].
    #[allow(clippy::needless_pass_by_value)] // same signature as the async client
    pub fn conceal(&self, request: ConcealSecret) -> Result<CreatedSecret> {
        self.client
            .call(&endpoints::conceal(self.client.context(), &request)?)
    }

    /// See [`crate::Secrets::generate`].
    #[allow(clippy::needless_pass_by_value)] // same signature as the async client
    pub fn generate(&self, request: GenerateSecret) -> Result<CreatedSecret> {
        self.client
            .call(&endpoints::generate(self.client.context(), &request)?)
    }

    /// See [`crate::Secrets::show`].
    pub fn show(&self, id: impl Into<SecretId>) -> Result<SecretMetadata> {
        self.client
            .call(&endpoints::show_secret(self.client.context(), &id.into()))
    }

    /// See [`crate::Secrets::status`].
    pub fn status(&self, id: impl Into<SecretId>) -> Result<SecretStatus> {
        self.client
            .call(&endpoints::secret_status(self.client.context(), &id.into()))
    }

    /// See [`crate::Secrets::status_many`].
    pub fn status_many<I>(&self, ids: I) -> Result<Vec<SecretMetadata>>
    where
        I: IntoIterator,
        I::Item: Into<SecretId>,
    {
        let ids: Vec<SecretId> = ids.into_iter().map(Into::into).collect();
        self.client
            .call(&endpoints::secret_status_many(self.client.context(), &ids)?)
    }

    /// See [`crate::Secrets::reveal`].
    pub fn reveal(&self, id: impl Into<SecretId>) -> Result<RevealedSecret> {
        self.client
            .call(&endpoints::reveal(self.client.context(), &id.into(), None)?)
    }

    /// See [`crate::Secrets::reveal_with_passphrase`].
    pub fn reveal_with_passphrase(
        &self,
        id: impl Into<SecretId>,
        passphrase: impl Into<SecretString>,
    ) -> Result<RevealedSecret> {
        self.client.call(&endpoints::reveal(
            self.client.context(),
            &id.into(),
            Some(&passphrase.into()),
        )?)
    }
}

/// Operations on receipts. See [`crate::Receipts`] for details.
#[derive(Debug, Clone, Copy)]
pub struct Receipts<'a> {
    client: &'a Client,
}

impl Receipts<'_> {
    /// See [`crate::Receipts::show`].
    pub fn show(&self, id: impl Into<ReceiptId>) -> Result<Receipt> {
        self.client
            .call(&endpoints::show_receipt(self.client.context(), &id.into()))
    }

    /// See [`crate::Receipts::recent`].
    pub fn recent(&self) -> Result<Vec<Receipt>> {
        self.client
            .call(&endpoints::recent_receipts(self.client.context()))
    }

    /// See [`crate::Receipts::burn`].
    pub fn burn(&self, id: impl Into<ReceiptId>) -> Result<Receipt> {
        self.client
            .call(&endpoints::burn(self.client.context(), &id.into(), None)?)
    }

    /// See [`crate::Receipts::burn_with_passphrase`].
    pub fn burn_with_passphrase(
        &self,
        id: impl Into<ReceiptId>,
        passphrase: impl Into<SecretString>,
    ) -> Result<Receipt> {
        self.client.call(&endpoints::burn(
            self.client.context(),
            &id.into(),
            Some(&passphrase.into()),
        )?)
    }

    /// See [`crate::Receipts::update_memo`].
    pub fn update_memo(&self, id: impl Into<ReceiptId>, memo: &str) -> Result<Receipt> {
        self.client.call(&endpoints::update_memo(
            self.client.context(),
            &id.into(),
            memo,
        )?)
    }
}

/// Server information. See [`crate::System`] for details.
#[derive(Debug, Clone, Copy)]
pub struct System<'a> {
    client: &'a Client,
}

impl System<'_> {
    /// See [`crate::System::status`].
    pub fn status(&self) -> Result<SystemStatus> {
        self.client
            .call(&endpoints::system_status(self.client.context()))
    }

    /// See [`crate::System::version`].
    pub fn version(&self) -> Result<SystemVersion> {
        self.client
            .call(&endpoints::system_version(self.client.context()))
    }

    /// See [`crate::System::supported_locales`].
    pub fn supported_locales(&self) -> Result<SupportedLocales> {
        self.client
            .call(&endpoints::supported_locales(self.client.context()))
    }
}
