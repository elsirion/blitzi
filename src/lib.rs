//! Easy to use Bitcoin Lighning client that uses [Fedimint](https://fedimint.org/) as its backend.
//!
//! You want to build a lightning powered app but don't look forward to dealing
//! with the complexity of running your own Lightning node? Blitzi is for you!
//! With Blitzi you can outsource the infrastructure to any Fedimint federation
//! of your choosing (or just go with the default for small amounts) and receive
//! and send Lightning payments without any hassle.
//!
//! # Examples
//! The fastest way to get started is to create a new Blitzi client with default
//! settings. This is only advisable for small amounts since it will use a
//! default Fedimint federation which the author of this library trusts, but
//! ultimately can't guarantee the security of. For larger amounts we recommend
//! making your own choice which federation to use based on your own due
//! diligence.
//!
//! ```no_run
//! # use anyhow::Result;
//! use blitzi::Blitzi;
//!
//! # #[tokio::main]
//! # async fn main() -> Result<()> {
//! // Create a new Blitzi client with default settings
//! let blitzi = Blitzi::new().await?;
//!
//! // Generate a new Lightning invoice for 1000 millisatoshi and await its payment
//! let invoice = blitzi.lightning_invoice(1000, "Test payment").await?;
//! println!("Invoice: {}", invoice);
//!
//! match blitzi.await_incoming_payment(&invoice).await {
//!     Ok(()) => println!("Payment received"),
//!     Err(_) => println!("Invoice expired"),
//! }
//!
//! # Ok(())
//! # }
//! ```
//!
//! # Fedimint
//! Blitzi uses Fedimint, an open source federated ecash mint implementation on
//! Bitcoin, to connect you to the Lighning network. Federated in this context
//! means that each federation is run by a group of people, also called
//! guardians, who are jointly responsible for the security of the funds held in
//! the federation. This means, while no signle guardian can steal your funds,
//! if a majority of the guardians are compromised, the funds are at risk, so
//! chose your federation wisely.
//!
//! The default federation used by Blitzi is [E-Cash Club], which for various
//! reasons seems the most reasonable choice at the time of writing (long run
//! time, multiple ASNs, etc.). For anything but toy amounts users should make
//! their own choice though. You can find a list of publicly known federations
//! on [Fedimint Observer], which also provices statistics and uptime statistics
//! about them.
//!
//! [E-Cash Club]: (https://observer.fedimint.org/federations/aeca6cc80ffc530bd2d54b09681f6edb9a415c362e4af2fe3d5e04137006fa21)
//! [Fedimint Observer]: (https://observer.fedimint.org/)
//!
//! # About the name
//!
//! Lightning bolts are called "Blitz" in German and adding an "i" at the end
//! makes it sound cute and wholesome for me :D
use std::path::PathBuf;
use std::str::FromStr;

use anyhow::{Context, anyhow, ensure};
use fedimint_bip39::{Bip39RootSecretStrategy, Mnemonic};
use fedimint_client::meta::MetaService;
use fedimint_client::module::meta::LegacyMetaSource;
use fedimint_client::secret::RootSecretStrategy;
use fedimint_client::{Client, ClientHandle, ClientModuleInstance, RootSecret};
use fedimint_core::bitcoin::hashes::sha256;
use fedimint_core::core::OperationId;
use fedimint_core::db::{Database, IRawDatabaseExt};
use fedimint_core::invite_code::InviteCode;
use fedimint_core::util::BoxStream;
use fedimint_core::{BitcoinHash, anyhow, hex};
use fedimint_ln_client::{
    LightningClientInit, LightningClientModule, LightningOperationMeta, LightningOperationMetaPay,
    LightningOperationMetaVariant, LnReceiveState, PayType,
};
use fedimint_meta_client::MetaModuleMetaSourceWithFallback;
use fedimint_mint_client::MintClientInit;
use futures_lite::stream::StreamExt;
use lightning_invoice::{Bolt11Invoice, Bolt11InvoiceDescription, Description};

const ECASH_CLUB_INVITE: &str = "fed11qgqzggnhwden5te0v9cxjtn9vd3jue3wvfkxjmnyva6kzunyd9skutnwv46z7qqpyzhv5mxgpl79xz7j649sj6qldmde5s2uxchy4uh7840qgymsqmazzp6sn43";

