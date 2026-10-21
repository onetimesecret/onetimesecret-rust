//! Resource handles on the async [`Client`].

pub(crate) mod endpoints;

use secrecy::SecretString;

use crate::client::Client;
use crate::error::Result;
use crate::types::{
    ConcealSecret, CreatedSecret, GenerateSecret, Receipt, ReceiptId, RevealedSecret, SecretId,
    SecretMetadata, SecretStatus, SupportedLocales, SystemStatus, SystemVersion,
};

/// Operations on secrets. Obtained from [`Client::secrets`].
#[derive(Debug, Clone, Copy)]
pub struct Secrets<'a> {
    client: &'a Client,
}

impl<'a> Secrets<'a> {
    pub(crate) fn new(client: &'a Client) -> Self {
        Self { client }
    }

    /// Store a value and get back a one-time link for it.
    pub async fn conceal(&self, request: ConcealSecret) -> Result<CreatedSecret> {
        let ep = endpoints::conceal(self.client.context(), &request)?;
        self.client.call(ep).await
    }

    /// Have the server generate a value and get back a one-time link for
    /// it. The value itself is not returned; it is revealed through the
    /// link like any other secret.
    pub async fn generate(&self, request: GenerateSecret) -> Result<CreatedSecret> {
        let ep = endpoints::generate(self.client.context(), &request)?;
        self.client.call(ep).await
    }

    /// Metadata about a secret, without its value.
    ///
    /// This is what the web UI does when someone opens the link: the
    /// server records the secret as previewed.
    pub async fn show(&self, id: impl Into<SecretId>) -> Result<SecretMetadata> {
        let ep = endpoints::show_secret(self.client.context(), &id.into());
        self.client.call(ep).await
    }

    /// A secret's state and remaining lifetime, without touching it.
    ///
    /// A secret the server does not hold (revealed, burned, expired, or
    /// never existed) is not an error: it comes back with
    /// [`SecretStatus::secret`] set to `None`.
    pub async fn status(&self, id: impl Into<SecretId>) -> Result<SecretStatus> {
        let ep = endpoints::secret_status(self.client.context(), &id.into());
        self.client.call(ep).await
    }

    /// The state of several secrets in one request. Secrets that are gone
    /// are simply absent from the result.
    pub async fn status_many<I>(&self, ids: I) -> Result<Vec<SecretMetadata>>
    where
        I: IntoIterator,
        I::Item: Into<SecretId>,
    {
        let ids: Vec<SecretId> = ids.into_iter().map(Into::into).collect();
        let ep = endpoints::secret_status_many(self.client.context(), &ids)?;
        self.client.call(ep).await
    }

    /// Reveal a secret's value. This is the one-time action: the server
    /// destroys the value as it hands it over.
    pub async fn reveal(&self, id: impl Into<SecretId>) -> Result<RevealedSecret> {
        let ep = endpoints::reveal(self.client.context(), &id.into(), None)?;
        self.client.call(ep).await
    }

    /// Reveal a passphrase-protected secret. A wrong passphrase is an
    /// [`ApiErrorKind::Invalid`](crate::ApiErrorKind::Invalid) error and
    /// does not consume the secret.
    pub async fn reveal_with_passphrase(
        &self,
        id: impl Into<SecretId>,
        passphrase: impl Into<SecretString>,
    ) -> Result<RevealedSecret> {
        let ep = endpoints::reveal(self.client.context(), &id.into(), Some(&passphrase.into()))?;
        self.client.call(ep).await
    }
}

/// Operations on receipts. Obtained from [`Client::receipts`].
#[derive(Debug, Clone, Copy)]
pub struct Receipts<'a> {
    client: &'a Client,
}

impl<'a> Receipts<'a> {
    pub(crate) fn new(client: &'a Client) -> Self {
        Self { client }
    }

    /// A receipt with its links and expiration.
    pub async fn show(&self, id: impl Into<ReceiptId>) -> Result<Receipt> {
        let ep = endpoints::show_receipt(self.client.context(), &id.into());
        self.client.call(ep).await
    }

    /// The account's receipts from the last 30 days, most recent first.
    /// Requires credentials.
    pub async fn recent(&self) -> Result<Vec<Receipt>> {
        let ep = endpoints::recent_receipts(self.client.context());
        self.client.call(ep).await
    }

    /// Destroy the secret before anyone reveals it.
    pub async fn burn(&self, id: impl Into<ReceiptId>) -> Result<Receipt> {
        let ep = endpoints::burn(self.client.context(), &id.into(), None)?;
        self.client.call(ep).await
    }

    /// Destroy a passphrase-protected secret before anyone reveals it.
    pub async fn burn_with_passphrase(
        &self,
        id: impl Into<ReceiptId>,
        passphrase: impl Into<SecretString>,
    ) -> Result<Receipt> {
        let ep = endpoints::burn(self.client.context(), &id.into(), Some(&passphrase.into()))?;
        self.client.call(ep).await
    }

    /// Set the creator's private note on a receipt (up to 500
    /// characters). Requires credentials, and the receipt must be yours.
    pub async fn update_memo(&self, id: impl Into<ReceiptId>, memo: &str) -> Result<Receipt> {
        let ep = endpoints::update_memo(self.client.context(), &id.into(), memo)?;
        self.client.call(ep).await
    }
}

/// Server information. Obtained from [`Client::system`].
#[derive(Debug, Clone, Copy)]
pub struct System<'a> {
    client: &'a Client,
}

impl<'a> System<'a> {
    pub(crate) fn new(client: &'a Client) -> Self {
        Self { client }
    }

    /// Whether the server is up. A cheap connectivity check.
    pub async fn status(&self) -> Result<SystemStatus> {
        self.client
            .call(endpoints::system_status(self.client.context()))
            .await
    }

    /// The server's version. Requires credentials.
    pub async fn version(&self) -> Result<SystemVersion> {
        self.client
            .call(endpoints::system_version(self.client.context()))
            .await
    }

    /// The locales the server can respond in.
    pub async fn supported_locales(&self) -> Result<SupportedLocales> {
        self.client
            .call(endpoints::supported_locales(self.client.context()))
            .await
    }
}