/// Utility type for amounts in millisatoshi reexported from fedimint-core.
pub use fedimint_core::Amount;
/// Utility module for parsing lightning invoices reexported from
/// lightning-invoice.
pub use lightning_invoice;

/// Builder for the Blitzi client that allows configuring the fedimint client's
/// settings.
///
/// ```no_run
/// # use anyhow::Result;
/// use blitzi::Blitzi;
///
/// # #[tokio::main]
/// # async fn main() -> Result<()> {
/// let blitzi = Blitzi::builder()
///     .datadir("/path/to/data")
///     .federation("fed11qgqzggnhwden5te0v9cxjtn9vd3jue3wvfkxjmnyva6kzunyd9skutnwv46z7qqpyzhv5mxgpl79xz7j649sj6qldmde5s2uxchy4uh743")?
///     .build()
///     .await?;
/// # Ok(())
/// # }
/// ```
pub struct BlitziBuilder {
    datadir: PathBuf,
    federation: InviteCode,
}

impl Default for BlitziBuilder {
    fn default() -> Self {
        let xdg = xdg::BaseDirectories::new();

        Self {
            datadir: xdg
                .data_home
                .expect("Could not determine XDG data home")
                .join("fedimint/default"),
            federation: InviteCode::from_str(ECASH_CLUB_INVITE).expect("can be parsed"),
        }
    }
}

impl BlitziBuilder {
    /// Sets the directory where Fedimint data will be stored. Defaults to
    /// `$XDG_DATA_HOME/fedimint/default`
    pub fn datadir(mut self, path: impl Into<PathBuf>) -> Self {
        self.datadir = path.into();
        self
    }

    /// Sets the federation to connect to via an already parsed invite code. If
    /// you have a string invite code, use [`Self::federation`] instead.
    pub fn federation_invite(mut self, invite: InviteCode) -> Self {
        self.federation = invite;
        self
    }

    /// Sets the federation to connect to via an invite code string. If you
    /// already have a parsed invite code, use [`Self::federation_invite`]
    /// instead.
    pub fn federation(mut self, invite: &str) -> anyhow::Result<Self> {
        let invite = InviteCode::from_str(invite)?;
        self.federation = invite;
        Ok(self)
    }

    /// Builds the Blitzi client.
    ///
    /// This function will open the existing Fedimint client or join the
    /// federation depending on whether the client has already been initialized.
    ///
    /// # Errors
    /// Returns an error if the database cannot be opened or if joining the
    /// federation fails.
    pub async fn build(self) -> anyhow::Result<Blitzi> {
        let mut client_builder = fedimint_client::Client::builder().await?;
        client_builder.with_module(MintClientInit);
        client_builder.with_module(LightningClientInit::default());
        let mut client_builder = client_builder.with_iroh_enable_next(false);
        client_builder.with_meta_service(MetaService::new(MetaModuleMetaSourceWithFallback::<
            LegacyMetaSource,
        >::default()));

        let db = fedimint_rocksdb::RocksDb::open(self.datadir)
            .await?
            .into_database();

        // TODO: use config being present to decide if to open or join
        let client = if let Some(root_secret) = try_load_root_secret(&db).await? {
            client_builder.open(db, root_secret).await?
        } else {
            let root_secret = generate_root_secret(&db).await?;
            client_builder
                .preview(&self.federation)
                .await?
                .join(db, root_secret)
                .await?
        };

        Ok(Blitzi { client })
    }
}

async fn try_load_root_secret(db: &Database) -> anyhow::Result<Option<RootSecret>> {
    let Some(entropy) = Client::load_decodable_client_secret_opt::<Vec<u8>>(db).await? else {
        return Ok(None);
    };

    let mnemonic = Mnemonic::from_entropy(&entropy)?;

    Ok(Some(RootSecret::StandardDoubleDerive(
        Bip39RootSecretStrategy::<12>::to_root_secret(&mnemonic),
    )))
}

async fn generate_root_secret(db: &Database) -> anyhow::Result<RootSecret> {
    let mnemonic = Mnemonic::generate(12)?;
    let entropy = mnemonic.to_entropy();

    Client::store_encodable_client_secret(db, &entropy).await?;

    Ok(RootSecret::StandardDoubleDerive(Bip39RootSecretStrategy::<
        12,
    >::to_root_secret(
        &mnemonic
    )))
}

/// The Blitzi client that allows paying and receiving payments on Lightning.
///
/// ```no_run
/// # use anyhow::Result;
/// # use fedimint_core::hex;
/// use blitzi::Blitzi;
///
/// # #[tokio::main]
/// # async fn main() -> Result<()> {
/// let blitzi = Blitzi::new().await?;
///
/// // Amount is in millisatoshi
/// let invoice = blitzi.lightning_invoice(1000, "Test payment").await?;
/// println!("Invoice: {}", invoice);
///
/// let preimage = blitzi.pay(&invoice).await?;
/// println!("Preimage: {}", hex::encode(preimage));
///
/// # Ok(())
/// # }
/// ```
pub struct Blitzi {
    client: ClientHandle,
}

impl Blitzi {
    /// Creates a new Blitzi client with default settings.
    pub async fn new() -> anyhow::Result<Self> {
        Self::builder().build().await
    }

    /// Creates a new Blitzi builder for more control. If you just want to go
    /// with the defaults use [`Blitzi::new`] instead.
    pub fn builder() -> BlitziBuilder {
        BlitziBuilder::default()
    }

    fn ln_module(&self) -> ClientModuleInstance<'_, LightningClientModule> {
        self.client
            .get_first_module::<LightningClientModule>()
            .expect("LN module not found")
    }

    /// Returns the current balance held by Blitzi.
    ///
    /// If you want to be notified when the balance changes, use
    /// [`Self::subscribe_balance_changes`] instead.
    pub async fn balance(&self) -> Amount {
        self.client
            .get_balance()
            .await
            .expect("Primary module not available, should not happen")
    }

    /// Returns a stream that yields the current balance every time it changes.
    /// Returns the balance in millisatoshi right away.
    pub async fn subscribe_balance_changes(&self) -> BoxStream<'static, Amount> {
        self.client.subscribe_balance_changes().await
    }

    /// Generates a new Lightning invoice for the given `amount` (up to milli
    /// satoshi precision) containing the given `description`.
    ///
    /// # Errors
    /// Returns an error if no LN gateway is available or if the invoice cannot
    /// be generated for any other reason.
    pub async fn lightning_invoice(
        &self,
        amount: Amount,
        description: &str,
    ) -> anyhow::Result<Bolt11Invoice> {
        let ln_client = self.ln_module();

        let ln_gateway = ln_client
            .get_gateway(None, false)
            .await?
            .ok_or_else(|| anyhow!("No LN gateway available"))?;
        let (_, invoice, _) = ln_client
            .create_bolt11_invoice(
                amount,
                Bolt11InvoiceDescription::Direct(Description::new(description.into())?),
                None,
                (),
                Some(ln_gateway),
            )
            .await?;

        Ok(invoice)
    }

    /// Waits for an invoice generated using [`Self::lightning_invoice`] to be
    /// paid.
    ///
    /// Returns an error in case it times out. There is no need to call this
    /// function unless you need to know if an invoice was paid. The funds will
    /// be received either way.
    pub async fn await_incoming_payment(&self, invoice: &Bolt11Invoice) -> anyhow::Result<()> {
        self.await_incoming_payment_by_hash(invoice.payment_hash())
            .await
    }

    /// Waits for an invoice generated using [`Self::lightning_invoice`] to be
    /// paid. See [`Self::await_incoming_payment`] for more details.
    pub async fn await_incoming_payment_by_hash(
        &self,
        payment_hash: &sha256::Hash,
    ) -> anyhow::Result<()> {
        let operation_id = OperationId(*payment_hash.as_ref());

        let operation = self
            .client
            .operation_log()
            .get_operation(operation_id)
            .await
            .context(
                "No operation found for payment hash, was the invoice issued by us?".to_string(),
            )?;
        ensure!(
            operation.operation_module_kind() == "ln",
            "Operation associated with payment hash is not an LN operation"
        );

        let operation_meta = operation.meta::<LightningOperationMeta>();
        ensure!(
            matches!(
                operation_meta.variant,
                LightningOperationMetaVariant::Receive { .. }
            ),
            "Operation associated with the payment hash is not an incoming payment"
        );

        let ln_module = self.ln_module();
        let mut update_stream = ln_module
            .subscribe_ln_receive(operation_id)
            .await
            .context("Unexpected error subscribing to operation")?
            .into_stream();
        while let Some(update) = update_stream.next().await {
            match update {
                LnReceiveState::Canceled { reason } => {
                    return Err(anyhow!("Payment was canceled: {}", reason));
                }
                LnReceiveState::Claimed => {
                    return Ok(());
                }
                _ => {}
            }
        }

        unreachable!("Stream ended unexpectedly");
    }

    /// Pays an invoice and returns the preimage of the payment.
    ///
    /// If an payment was already made to the same invoice, the result of the
    /// previous payment will be returned again. This allows building safe retry
    /// logic that just tries to pay an invoice again if it's unclear if a
    /// previous call to this function succeeded or not (e.g. in the case of a
    /// crash).
    ///
    /// Retries are not supported for now since they will likely fail too if the
    /// original attempt failed and would add additional complexity.
    pub async fn pay(&self, invoice: &Bolt11Invoice) -> anyhow::Result<[u8; 32]> {
        let ln_client = self.ln_module();
        let operation_id = Self::get_payment_operation_id(invoice.payment_hash());
        let pay_type = if let Some(operation) = self
            .client
            .operation_log()
            .get_operation(operation_id)
            .await
        {
            match operation.meta::<LightningOperationMeta>().variant {
                LightningOperationMetaVariant::Pay(LightningOperationMetaPay {
                    is_internal_payment,
                    ..
                }) => {
                    if is_internal_payment {
                        PayType::Internal(operation_id)
                    } else {
                        PayType::Lightning(operation_id)
                    }
                }
                _ => {
                    return Err(anyhow!(
                        "Operation associated with the payment hash is not an incoming payment"
                    ));
                }
            }
        } else {
            let ln_gateway = ln_client
                .get_gateway(None, false)
                .await?
                .ok_or_else(|| anyhow!("No LN gateway available"))?;

            let payment = ln_client
                .pay_bolt11_invoice(Some(ln_gateway), invoice.clone(), ())
                .await?;
            payment.payment_type
        };

        let preimage = match pay_type {
            PayType::Internal(operation_id) => {
                match ln_client
                    .subscribe_internal_pay(operation_id)
                    .await?
                    .await_outcome()
                    .await
                    .context("No outcome found for payment, should never happen")?
                {
                    fedimint_ln_client::InternalPayState::Preimage(preimage) => preimage.0,
                    state => return Err(anyhow!("Payment failed: {:?}", state)),
                }
            }
            PayType::Lightning(operation_id) => {
                match ln_client
                    .subscribe_ln_pay(operation_id)
                    .await?
                    .await_outcome()
                    .await
                    .context("No outcome found for payment, should never happen")?
                {
                    fedimint_ln_client::LnPayState::Success { preimage } => hex::decode(preimage)
                        .context("Invalid preimage")?
                        .try_into()
                        .ok()
                        .context("Invalid preimage length")?,
                    state => return Err(anyhow!("Payment failed: {:?}", state)),
                }
            }
        };

        Ok(preimage)
    }

    fn get_payment_operation_id(payment_hash: &sha256::Hash) -> OperationId {
        // Copied from fedimint-ln-client
        fn get_payment_operation_id(payment_hash: &sha256::Hash, index: u16) -> OperationId {
            // Copy the 32 byte payment hash and a 2 byte index to make every payment
            // attempt have a unique `OperationId`
            let mut bytes = [0; 34];
            bytes[0..32].copy_from_slice(&payment_hash.to_byte_array());
            bytes[32..34].copy_from_slice(&index.to_le_bytes());
            let hash: sha256::Hash = BitcoinHash::hash(&bytes);
            OperationId(hash.to_byte_array())
        }

        get_payment_operation_id(payment_hash, 0)
    }
}
