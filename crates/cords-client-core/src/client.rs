//! Native client orchestration shared by the CLI and Tauri.
use anyhow::{Context as _, Result, bail, ensure};
use cords_crypto::{
    conversation::{ConversationCrypto, Processed},
    protection,
};
use cords_identity::{Identity, validate_contact, validate_transition};
use cords_protocol::{
    ServerOrigin, SignedServerMetadataV1,
    messaging::{
        Challenge, ChallengeRequest, Channel, ChannelBind, ChannelCreate, ChannelReplace,
        ChannelSuccession, CommitUpload, ConfidentialityMode, Contact, DepartureProof,
        DepartureReceipt, DepartureRequest, DeviceAuthorization, EventUpload, GroupBinding,
        IdentityBurn, KeyPackageUpload, MAX_TEXT, MembershipRequest, Message, OwnershipClaim,
        OwnershipProof, OwnershipRecoveryClaim, OwnershipRecoveryProof, OwnershipState,
        PolicyRemoval, PublicMessage, RevocationRequest, RosterAction, RosterOperation,
        RosterRequest, RouteEvent, ServerDeparture, Session, SessionRequest, Signed,
        SuccessorAcceptance, SuccessorDesignation, SuccessorRequest, WS_PROTOCOL, canonical,
        decode, encode, hash,
    },
};
use cords_storage::SqliteStore;
use futures_util::{SinkExt as _, StreamExt as _};
use rustls::pki_types::pem::PemObject as _;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sqlx::Row as _;
use std::{
    collections::BTreeMap,
    fs::{File, OpenOptions},
    path::Path,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio_tungstenite::{
    Connector, connect_async_tls_with_config,
    tungstenite::{Message as SocketMessage, client::IntoClientRequest as _},
};
use zeroize::Zeroize as _;
use zeroize::Zeroizing;

#[derive(Default, Serialize, Deserialize)]
struct Durable {
    #[serde(default)]
    ui_preferences: serde_json::Value,
    #[serde(default)]
    revoked: bool,
    identity: String,
    crypto: String,
    origin: String,
    server_id: String,
    server_key: String,
    #[serde(default)]
    ownership_state: String,
    #[serde(default)]
    ownership_generation: u64,
    #[serde(default)]
    admission_state: String,
    #[serde(default)]
    join_policy: Vec<String>,
    session: Option<Session>,
    cursors: BTreeMap<String, u64>,
    contacts: BTreeMap<String, Signed<DeviceAuthorization>>,
    pending: Option<Pending>,
    own_events: BTreeMap<String, String>,
    last_upload: Option<EventUpload>,
    #[serde(default)]
    reserved_roster: Option<(String, RosterOperation)>,
    #[serde(default)]
    servers: BTreeMap<String, ServerSnapshot>,
    #[serde(default)]
    archived_servers: BTreeMap<String, ArchivedServer>,
    #[serde(default)]
    pending_burns: BTreeMap<String, PendingBurn>,
    #[serde(default)]
    pending_departures: BTreeMap<String, DepartureRequest<ServerDeparture>>,
    #[serde(default)]
    burned: bool,
    #[serde(default)]
    channel_identities: BTreeMap<String, LocalChannel>,
}
#[derive(Clone, Serialize, Deserialize)]
struct LocalChannel {
    channel: Channel,
    acknowledged: bool,
    #[serde(default)]
    archived: bool,
    #[serde(default)]
    retained_history: Vec<Message>,
}
#[derive(Clone, Serialize, Deserialize)]
struct ServerSnapshot {
    origin: String,
    server_id: String,
    server_key: String,
    ownership_state: String,
    #[serde(default)]
    ownership_generation: u64,
    #[serde(default)]
    admission_state: String,
    join_policy: Vec<String>,
    session: Option<Session>,
    cursors: BTreeMap<String, u64>,
    contacts: BTreeMap<String, Signed<DeviceAuthorization>>,
    pending: Option<Pending>,
    own_events: BTreeMap<String, String>,
    last_upload: Option<EventUpload>,
    reserved_roster: Option<(String, RosterOperation)>,
    crypto: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ArchivedServer {
    pub origin: String,
    pub server_id: String,
    pub archived_at: u64,
    pub remote_drop_confirmed: bool,
}
#[derive(Clone, Serialize, Deserialize)]
struct PendingBurn {
    origin: String,
    server_key: String,
    record: Signed<cords_protocol::messaging::IdentityBurn>,
    idempotency_key: String,
    #[serde(default)]
    attempts: u32,
    #[serde(default)]
    next_retry_at: u64,
    #[serde(default)]
    last_error: Option<String>,
}
#[derive(Clone, Debug, Serialize)]
pub struct ServerListing {
    pub origin: String,
    pub server_id: String,
    pub ownership_state: String,
    pub active: bool,
    pub archived: bool,
    pub remote_drop_confirmed: bool,
}
#[derive(Clone, Debug, Serialize)]
pub struct DepartureOutcome {
    pub remote_confirmed: bool,
    pub warning: Option<String>,
}
#[derive(Clone, Debug, Serialize)]
pub struct BurnOutcome {
    pub confirmed: usize,
    pub pending: Vec<String>,
}
impl Drop for Durable {
    fn drop(&mut self) {
        self.identity.zeroize();
        self.crypto.zeroize();
        if let Some(session) = self.session.as_mut() {
            session.token.zeroize();
        }
        for server in self.servers.values_mut() {
            server.crypto.zeroize();
            if let Some(session) = server.session.as_mut() {
                session.token.zeroize();
            }
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
struct Pending {
    path: String,
    body: serde_json::Value,
    kind: String,
    route: String,
}

pub struct Client {
    store: SqliteStore,
    key: Zeroizing<[u8; 32]>,
    installation: String,
    durable: Durable,
    identity: Identity,
    crypto: ConversationCrypto,
    http: reqwest::Client,
    tls: Arc<rustls::ClientConfig>,
    _lock: File,
}
impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client")
            .field("installation", &self.installation)
            .finish_non_exhaustive()
    }
}
#[derive(Debug, Serialize)]
pub struct Status {
    pub account_id: String,
    pub device_id: String,
    pub server_id: String,
    pub origin: String,
    pub ownership_state: String,
    pub ownership_generation: u64,
    pub burned: bool,
    pub admission_state: String,
    pub join_policy: Vec<String>,
    pub cursors: BTreeMap<String, u64>,
}
#[derive(Debug, Serialize)]
pub struct SyncResult {
    pub fetched: usize,
    pub messages: Vec<Message>,
}
#[derive(Debug)]
pub enum Notification {
    Advanced,
    Disconnected,
}

#[derive(Debug, thiserror::Error)]
#[error("{code}: HTTP {status}")]
pub struct ApiFailure {
    pub code: &'static str,
    pub status: reqwest::StatusCode,
}

fn now() -> Result<u64> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs())
}
fn id() -> String {
    uuid::Uuid::now_v7().to_string()
}

// Explicit integration-test build only. Exit without unwinding at a durable boundary.
fn crash_boundary(stage: &str, operation: &str) {
    #[cfg(feature = "fault-injection")]
    if std::env::var("CORDS_TEST_CRASH_AT")
        .is_ok_and(|value| value == format!("{stage}.{operation}"))
    {
        std::process::exit(73);
    }
    #[cfg(not(feature = "fault-injection"))]
    let _ = (stage, operation);
}

#[derive(Deserialize)]
struct Welcome {
    sequence: u64,
    welcome: String,
}
impl Client {
    /// Create a new password-protected vault. Desktop callers must use this instead of relying on
    /// the compatibility initialization behavior in [`Self::open`].
    /// # Errors
    /// Returns an error for an existing vault, an invalid password, or failed durable storage.
    pub async fn create(
        directory: &Path,
        migrations: &Path,
        passphrase: &[u8],
        ca: Option<&Path>,
    ) -> Result<Self> {
        ensure!(
            std::str::from_utf8(passphrase)?.chars().count() >= 12,
            "use a password of at least 12 characters"
        );
        ensure!(
            !directory.join("client.db").exists(),
            "account vault already exists"
        );
        Self::open(directory, migrations, Some(passphrase), ca).await
    }

    /// Open an existing vault without creating identity material.
    /// # Errors
    /// Returns an error when the vault does not exist or cannot be authenticated.
    pub async fn open_existing(
        directory: &Path,
        migrations: &Path,
        passphrase: &[u8],
        ca: Option<&Path>,
    ) -> Result<Self> {
        ensure!(
            directory.join("client.db").is_file(),
            "account vault is missing"
        );
        Self::open(directory, migrations, Some(passphrase), ca).await
    }

    /// Replace legacy storage-key protection with a mandatory account password without changing
    /// the storage key, account root, device key, or MLS state.
    /// # Errors
    /// Returns an error if the new password is invalid or the rewrap transaction fails.
    pub async fn rewrap_password(&mut self, passphrase: &[u8]) -> Result<()> {
        ensure!(
            std::str::from_utf8(passphrase)?.chars().count() >= 12,
            "use a password of at least 12 characters"
        );
        let salt = protection::random_key();
        let wrapping = protection::derive_wrapping_key(passphrase, salt.as_ref())?;
        let wrapped = protection::seal(
            &wrapping,
            format!("{}/master/v1", self.installation).as_bytes(),
            self.key.as_ref(),
        )?;
        sqlx::query(
            "UPDATE installation SET protection='passphrase',salt=?1,wrapped_key=?2 WHERE singleton=1",
        )
        .bind(salt.as_ref())
        .bind(wrapped)
        .execute(self.store.pool())
        .await?;
        Ok(())
    }

    /// Clear the current server bearer session while preserving the encrypted local vault.
    /// # Errors
    /// Returns an error when the cleared state cannot be persisted.
    pub async fn sign_out(&mut self) -> Result<()> {
        if let Some(mut session) = self.durable.session.take() {
            session.token.zeroize();
        }
        self.persist(&[], &[]).await
    }

    /// Close `SQLite` before a vault directory is moved or removed.
    pub async fn shutdown(self) {
        self.store.pool().close().await;
    }

    /// Local presentation preferences, protected by the installation storage key.
    #[must_use]
    pub fn ui_preferences(&self) -> serde_json::Value {
        self.durable.ui_preferences.clone()
    }

    /// Public identity and session metadata; never includes bearer tokens or secrets.
    #[must_use]
    pub fn identity_view(&self) -> serde_json::Value {
        serde_json::json!({
            "authorization": self.identity.authorization.value,
            "revoked": self.durable.revoked,
            "membership": self.durable.session.as_ref().map(|s| &s.membership.value),
            "session_expires_at": self.durable.session.as_ref().map(|s| s.expires_at),
            "burn_deliveries": self.durable.pending_burns.values().map(|notice| serde_json::json!({
                "origin": notice.origin, "last_error": notice.last_error, "next_retry_at": notice.next_retry_at
            })).collect::<Vec<_>>(),
        })
    }

    fn self_contact(&self) -> Result<Contact> {
        let preferences = &self.durable.ui_preferences;
        let nickname = preferences
            .get("displayName")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("You")
            .to_owned();
        let picture = preferences.get("avatar");
        let url = picture
            .and_then(|p| p.get("url"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        let avatar = if url.is_empty() {
            None
        } else {
            // Quantization is explicit and bounded; malformed preferences must not silently clamp.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            fn quantize(value: f64, min: f64, max: f64, scale: f64) -> Result<u16> {
                ensure!(
                    value.is_finite() && (min..=max).contains(&value),
                    "invalid avatar crop or zoom"
                );
                Ok((value * scale).round() as u16)
            }
            let number = |field: &str, default: f64| {
                picture
                    .and_then(|p| p.get(field))
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(default)
            };
            Some(cords_protocol::messaging::UserCardPicture {
                url: url.into(),
                data: String::new(),
                shape: picture
                    .and_then(|p| p.get("shape"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("circle")
                    .into(),
                x: quantize(number("x", 50.0), 0.0, 100.0, 1.0)?,
                y: quantize(number("y", 50.0), 0.0, 100.0, 1.0)?,
                zoom_milli: quantize(number("zoom", 1.0), 1.0, 4.0, 1000.0)?,
            })
        };
        let mut contact = self.identity.contact(&self.crypto.public_key())?;
        contact.user_card = Some(self.identity.sign_device(
            cords_protocol::messaging::UserCard {
                version: 1,
                account_id: self.identity.authorization.value.account_id.clone(),
                device_id: self.identity.authorization.value.device_id.clone(),
                nickname,
                avatar,
                issued_at: now()?,
            },
        )?);
        validate_contact(&contact, now()?).context("self-declared user card is invalid")?;
        Ok(contact)
    }

    /// # Errors
    /// Rejects oversized preferences and failed durable writes. Reload after failure.
    pub async fn save_ui_preferences(&mut self, value: serde_json::Value) -> Result<()> {
        ensure!(value.is_object(), "preferences must be an object");
        ensure!(
            serde_json::to_vec(&value)?.len() <= 2_000_000,
            "preferences exceed storage limit"
        );
        self.durable.ui_preferences = value;
        self.persist(&[], &[]).await
    }

    /// # Errors
    /// Returns an error on invalid trust or protocol state, unavailable transport, or failed authenticated storage. Reload durable state after a failed operation.
    pub async fn open(
        directory: &Path,
        migrations: &Path,
        passphrase: Option<&[u8]>,
        ca: Option<&Path>,
    ) -> Result<Self> {
        std::fs::create_dir_all(directory)?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(directory.join("installation.lock"))?;
        fs2::FileExt::try_lock_exclusive(&lock)
            .context("installation is already open in another process")?;
        let database = directory.join("client.db");
        let store =
            SqliteStore::connect(&format!("sqlite://{}?mode=rwc", database.to_string_lossy()))
                .await?;
        store.migrate(migrations).await?;
        let (installation, key) = Self::unlock_storage(&store, passphrase).await?;
        let encrypted: Option<Vec<u8>> =
            sqlx::query_scalar("SELECT sealed FROM client_state WHERE singleton=1")
                .fetch_optional(store.pool())
                .await?;
        let (durable, identity, crypto) = if let Some(bytes) = encrypted {
            let plain =
                protection::open(&key, format!("{installation}/state/v1").as_bytes(), &bytes)?;
            let durable: Durable = serde_json::from_slice(&plain)?;
            let identity =
                Identity::import_secret(&Zeroizing::new(decode(&durable.identity)?), now()?)?;
            let crypto = ConversationCrypto::restore(&Zeroizing::new(decode(&durable.crypto)?))?;
            (durable, identity, crypto)
        } else {
            bail!(
                "installation metadata exists but identity state is missing; restore matching client state"
            )
        };
        let (http, tls) = Self::transport(ca)?;
        let mut client = Self {
            store,
            key,
            installation,
            durable,
            identity,
            crypto,
            http,
            tls,
            _lock: lock,
        };
        client.migrate_legacy_cache().await?;
        client.persist(&[], &[]).await?;
        Ok(client)
    }
    async fn unlock_storage(
        store: &SqliteStore,
        passphrase: Option<&[u8]>,
    ) -> Result<(String, Zeroizing<[u8; 32]>)> {
        let row = sqlx::query(
            "SELECT id,protection,salt,wrapped_key FROM installation WHERE singleton=1",
        )
        .fetch_optional(store.pool())
        .await?;
        let result = if let Some(row) = row {
            let installation: String = row.try_get("id")?;
            let key = match row.try_get::<String, _>("protection")?.as_str() {
                "passphrase" => {
                    let pass = passphrase.context("this installation requires a passphrase")?;
                    let wrapping =
                        protection::derive_wrapping_key(pass, &row.try_get::<Vec<u8>, _>("salt")?)?;
                    let bytes = protection::open(
                        &wrapping,
                        format!("{installation}/master/v1").as_bytes(),
                        &row.try_get::<Vec<u8>, _>("wrapped_key")?,
                    )?;
                    Zeroizing::new(bytes.as_slice().try_into().context("invalid master key")?)
                }
                "os" => {
                    let bytes =
                        Zeroizing::new(keyring::Entry::new("Cords", &installation)?.get_secret()?);
                    Zeroizing::new(
                        bytes
                            .as_slice()
                            .try_into()
                            .context("invalid OS master key")?,
                    )
                }
                _ => bail!("unsupported installation protection"),
            };
            (installation, key)
        } else {
            let installation = id();
            let key = protection::random_key();
            let salt = protection::random_key();
            let (mode, wrapped) = if let Some(pass) = passphrase {
                ensure!(pass.len() >= 12, "use a passphrase of at least 12 bytes");
                let wrapping = protection::derive_wrapping_key(pass, salt.as_ref())?;
                (
                    "passphrase",
                    protection::seal(
                        &wrapping,
                        format!("{installation}/master/v1").as_bytes(),
                        key.as_ref(),
                    )?,
                )
            } else {
                keyring::Entry::new("Cords", &installation)
                    .context("OS credential store unavailable; supply an explicit passphrase")?
                    .set_secret(key.as_ref())?;
                ("os", Vec::new())
            };
            let identity = Identity::generate(now()?)?;
            let crypto = ConversationCrypto::new(identity.authorization.value.device_id.clone())?;
            let mut durable = Durable::default();
            durable.identity = encode(identity.export_secret()?.as_slice());
            durable.crypto = encode(crypto.snapshot()?.as_slice());
            let sealed = protection::seal(
                &key,
                format!("{installation}/state/v1").as_bytes(),
                &Zeroizing::new(serde_json::to_vec(&durable)?),
            )?;
            let mut tx = store.pool().begin().await?;
            sqlx::query("INSERT INTO installation(singleton,id,protection,salt,wrapped_key) VALUES(1,?1,?2,?3,?4)").bind(&installation).bind(mode).bind(salt.as_ref()).bind(wrapped).execute(&mut *tx).await?;
            sqlx::query("INSERT INTO client_state(singleton,sealed) VALUES(1,?1)")
                .bind(sealed)
                .execute(&mut *tx)
                .await?;
            tx.commit().await?;
            (installation, key)
        };
        Ok(result)
    }
    fn transport(ca: Option<&Path>) -> Result<(reqwest::Client, Arc<rustls::ClientConfig>)> {
        let mut roots = rustls::RootCertStore::empty();
        for cert in rustls_native_certs::load_native_certs().certs {
            roots.add(cert)?;
        }
        let mut http = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .redirect(reqwest::redirect::Policy::none());
        if let Some(path) = ca {
            let pem = std::fs::read(path)?;
            http = http.add_root_certificate(reqwest::Certificate::from_pem(&pem)?);
            for certificate in rustls::pki_types::CertificateDer::pem_slice_iter(&pem) {
                roots.add(certificate?)?;
            }
        }
        let tls = rustls::ClientConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_protocol_versions(&[&rustls::version::TLS13])?
        .with_root_certificates(roots)
        .with_no_client_auth();
        Ok((
            http.min_tls_version(reqwest::tls::Version::TLS_1_3)
                .build()?,
            Arc::new(tls),
        ))
    }
    pub fn status(&self) -> Status {
        Status {
            account_id: self.identity.authorization.value.account_id.clone(),
            device_id: self.identity.authorization.value.device_id.clone(),
            server_id: self.durable.server_id.clone(),
            origin: self.durable.origin.clone(),
            ownership_state: self.durable.ownership_state.clone(),
            ownership_generation: self.durable.ownership_generation,
            burned: self.durable.burned,
            admission_state: self.durable.admission_state.clone(),
            join_policy: self.durable.join_policy.clone(),
            cursors: self.durable.cursors.clone(),
        }
    }
    fn active_snapshot(&self) -> Result<ServerSnapshot> {
        Ok(ServerSnapshot {
            origin: self.durable.origin.clone(),
            server_id: self.durable.server_id.clone(),
            server_key: self.durable.server_key.clone(),
            ownership_state: self.durable.ownership_state.clone(),
            ownership_generation: self.durable.ownership_generation,
            admission_state: self.durable.admission_state.clone(),
            join_policy: self.durable.join_policy.clone(),
            session: self.durable.session.clone(),
            cursors: self.durable.cursors.clone(),
            contacts: self.durable.contacts.clone(),
            pending: self.durable.pending.clone(),
            own_events: self.durable.own_events.clone(),
            last_upload: self.durable.last_upload.clone(),
            reserved_roster: self.durable.reserved_roster.clone(),
            crypto: encode(self.crypto.snapshot()?.as_slice()),
        })
    }
    fn activate_snapshot(&mut self, server: ServerSnapshot) -> Result<()> {
        self.crypto = ConversationCrypto::restore(&Zeroizing::new(decode(&server.crypto)?))?;
        self.durable.origin = server.origin;
        self.durable.server_id = server.server_id;
        self.durable.server_key = server.server_key;
        self.durable.ownership_state = server.ownership_state;
        self.durable.ownership_generation = server.ownership_generation;
        self.durable.admission_state = server.admission_state;
        self.durable.join_policy = server.join_policy;
        self.durable.session = server.session;
        self.durable.cursors = server.cursors;
        self.durable.contacts = server.contacts;
        self.durable.pending = server.pending;
        self.durable.own_events = server.own_events;
        self.durable.last_upload = server.last_upload;
        self.durable.reserved_roster = server.reserved_roster;
        Ok(())
    }
    fn clear_active_server(&mut self) -> Result<()> {
        self.crypto = ConversationCrypto::new(self.identity.authorization.value.device_id.clone())?;
        self.durable.origin.clear();
        self.durable.server_id.clear();
        self.durable.server_key.clear();
        self.durable.ownership_state.clear();
        self.durable.ownership_generation = 0;
        self.durable.admission_state.clear();
        self.durable.join_policy.clear();
        self.durable.session = None;
        self.durable.cursors.clear();
        self.durable.contacts.clear();
        self.durable.pending = None;
        self.durable.own_events.clear();
        self.durable.last_upload = None;
        self.durable.reserved_roster = None;
        Ok(())
    }
    async fn migrate_legacy_cache(&self) -> Result<()> {
        let has_legacy: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM message_cache WHERE server_id='')")
                .fetch_one(self.store.pool())
                .await?;
        let has_ciphertext: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM ciphertext_cache WHERE server_id='')")
                .fetch_one(self.store.pool())
                .await?;
        if !has_legacy && !has_ciphertext {
            return Ok(());
        }
        ensure!(
            !self.durable.server_id.is_empty(),
            "legacy cache has no pinned server identity"
        );
        let mut tx = self.store.pool().begin().await?;
        while let Some(row) = sqlx::query(
            "SELECT route_id,message_id,sealed FROM message_cache WHERE server_id='' LIMIT 1",
        )
        .fetch_optional(&mut *tx)
        .await?
        {
            let route: String = row.try_get("route_id")?;
            let message: String = row.try_get("message_id")?;
            let sealed: Vec<u8> = row.try_get("sealed")?;
            let old_context = format!("{}/cache/{route}/{message}/v1", self.installation);
            let plain = Zeroizing::new(protection::open(
                &self.key,
                old_context.as_bytes(),
                &sealed,
            )?);
            let context = format!(
                "{}/cache/{}/{route}/{message}/v2",
                self.installation, self.durable.server_id
            );
            let resealed = protection::seal(&self.key, context.as_bytes(), &plain)?;
            sqlx::query("UPDATE message_cache SET server_id=?1,sealed=?2 WHERE server_id='' AND route_id=?3 AND message_id=?4")
                .bind(&self.durable.server_id).bind(resealed).bind(route).bind(message).execute(&mut *tx).await?;
        }
        sqlx::query("UPDATE ciphertext_cache SET server_id=?1 WHERE server_id=''")
            .bind(&self.durable.server_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }
    pub fn servers(&self) -> Vec<ServerListing> {
        let mut result: Vec<_> = self
            .durable
            .servers
            .values()
            .map(|s| ServerListing {
                origin: s.origin.clone(),
                server_id: s.server_id.clone(),
                ownership_state: s.ownership_state.clone(),
                active: s.server_id == self.durable.server_id,
                archived: false,
                remote_drop_confirmed: false,
            })
            .collect();
        for s in self.durable.archived_servers.values() {
            result.push(ServerListing {
                origin: s.origin.clone(),
                server_id: s.server_id.clone(),
                ownership_state: String::new(),
                active: false,
                archived: true,
                remote_drop_confirmed: s.remote_drop_confirmed,
            });
        }
        result
    }
    pub fn is_burned(&self) -> bool {
        self.durable.burned
    }
    /// Poll signed owner state without initiating recovery or opening any UI.
    /// # Errors
    /// Refuses invalid signatures, state rollback, unavailable transport or failed storage.
    pub async fn refresh_ownership_state(&mut self) -> Result<String> {
        if self.durable.server_id.is_empty() {
            return Ok(String::new());
        }
        let state: Signed<OwnershipState> = Self::response(
            self.http
                .get(format!("{}/api/v1/ownership", self.durable.origin))
                .send()
                .await?,
        )
        .await?;
        state.verify(&self.durable.server_key)?;
        ensure!(
            state.value.version == 1
                && state.value.server_id == self.durable.server_id
                && matches!(
                    state.value.state.as_str(),
                    "UNCLAIMED" | "CLAIMED" | "OWNER_LOCKDOWN"
                ),
            "invalid signed owner state"
        );
        ensure!(
            state.value.generation >= self.durable.ownership_generation,
            "CORDS_OWNERSHIP_STATE_ROLLBACK"
        );
        ensure!(
            state.value.generation != self.durable.ownership_generation
                || state.value.state == self.durable.ownership_state,
            "CORDS_OWNERSHIP_STATE_CONFLICT"
        );
        if self.durable.ownership_state != state.value.state
            || self.durable.ownership_generation != state.value.generation
        {
            // Transfers/recovery reissue credentials and invalidate sessions. Renew by device
            // proof, never by automatically submitting a new membership request.
            if let Some(session) = &mut self.durable.session {
                session.expires_at = 0;
            }
            self.durable.ownership_state = state.value.state;
            self.durable.ownership_generation = state.value.generation;
            self.persist(&[], &[]).await?;
        }
        Ok(self.durable.ownership_state.clone())
    }
    /// Select an already pinned server without changing its trusted identity.
    /// # Errors
    /// Refuses burned identities, unknown servers or unreadable persistent MLS state.
    pub async fn select_server(&mut self, server_id: &str) -> Result<Status> {
        ensure!(!self.durable.burned, "identity has been burned");
        let server = self
            .durable
            .servers
            .get(server_id)
            .cloned()
            .context("server not tracked")?;
        self.activate_snapshot(server)?;
        self.persist(&[], &[]).await?;
        Ok(self.status())
    }
    /// Clear the active server without removing its trusted snapshot or cached data.
    /// # Errors
    /// Refuses unreadable state or failed persistent storage.
    pub async fn deselect_server(&mut self) -> Result<Status> {
        if !self.durable.server_id.is_empty() {
            self.durable
                .servers
                .insert(self.durable.server_id.clone(), self.active_snapshot()?);
        }
        self.clear_active_server()?;
        self.persist(&[], &[]).await?;
        Ok(self.status())
    }
    async fn persist(
        &mut self,
        messages: &[(String, u64, Message)],
        events: &[RouteEvent],
    ) -> Result<()> {
        self.durable.identity = encode(self.identity.export_secret()?.as_slice());
        self.durable.crypto = encode(self.crypto.snapshot()?.as_slice());
        if !self.durable.server_id.is_empty() {
            self.durable
                .servers
                .insert(self.durable.server_id.clone(), self.active_snapshot()?);
        }
        let plain = Zeroizing::new(serde_json::to_vec(&self.durable)?);
        let sealed = protection::seal(
            &self.key,
            format!("{}/state/v1", self.installation).as_bytes(),
            &plain,
        )?;
        let mut tx = self.store.pool().begin().await?;
        sqlx::query("INSERT INTO client_state(singleton,sealed) VALUES(1,?1) ON CONFLICT(singleton) DO UPDATE SET sealed=excluded.sealed").bind(sealed).execute(&mut *tx).await?;
        for (route, sequence, message) in messages {
            let context = format!(
                "{}/cache/{}/{route}/{}/v2",
                self.installation, self.durable.server_id, message.message_id
            );
            let sealed = protection::seal(
                &self.key,
                context.as_bytes(),
                &Zeroizing::new(serde_json::to_vec(message)?),
            )?;
            sqlx::query("INSERT INTO message_cache(server_id,route_id,message_id,sequence,sealed) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(server_id,route_id,message_id) DO UPDATE SET sequence=excluded.sequence,sealed=excluded.sealed")
                .bind(&self.durable.server_id).bind(route).bind(&message.message_id).bind(i64::try_from(*sequence)?).bind(sealed).execute(&mut *tx).await?;
        }
        for event in events {
            sqlx::query("INSERT INTO ciphertext_cache(server_id,route_id,sequence,envelope) VALUES(?1,?2,?3,?4) ON CONFLICT(server_id,route_id,sequence) DO NOTHING")
                .bind(&self.durable.server_id).bind(&event.envelope.route_id).bind(i64::try_from(event.server_sequence)?).bind(protection::seal(&self.key, format!("{}/event/{}/{}/{}", self.installation, self.durable.server_id, event.envelope.route_id, event.server_sequence).as_bytes(), &Zeroizing::new(serde_json::to_vec(event)?))?).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(())
    }
    /// # Errors
    /// Returns an error on invalid trust or protocol state, unavailable transport, or failed authenticated storage. Reload durable state after a failed operation.
    pub async fn reload(&mut self) -> Result<()> {
        let sealed: Vec<u8> =
            sqlx::query_scalar("SELECT sealed FROM client_state WHERE singleton=1")
                .fetch_one(self.store.pool())
                .await?;
        self.durable = serde_json::from_slice(&protection::open(
            &self.key,
            format!("{}/state/v1", self.installation).as_bytes(),
            &sealed,
        )?)?;
        self.crypto = ConversationCrypto::restore(&Zeroizing::new(decode(&self.durable.crypto)?))?;
        Ok(())
    }
    async fn response<T: DeserializeOwned>(mut response: reqwest::Response) -> Result<T> {
        let status = response.status();
        if response
            .content_length()
            .is_some_and(|n| n > 16 * 1024 * 1024)
        {
            bail!("server response too large");
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            ensure!(
                bytes.len().saturating_add(chunk.len()) <= 16 * 1024 * 1024,
                "server response too large"
            );
            bytes.extend_from_slice(&chunk);
        }
        if !status.is_success() {
            let error: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_default();
            let code = match error.get("code").and_then(serde_json::Value::as_str) {
                Some("CORDS_INVALID_OBJECT") => "CORDS_INVALID_OBJECT",
                Some("CORDS_IDENTITY_NOT_YET_VALID") => {
                    "CORDS_IDENTITY_NOT_YET_VALID (check client and server clocks)"
                }
                Some("CORDS_PERMISSION_DENIED") => "CORDS_PERMISSION_DENIED",
                Some("CORDS_STATE_CONFLICT") => "CORDS_STATE_CONFLICT",
                Some("CORDS_AUTH_CHALLENGE_EXPIRED") => "CORDS_AUTH_CHALLENGE_EXPIRED",
                Some("CORDS_RATE_LIMITED") => "CORDS_RATE_LIMITED",
                Some("CORDS_STORAGE_UNAVAILABLE") => "CORDS_STORAGE_UNAVAILABLE",
                Some("CORDS_OWNERSHIP_STATE_CONFLICT") => "CORDS_OWNERSHIP_STATE_CONFLICT",
                Some("CORDS_OWNERSHIP_UNAVAILABLE") => "CORDS_OWNERSHIP_UNAVAILABLE",
                Some("CORDS_APPROVAL_PENDING") => "CORDS_APPROVAL_PENDING",
                Some("CORDS_JOIN_REJECTED") => "CORDS_JOIN_REJECTED",
                Some("OWNER_RECOVERY_REQUIRED") => "OWNER_RECOVERY_REQUIRED",
                _ => "CORDS_HTTP_ERROR",
            };
            return Err(ApiFailure { code, status }.into());
        }
        Ok(serde_json::from_slice(&bytes)?)
    }
    async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let session = self.durable.session.as_ref().context("not authenticated")?;
        Self::response(
            self.http
                .get(format!("{}{path}", self.durable.origin))
                .bearer_auth(&session.token)
                .send()
                .await?,
        )
        .await
    }
    async fn post<T: Serialize, R: DeserializeOwned>(
        &self,
        path: &str,
        body: &T,
        authenticated: bool,
    ) -> Result<R> {
        let mut request = self
            .http
            .post(format!("{}{path}", self.durable.origin))
            .json(body);
        if authenticated {
            request = request.bearer_auth(
                &self
                    .durable
                    .session
                    .as_ref()
                    .context("not authenticated")?
                    .token,
            );
        }
        Self::response(request.send().await?)
            .await
            .with_context(|| format!("request {path} failed"))
    }
    async fn verify_pinned_server(
        &self,
        origin: &str,
        server_id: &str,
        server_key: &str,
    ) -> Result<()> {
        let origin = ServerOrigin::parse(origin)?;
        let signed: SignedServerMetadataV1 = Self::response(
            self.http
                .get(origin.join("/.well-known/cords/server")?)
                .send()
                .await?,
        )
        .await?;
        signed.metadata.verify(&signed.signature)?;
        ensure!(
            signed.metadata.server_id == server_id
                && signed.metadata.server_signing_key == server_key,
            "CORDS_SERVER_KEY_CHANGED"
        );
        Ok(())
    }
    async fn signed_departure(
        &mut self,
        origin: &str,
        server_id: &str,
        server_key: &str,
    ) -> Result<()> {
        self.verify_pinned_server(origin, server_id, server_key)
            .await?;
        let request = if let Some(request) = self.durable.pending_departures.get(server_id) {
            request.clone()
        } else {
            let record = self.identity.sign_root(ServerDeparture {
                version: 1,
                server_id: server_id.into(),
                account_id: self.identity.authorization.value.account_id.clone(),
                issued_at: now()?,
                nonce: encode(protection::random_key().as_ref()),
            })?;
            let request = DepartureRequest {
                record: record.clone(),
                idempotency_key: id(),
            };
            self.durable
                .pending_departures
                .insert(server_id.into(), request.clone());
            self.persist(&[], &[]).await?;
            request
        };
        let receipt: Signed<DepartureReceipt> = Self::response(
            self.http
                .post(format!("{origin}/api/v1/membership/drop"))
                .json(&request)
                .send()
                .await?,
        )
        .await?;
        receipt.verify(server_key)?;
        ensure!(
            receipt.value.version == 1
                && receipt.value.server_id == server_id
                && receipt.value.account_id == self.identity.authorization.value.account_id
                && receipt.value.kind == "drop"
                && receipt.value.request_hash == hash(canonical(&request.record)?),
            "invalid server departure confirmation"
        );
        Ok(())
    }
    async fn persist_purging_server(&mut self, server_id: &str) -> Result<()> {
        self.durable.identity = encode(self.identity.export_secret()?.as_slice());
        self.durable.crypto = encode(self.crypto.snapshot()?.as_slice());
        let sealed = protection::seal(
            &self.key,
            format!("{}/state/v1", self.installation).as_bytes(),
            &Zeroizing::new(serde_json::to_vec(&self.durable)?),
        )?;
        let mut tx = self.store.pool().begin().await?;
        sqlx::query("UPDATE client_state SET sealed=?1 WHERE singleton=1")
            .bind(sealed)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM message_cache WHERE server_id=?1")
            .bind(server_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM ciphertext_cache WHERE server_id=?1")
            .bind(server_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }
    fn detach_server(&mut self, server_id: &str) -> Result<()> {
        self.durable.pending_departures.remove(server_id);
        self.durable.servers.remove(server_id);
        if self.durable.server_id == server_id {
            self.clear_active_server()?;
            if let Some(next) = self.durable.servers.values().next().cloned() {
                self.activate_snapshot(next)?;
            }
        }
        Ok(())
    }
    /// Remove the selected server and its cached data. Local-only removal must be a separate,
    /// explicitly confirmed UI action after a failed authenticated departure.
    /// # Errors
    /// Refuses unknown servers, unverified remote departure or failed local storage.
    pub async fn remove_server(
        &mut self,
        server_id: &str,
        local_only: bool,
    ) -> Result<DepartureOutcome> {
        let server = self
            .durable
            .servers
            .get(server_id)
            .cloned()
            .context("server not tracked")?;
        if !local_only {
            self.signed_departure(&server.origin, &server.server_id, &server.server_key)
                .await?;
        }
        self.detach_server(server_id)?;
        self.persist_purging_server(server_id).await?;
        Ok(DepartureOutcome {
            remote_confirmed: !local_only,
            warning: local_only.then(|| "Remote membership removal was not verified.".into()),
        })
    }
    /// Archive only the origin, pinned identity, and departure outcome; active MLS state and
    /// cached conversations are erased even if the remote request cannot be delivered.
    /// # Errors
    /// Refuses unknown servers or failed local storage; delivery errors are returned as warnings.
    pub async fn archive_server(&mut self, server_id: &str) -> Result<DepartureOutcome> {
        let server = self
            .durable
            .servers
            .get(server_id)
            .cloned()
            .context("server not tracked")?;
        let remote = self
            .signed_departure(&server.origin, &server.server_id, &server.server_key)
            .await;
        let confirmed = remote.is_ok();
        self.detach_server(server_id)?;
        self.durable.archived_servers.insert(
            server_id.into(),
            ArchivedServer {
                origin: server.origin,
                server_id: server_id.into(),
                archived_at: now()?,
                remote_drop_confirmed: confirmed,
            },
        );
        self.persist_purging_server(server_id).await?;
        Ok(DepartureOutcome {
            remote_confirmed: confirmed,
            warning: remote.err().map(|e| {
                format!("Archived locally; remote membership removal was not verified: {e}")
            }),
        })
    }
    /// Persist every root-signed notice before network delivery. This account cannot resume
    /// ordinary messaging after the burn begins; failed destinations remain in the retry queue.
    /// # Errors
    /// Refuses a repeated burn or failed signing/storage; delivery errors remain in the queue.
    pub async fn burn_identity(&mut self) -> Result<BurnOutcome> {
        ensure!(!self.durable.burned, "identity burn has already started");
        let mut targets: Vec<(String, String, String)> = self
            .durable
            .servers
            .values()
            .map(|s| (s.server_id.clone(), s.origin.clone(), s.server_key.clone()))
            .collect();
        for archived in self
            .durable
            .archived_servers
            .values()
            .filter(|s| !s.remote_drop_confirmed)
        {
            if !targets.iter().any(|t| t.0 == archived.server_id) {
                // An archived server has only a fingerprint, not its raw public signing key.
                // The notice remains unresolved unless the key is safely rediscovered.
                targets.push((
                    archived.server_id.clone(),
                    archived.origin.clone(),
                    String::new(),
                ));
            }
        }
        for (server_id, origin, server_key) in targets {
            let record = self.identity.sign_root(IdentityBurn {
                version: 1,
                server_id: server_id.clone(),
                account_id: self.identity.authorization.value.account_id.clone(),
                issued_at: now()?,
                nonce: encode(protection::random_key().as_ref()),
            })?;
            self.durable.pending_burns.insert(
                server_id,
                PendingBurn {
                    origin,
                    server_key,
                    record,
                    idempotency_key: id(),
                    attempts: 0,
                    next_retry_at: 0,
                    last_error: None,
                },
            );
        }
        self.durable.burned = true;
        self.persist(&[], &[]).await?;
        self.retry_pending_burns().await
    }
    /// Retry one due destination and persist its result and backoff.
    /// # Errors
    /// Returns clock/storage errors; destination failures are preserved in the returned pending list.
    pub async fn retry_pending_burns(&mut self) -> Result<BurnOutcome> {
        let time = now()?;
        let pending: Vec<_> = self
            .durable
            .pending_burns
            .iter()
            .filter(|(_, notice)| notice.next_retry_at <= time)
            .take(1)
            .map(|(id, notice)| (id.clone(), notice.clone()))
            .collect();
        let mut confirmed = 0;
        for (server_id, notice) in pending {
            let delivered = self.deliver_burn(&server_id, &notice).await;
            match delivered {
                Ok(()) => {
                    self.durable.pending_burns.remove(&server_id);
                    confirmed += 1;
                }
                Err(error) => {
                    let notice = self
                        .durable
                        .pending_burns
                        .get_mut(&server_id)
                        .context("missing burn notice")?;
                    notice.attempts = notice.attempts.saturating_add(1);
                    notice.next_retry_at =
                        time.saturating_add(30 * (1_u64 << notice.attempts.min(7)));
                    notice.last_error = Some(format!("{error:#}"));
                }
            }
            self.persist(&[], &[]).await?;
        }
        Ok(BurnOutcome {
            confirmed,
            pending: self
                .durable
                .pending_burns
                .values()
                .map(|notice| {
                    format!(
                        "{}: {}",
                        notice.origin,
                        notice.last_error.as_deref().unwrap_or("awaiting delivery")
                    )
                })
                .collect(),
        })
    }
    async fn deliver_burn(&self, server_id: &str, notice: &PendingBurn) -> Result<()> {
        // For archived records with no retained raw key, discovery can recover the key only
        // if its hash matches the old pin. A changed identity is never trusted silently.
        let key = if notice.server_key.is_empty() {
            let origin = ServerOrigin::parse(&notice.origin)?;
            let signed: SignedServerMetadataV1 = Self::response(
                self.http
                    .get(origin.join("/.well-known/cords/server")?)
                    .send()
                    .await?,
            )
            .await?;
            signed.metadata.verify(&signed.signature)?;
            ensure!(
                signed.metadata.server_id == server_id,
                "CORDS_SERVER_KEY_CHANGED"
            );
            signed.metadata.server_signing_key
        } else {
            notice.server_key.clone()
        };
        self.verify_pinned_server(&notice.origin, server_id, &key)
            .await?;
        let request = DepartureRequest {
            record: notice.record.clone(),
            idempotency_key: notice.idempotency_key.clone(),
        };
        let response: Result<Signed<DepartureReceipt>> = async {
            Self::response(
                self.http
                    .post(format!("{}/api/v1/identity/burn", notice.origin))
                    .json(&request)
                    .send()
                    .await?,
            )
            .await
        }
        .await;
        let receipt = response?;
        receipt.verify(&key)?;
        ensure!(
            receipt.value.version == 1
                && receipt.value.server_id == server_id
                && receipt.value.account_id == self.identity.authorization.value.account_id
                && receipt.value.kind == "burn"
                && receipt.value.request_hash == hash(canonical(&notice.record)?),
            "invalid identity burn confirmation"
        );
        Ok(())
    }
    /// # Errors
    /// Returns an error on invalid trust or protocol state, unavailable transport, or failed authenticated storage. Reload durable state after a failed operation.
    pub async fn trust(&mut self, origin: &str) -> Result<Status> {
        ensure!(!self.durable.burned, "CORDS_IDENTITY_BURNED");
        let origin = ServerOrigin::parse(origin)?;
        let signed: SignedServerMetadataV1 = Self::response(
            self.http
                .get(origin.join("/.well-known/cords/server")?)
                .send()
                .await?,
        )
        .await?;
        signed.metadata.verify(&signed.signature)?;
        ensure!(
            signed.metadata.protocol_min <= 1 && signed.metadata.protocol_max >= 1,
            "no common protocol"
        );
        let discovered = signed.metadata.server_id.clone();
        ensure!(
            signed
                .metadata
                .features
                .iter()
                .any(|feature| feature == "user-cards-v1")
                && signed
                    .metadata
                    .features
                    .iter()
                    .any(|feature| feature == "member-pages-v1"),
            "CORDS_SERVER_UPGRADE_REQUIRED: signed user cards and paginated membership are unavailable; update the server before joining"
        );
        let known = self
            .durable
            .servers
            .values()
            .find(|server| server.origin == origin.to_string())
            .cloned();
        if let Some(server) = &known {
            ensure!(
                server.server_id == discovered
                    && server.server_key == signed.metadata.server_signing_key,
                "CORDS_SERVER_KEY_CHANGED"
            );
        } else {
            ensure!(
                !self.durable.servers.contains_key(&discovered),
                "CORDS_SERVER_ORIGIN_CHANGED"
            );
        }
        let ownership: Signed<OwnershipState> = Self::response(
            self.http
                .get(origin.join("/api/v1/ownership")?)
                .send()
                .await?,
        )
        .await?;
        ownership.verify(&signed.metadata.server_signing_key)?;
        ensure!(
            ownership.value.version == 1
                && ownership.value.server_id == discovered
                && matches!(
                    ownership.value.state.as_str(),
                    "UNCLAIMED" | "CLAIMED" | "OWNER_LOCKDOWN"
                )
                && ownership.value.generation > 0,
            "invalid ownership state"
        );
        if let Some(server) = &known {
            ensure!(
                ownership.value.generation >= server.ownership_generation,
                "CORDS_OWNERSHIP_STATE_ROLLBACK"
            );
            ensure!(
                ownership.value.generation != server.ownership_generation
                    || ownership.value.state == server.ownership_state,
                "CORDS_OWNERSHIP_STATE_CONFLICT"
            );
        }
        if let Some(server) = known {
            self.activate_snapshot(server)?;
        } else if self.durable.origin != origin.to_string() {
            self.clear_active_server()?;
        }
        self.durable.origin = origin.to_string();
        self.durable.server_id = discovered;
        self.durable.server_key = signed.metadata.server_signing_key;
        self.durable.join_policy = signed.metadata.join_policy;
        self.durable.ownership_state = ownership.value.state;
        self.durable.ownership_generation = ownership.value.generation;
        self.persist(&[], &[]).await?;
        Ok(self.status())
    }
    /// # Errors
    /// Returns an error on invalid trust or protocol state, unavailable transport, or failed authenticated storage. Reload durable state after a failed operation.
    pub async fn authenticate(&mut self) -> Result<Status> {
        self.authenticate_with_rejoin(true).await
    }
    #[allow(clippy::too_many_lines)] // Keep challenge binding, explicit rejoin and credential validation together.
    async fn authenticate_with_rejoin(&mut self, allow_rejoin: bool) -> Result<Status> {
        ensure!(!self.durable.burned, "CORDS_IDENTITY_BURNED");
        ensure!(
            !self.durable.revoked,
            "this installation's device is revoked"
        );
        let origin = self.durable.origin.clone();
        let fingerprint = self.durable.server_id.clone();
        ensure!(
            !fingerprint.is_empty(),
            "inspect and accept server identity first"
        );
        self.trust(&origin).await?;
        ensure!(
            self.durable.ownership_state == "CLAIMED",
            "{}",
            if self.durable.ownership_state == "OWNER_LOCKDOWN" {
                "OWNER_RECOVERY_REQUIRED"
            } else {
                "CORDS_SERVER_UNCLAIMED"
            }
        );
        let mut purpose = if self.durable.session.is_some() || !allow_rejoin {
            "authenticate"
        } else {
            "join"
        };
        let mut request = ChallengeRequest {
            contact: self.self_contact()?,
            purpose: purpose.into(),
            idempotency_key: id(),
        };
        validate_contact(&request.contact, now()?).context("local identity validation failed")?;
        let response: Result<Challenge> = self
            .post(
                if purpose == "join" {
                    "/api/v1/join/request"
                } else {
                    "/api/v1/auth/challenge"
                },
                &request,
                false,
            )
            .await;
        let challenge = match response {
            Ok(challenge) => challenge,
            Err(error)
                if purpose == "authenticate"
                    && error
                        .downcast_ref::<ApiFailure>()
                        .is_some_and(|failure| failure.code == "CORDS_PERMISSION_DENIED") =>
            {
                self.durable.session = None;
                self.durable.admission_state = "removed".into();
                self.persist(&[], &[]).await?;
                if !allow_rejoin {
                    return Err(error);
                }
                purpose = "join";
                request.purpose = purpose.into();
                request.idempotency_key = id();
                self.post("/api/v1/join/request", &request, false).await?
            }
            Err(error) => return Err(error),
        };
        ensure!(
            challenge.server_id == fingerprint
                && challenge.device_id == request.contact.authorization.value.device_id
                && challenge.account_id == request.contact.authorization.value.account_id
                && challenge.authorization_hash == hash(canonical(&request.contact.authorization)?)
                && challenge.purpose == purpose
                && challenge.expires_at > now()?,
            "invalid authentication challenge context"
        );
        let request = SessionRequest {
            proof: self.identity.sign_device(challenge)?,
            idempotency_key: id(),
        };
        let session: Session = match self.post("/api/v1/auth/session", &request, false).await {
            Ok(session) => session,
            Err(error) => {
                if let Some(failure) = error.downcast_ref::<ApiFailure>() {
                    match failure.code {
                        "CORDS_APPROVAL_PENDING" => self.durable.admission_state = "pending".into(),
                        "CORDS_JOIN_REJECTED" => self.durable.admission_state = "rejected".into(),
                        _ => return Err(error),
                    }
                    self.durable.session = None;
                    self.persist(&[], &[]).await?;
                }
                return Err(error);
            }
        };
        session.membership.verify(&self.durable.server_key)?;
        ensure!(
            session.membership.value.server_id == fingerprint
                && session.membership.value.account_id
                    == self.identity.authorization.value.account_id
                && session.membership.value.device_id
                    == self.identity.authorization.value.device_id
                && session.membership.value.status == "active",
            "invalid membership"
        );
        self.durable.session = Some(session);
        self.durable.admission_state = "active".into();
        self.persist(&[], &[]).await?;
        self.flush().await?;
        Ok(self.status())
    }

    /// Claim persistently unclaimed server ownership with its server-generated one-time code.
    /// # Errors
    /// Returns an error when the claim is unavailable, invalid, already owned by another account,
    /// or the signed replacement membership is invalid.
    pub async fn claim_ownership(&mut self, claim_code: &str) -> Result<Status> {
        ensure!(!self.durable.burned, "CORDS_IDENTITY_BURNED");
        ensure!(claim_code.len() == 43, "invalid ownership claim code");
        ensure!(!self.durable.server_id.is_empty(), "trust the server first");
        self.trust(&self.durable.origin.clone()).await?;
        ensure!(
            self.durable.ownership_state == "UNCLAIMED",
            "server is not unclaimed"
        );
        let contact = self.self_contact()?;
        let challenge_request = ChallengeRequest {
            contact: contact.clone(),
            purpose: "claim_ownership".into(),
            idempotency_key: id(),
        };
        let challenge: Challenge = self
            .post("/api/v1/ownership/challenge", &challenge_request, false)
            .await?;
        ensure!(
            challenge.server_id == self.durable.server_id
                && challenge.account_id == self.identity.authorization.value.account_id
                && challenge.device_id == self.identity.authorization.value.device_id
                && challenge.authorization_hash == hash(canonical(&contact.authorization)?)
                && challenge.purpose == "claim_ownership"
                && challenge.expires_at > now()?,
            "invalid ownership challenge context"
        );
        let root_proof = self.identity.sign_root(OwnershipProof {
            version: 1,
            server_id: challenge.server_id.clone(),
            account_id: challenge.account_id.clone(),
            device_id: challenge.device_id.clone(),
            challenge_hash: hash(canonical(&challenge)?),
        })?;
        let request = OwnershipClaim {
            claim_code: claim_code.into(),
            contact,
            device_proof: self.identity.sign_device(challenge)?,
            root_proof,
            idempotency_key: id(),
        };
        let session: Session = self
            .post("/api/v1/ownership/claim", &request, false)
            .await?;
        session.membership.verify(&self.durable.server_key)?;
        ensure!(
            session.membership.value.server_id == self.durable.server_id
                && session.membership.value.account_id
                    == self.identity.authorization.value.account_id
                && session.membership.value.device_id
                    == self.identity.authorization.value.device_id
                && session.membership.value.status == "active"
                && session
                    .membership
                    .value
                    .capabilities
                    .iter()
                    .any(|capability| capability == "server.manage"),
            "invalid ownership membership"
        );
        self.durable.session = Some(session);
        self.durable.ownership_state = "CLAIMED".into();
        self.durable.admission_state = "active".into();
        self.persist(&[], &[]).await?;
        self.flush().await?;
        Ok(self.status())
    }
    /// Designate an existing server member as successor. The server starts its 30-day clock
    /// only when it accepts the signed designation.
    /// Designate a successor using the owner account root.
    /// # Errors
    /// Returns signing, authorization, server-policy or transport errors.
    pub async fn designate_successor(&mut self, successor_account_id: &str) -> Result<String> {
        ensure!(!self.durable.burned, "CORDS_IDENTITY_BURNED");
        ensure!(!self.durable.server_id.is_empty(), "select a server first");
        let record = self.identity.sign_root(SuccessorDesignation {
            version: 1,
            server_id: self.durable.server_id.clone(),
            owner_account_id: self.identity.authorization.value.account_id.clone(),
            successor_account_id: successor_account_id.into(),
            nonce: encode(protection::random_key().as_ref()),
        })?;
        self.post(
            "/api/v1/ownership/successor",
            &SuccessorRequest {
                record,
                idempotency_key: id(),
            },
            false,
        )
        .await
    }
    /// Countersign the exact designation as its designated successor.
    /// # Errors
    /// Returns signing, authorization, server-policy or transport errors.
    pub async fn accept_successor(&mut self, designation_hash: &str) -> Result<()> {
        ensure!(!self.durable.burned, "CORDS_IDENTITY_BURNED");
        ensure!(!self.durable.server_id.is_empty(), "select a server first");
        let record = self.identity.sign_root(SuccessorAcceptance {
            version: 1,
            server_id: self.durable.server_id.clone(),
            successor_account_id: self.identity.authorization.value.account_id.clone(),
            designation_hash: designation_hash.into(),
            nonce: encode(protection::random_key().as_ref()),
        })?;
        let accepted: bool = self
            .post(
                "/api/v1/ownership/successor/accept",
                &SuccessorRequest {
                    record,
                    idempotency_key: id(),
                },
                false,
            )
            .await?;
        ensure!(accepted, "successor acceptance was not confirmed");
        Ok(())
    }
    /// Operator-code recovery is deliberately interactive and never called by background sync.
    /// Submit an operator recovery code with fresh root/device possession proofs.
    /// # Errors
    /// Refuses burned identities, wrong state/code, invalid server responses or failed storage.
    pub async fn recover_owner(&mut self, code: &str) -> Result<Status> {
        ensure!(!self.durable.burned, "CORDS_IDENTITY_BURNED");
        ensure!(code.len() == 43, "invalid operator recovery code");
        ensure!(!self.durable.server_id.is_empty(), "select a server first");
        self.trust(&self.durable.origin.clone()).await?;
        ensure!(
            self.durable.ownership_state == "OWNER_LOCKDOWN",
            "server is not in owner lockdown"
        );
        let contact = self.self_contact()?;
        let request = ChallengeRequest {
            contact: contact.clone(),
            purpose: "recover_ownership".into(),
            idempotency_key: id(),
        };
        let challenge: Challenge = self
            .post("/api/v1/ownership/recovery/challenge", &request, false)
            .await?;
        ensure!(
            challenge.server_id == self.durable.server_id
                && challenge.account_id == self.identity.authorization.value.account_id
                && challenge.device_id == self.identity.authorization.value.device_id
                && challenge.authorization_hash == hash(canonical(&contact.authorization)?)
                && challenge.purpose == "recover_ownership"
                && challenge.expires_at > now()?,
            "invalid owner recovery challenge"
        );
        let root_proof = self.identity.sign_root(OwnershipRecoveryProof {
            version: 1,
            server_id: challenge.server_id.clone(),
            account_id: challenge.account_id.clone(),
            device_id: challenge.device_id.clone(),
            challenge_hash: hash(canonical(&challenge)?),
        })?;
        let claim = OwnershipRecoveryClaim {
            recovery_code: code.into(),
            contact,
            device_proof: self.identity.sign_device(challenge)?,
            root_proof,
            idempotency_key: id(),
        };
        let session: Session = self
            .post("/api/v1/ownership/recovery/claim", &claim, false)
            .await?;
        session.membership.verify(&self.durable.server_key)?;
        ensure!(
            session.membership.value.server_id == self.durable.server_id
                && session.membership.value.account_id
                    == self.identity.authorization.value.account_id
                && session.membership.value.device_id
                    == self.identity.authorization.value.device_id
                && session.membership.value.status == "active"
                && session
                    .membership
                    .value
                    .capabilities
                    .iter()
                    .any(|c| c == "server.manage"),
            "invalid recovered owner membership"
        );
        self.durable.session = Some(session);
        self.durable.ownership_state = "CLAIMED".into();
        self.durable.admission_state = "active".into();
        self.persist(&[], &[]).await?;
        Ok(self.status())
    }
    async fn ensure_session(&mut self) -> Result<()> {
        ensure!(!self.durable.burned, "CORDS_IDENTITY_BURNED");
        ensure!(
            self.durable.admission_state != "pending",
            "CORDS_APPROVAL_PENDING"
        );
        ensure!(
            self.durable.admission_state != "rejected",
            "CORDS_JOIN_REJECTED"
        );
        ensure!(
            self.durable.admission_state != "removed",
            "CORDS_MEMBERSHIP_REMOVED: explicitly request membership again or leave this server"
        );
        ensure!(
            !self.durable.revoked,
            "this installation's device is revoked"
        );
        let renewal_time = now()?.saturating_add(5);
        if self
            .durable
            .session
            .as_ref()
            .is_none_or(|s| s.expires_at <= renewal_time)
        {
            self.authenticate_with_rejoin(false).await?;
        }
        Ok(())
    }
    async fn queue<T: Serialize>(
        &mut self,
        path: String,
        body: T,
        kind: &str,
        route: &str,
    ) -> Result<serde_json::Value> {
        ensure!(
            self.durable.pending.is_none(),
            "a previous operation must be recovered first"
        );
        self.durable.pending = Some(Pending {
            path,
            body: serde_json::to_value(body)?,
            kind: kind.into(),
            route: route.into(),
        });
        self.persist(&[], &[]).await?;
        self.flush().await
    }
    /// # Errors
    /// Returns an error on invalid trust or protocol state, unavailable transport, or failed authenticated storage. Reload durable state after a failed operation.
    pub async fn flush(&mut self) -> Result<serde_json::Value> {
        ensure!(!self.durable.burned, "CORDS_IDENTITY_BURNED");
        let mut first_result = None;
        loop {
            let Some(pending) = self.durable.pending.clone() else {
                return Ok(first_result.unwrap_or(serde_json::Value::Null));
            };
            let response: Result<serde_json::Value> = self
                .post(&pending.path, &pending.body, pending.kind != "revoke")
                .await;
            let value = match response {
                Ok(value) => value,
                Err(error) => {
                    if error
                        .downcast_ref::<ApiFailure>()
                        .is_some_and(|failure| failure.code == "CORDS_PERMISSION_DENIED")
                    {
                        // A verified refusal is terminal for this request, not an unknown delivery outcome.
                        self.durable.pending = None;
                        self.persist(&[], &[]).await?;
                    }
                    return Err(error);
                }
            };
            crash_boundary("response_received", &pending.kind);
            first_result.get_or_insert_with(|| value.clone());
            let mut followup = None;
            match pending.kind.as_str() {
                "revoke" => {
                    self.durable.revoked = true;
                    self.durable.session = None;
                }
                "create" | "replace" => {
                    let route = value.as_str().context("invalid channel response")?;
                    let channel = self.fetch_channel(route).await?;
                    self.durable.cursors.insert(route.into(), 0);
                    if channel.confidentiality_mode == ConfidentialityMode::Public {
                        self.durable.pending = None;
                        self.persist(&[], &[]).await?;
                        continue;
                    }
                    self.crypto.create(route)?;
                    let binding = self.identity.sign_device(GroupBinding {
                        version: 1,
                        server_id: self.durable.server_id.clone(),
                        channel_id: route.into(),
                        group_id: route.into(),
                        creator_device_id: self.identity.authorization.value.device_id.clone(),
                    })?;
                    followup = Some(Pending {
                        path: format!("/api/v1/channels/{route}/binding"),
                        body: serde_json::to_value(ChannelBind {
                            binding,
                            idempotency_key: id(),
                        })?,
                        kind: "binding".into(),
                        route: route.into(),
                    });
                }
                "commit" => {
                    let event: RouteEvent = serde_json::from_value(value.clone())?;
                    self.crypto.merge_pending(&pending.route)?;
                    self.validate_roster(&pending.route).await?;
                    self.durable.reserved_roster = None;
                    self.durable
                        .own_events
                        .insert(event.envelope.event_id.clone(), "commit".into());
                }
                "roster" => {
                    self.durable.reserved_roster = Some((
                        pending.route.clone(),
                        serde_json::from_value(value.clone())?,
                    ));
                }
                _ => {}
            }
            self.durable.pending = followup;
            self.persist(&[], &[]).await?;
        }
    }
    /// # Errors
    /// Returns an error on invalid trust or protocol state, unavailable transport, or failed authenticated storage. Reload durable state after a failed operation.
    pub async fn publish_key_package(&mut self) -> Result<serde_json::Value> {
        self.ensure_session().await?;
        self.flush().await?;
        let request = KeyPackageUpload {
            key_package: encode(self.crypto.key_package()?),
            idempotency_key: id(),
        };
        self.queue("/api/v1/key-packages".into(), request, "keypackage", "")
            .await
    }
    /// # Errors
    /// Returns an error on invalid trust or protocol state, unavailable transport, or failed authenticated storage. Reload durable state after a failed operation.
    pub async fn create_channel(&mut self, name: &str) -> Result<String> {
        self.create_channel_with_mode(name, ConfidentialityMode::Encrypted)
            .await
    }
    /// Create a permanently classified channel; encryption remains the default.
    pub async fn create_channel_with_mode(
        &mut self,
        name: &str,
        mode: ConfidentialityMode,
    ) -> Result<String> {
        self.ensure_session().await?;
        self.flush().await?;
        let value = self
            .queue(
                "/api/v1/channels".into(),
                ChannelCreate {
                    confidentiality_mode: mode,
                    name: name.into(),
                    idempotency_key: id(),
                },
                "create",
                "",
            )
            .await?;
        Ok(value.as_str().context("invalid channel response")?.into())
    }
    /// # Errors
    /// Returns an error on invalid trust or protocol state, unavailable transport, or failed authenticated storage. Reload durable state after a failed operation.
    pub async fn members(&mut self) -> Result<Vec<Contact>> {
        self.ensure_session().await?;
        let mut contacts = Vec::new();
        let mut after = String::new();
        loop {
            let page: Vec<Contact> = self.get(&format!("/api/v1/members?after={after}")).await?;
            ensure!(page.len() <= 16, "oversized member page");
            for contact in &page {
                validate_contact(contact, now()?)?;
                let device = &contact.authorization.value.device_id;
                ensure!(
                    device > &after
                        && device.len() <= 128
                        && device
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b == b'-'),
                    "invalid member page cursor"
                );
                after.clone_from(device);
            }
            let complete = page.len() < 16;
            contacts.extend(page);
            ensure!(
                contacts.len() <= 1000,
                "member directory exceeds supported client size"
            );
            if complete {
                break;
            }
        }
        Ok(contacts)
    }
    /// # Errors
    /// Returns an error on invalid trust or protocol state, unavailable transport, or failed authenticated storage. Reload durable state after a failed operation.
    pub async fn channels(&mut self) -> Result<Vec<Channel>> {
        self.ensure_session().await?;
        let channels: Vec<Channel> = self.get("/api/v1/channels").await?;
        let mut verified = Vec::new();
        for channel in channels {
            verified.push(self.observe_channel(channel).await?);
        }
        self.persist(&[], &[]).await?;
        Ok(verified)
    }

    async fn fetch_channel(&mut self, route: &str) -> Result<Channel> {
        let channel: Channel = self.get(&format!("/api/v1/channels/{route}")).await?;
        ensure!(
            channel.channel_id == route,
            "Channel Identity Conflict: response changed the requested channel identity"
        );
        self.observe_channel(channel).await
    }

    async fn observe_channel(&mut self, mut channel: Channel) -> Result<Channel> {
        const CONFLICT: &str = "Channel Identity Conflict: This server attempted to change the confidentiality mode or established identity of an existing channel. Cords has rejected the change because it violates the channel's established security properties.";
        let identity = channel
            .identity
            .as_ref()
            .context("Channel identity verification unavailable: upgrade the server")?;
        identity.verify(&self.durable.server_key)?;
        let v = &identity.value;
        ensure!(
            v.version == 1
                && v.server_id == self.durable.server_id
                && v.channel_id == channel.channel_id
                && uuid::Uuid::parse_str(&v.channel_id).is_ok()
                && v.confidentiality_mode == channel.confidentiality_mode
                && v.name == channel.name
                && v.creator_device_id == channel.creator_device_id,
            CONFLICT
        );
        let transition = channel
            .transition
            .as_ref()
            .context("missing authenticated channel state")?;
        transition.verify(&self.durable.server_key)?;
        let t = &transition.value;
        ensure!(
            t.version == 1 && t.server_id == v.server_id && t.channel_id == v.channel_id,
            CONFLICT
        );
        let key = format!("{}/{}", v.server_id, v.channel_id);
        // Upgrade continuity: existing MLS state predates signed mode metadata.
        // It is already proof that this locally established route is encrypted.
        if self.crypto.epoch(&channel.channel_id).is_ok() {
            ensure!(
                v.confidentiality_mode == ConfidentialityMode::Encrypted,
                CONFLICT
            );
        }
        let previous = self.durable.channel_identities.get(&key).cloned();
        if let Some(previous) = &previous {
            ensure!(
                canonical(&previous.channel.identity)? == canonical(&channel.identity)?,
                CONFLICT
            );
            let old = previous
                .channel
                .transition
                .as_ref()
                .context("missing retained channel state")?;
            ensure!(
                t.generation >= old.value.generation
                    && (t.generation != old.value.generation
                        || canonical(transition)? == canonical(old)?)
                    && (!old.value.retired || t.retired),
                "Channel Identity Conflict: retired or authenticated channel state was rolled back"
            );
            if old.value.succession.is_some() {
                ensure!(
                    canonical(&old.value.succession)? == canonical(&t.succession)?,
                    "Channel Identity Conflict: succession changed"
                );
            }
        }
        let mut downgrade = false;
        if let Some(record) = &t.succession {
            let s = &record.value;
            s.predecessor.verify(&self.durable.server_key)?;
            validate_contact(&s.initiator, s.issued_at)?;
            record.verify(&s.initiator.authorization.value.device_public_key)?;
            s.membership.verify(&self.durable.server_key)?;
            let m = &s.membership.value;
            let a = &s.initiator.authorization.value;
            let original = &s.predecessor.value;
            ensure!(
                s.version == 1
                    && original.version == 1
                    && s.server_id == v.server_id
                    && original.server_id == v.server_id
                    && original.channel_id != s.successor_channel_id
                    && uuid::Uuid::parse_str(&s.successor_channel_id).is_ok()
                    && uuid::Uuid::parse_str(&s.nonce).is_ok()
                    && m.server_id == v.server_id
                    && m.status == "active"
                    && m.device_id == a.device_id
                    && m.account_id == a.account_id
                    && m.capabilities.iter().any(|c| c == "channel.create")
                    && (original.creator_device_id == a.device_id
                        || m.capabilities.iter().any(|c| c == "channel.manage")),
                "invalid channel succession authority"
            );
            let original_key = format!("{}/{}", v.server_id, original.channel_id);
            if let Some(known) = self.durable.channel_identities.get(&original_key) {
                ensure!(
                    canonical(&known.channel.identity)? == canonical(&Some(s.predecessor.clone()))?,
                    CONFLICT
                );
            }
            if v.channel_id == original.channel_id {
                ensure!(
                    t.retired && canonical(identity)? == canonical(&s.predecessor)?,
                    "invalid predecessor retirement"
                );
            } else {
                ensure!(
                    v.channel_id == s.successor_channel_id
                        && v.confidentiality_mode == s.confidentiality_mode
                        && v.name == s.name
                        && v.creator_device_id == a.device_id,
                    "invalid successor identity"
                );
                downgrade = original.confidentiality_mode == ConfidentialityMode::Encrypted
                    && v.confidentiality_mode == ConfidentialityMode::Public;
            }
        }
        let archived = t.retired || previous.as_ref().is_some_and(|p| p.archived);
        let acknowledged = previous.as_ref().is_some_and(|p| p.acknowledged);
        let retained_history = if let Some(old) = &previous {
            if old.archived {
                old.retained_history.clone()
            } else if archived {
                self.history(&channel.channel_id).await?
            } else {
                Vec::new()
            }
        } else if archived {
            self.history(&channel.channel_id).await?
        } else {
            Vec::new()
        };
        channel.requires_public_acknowledgement = downgrade && !acknowledged;
        channel.locally_archived = archived;
        self.durable.channel_identities.insert(
            key,
            LocalChannel {
                channel: channel.clone(),
                acknowledged,
                archived,
                retained_history,
            },
        );
        self.persist(&[], &[]).await?;
        Ok(channel)
    }

    /// Explicit user action, durably scoped to this successor identity.
    pub async fn acknowledge_public_successor(&mut self, route: &str) -> Result<()> {
        let channel = self.fetch_channel(route).await?;
        ensure!(
            channel.confidentiality_mode == ConfidentialityMode::Public
                && !channel.locally_archived,
            "not an active public channel"
        );
        let local = self
            .durable
            .channel_identities
            .get_mut(&format!("{}/{route}", self.durable.server_id))
            .context("missing channel identity")?;
        local.acknowledged = true;
        self.persist(&[], &[]).await
    }

    /// Atomically retire the predecessor and establish a fresh server-adopted UUID.
    pub async fn replace_channel(
        &mut self,
        route: &str,
        name: &str,
        mode: ConfidentialityMode,
    ) -> Result<String> {
        self.ensure_session().await?;
        self.flush().await?;
        let channel = self.fetch_channel(route).await?;
        ensure!(!channel.locally_archived, "CHANNEL_RETIRED");
        let request = ChannelReplace {
            record: self.identity.sign_device(ChannelSuccession {
                version: 1,
                server_id: self.durable.server_id.clone(),
                predecessor: channel.identity.context("missing predecessor identity")?,
                successor_channel_id: id(),
                name: name.into(),
                confidentiality_mode: mode,
                initiator: self.identity.contact(&self.crypto.public_key())?,
                membership: self
                    .durable
                    .session
                    .as_ref()
                    .context("missing session")?
                    .membership
                    .clone(),
                issued_at: now()?,
                nonce: id(),
            })?,
            idempotency_key: id(),
        };
        let value = self
            .queue(
                format!("/api/v1/channels/{route}/replace"),
                request,
                "replace",
                route,
            )
            .await?;
        self.fetch_channel(route).await?;
        Ok(value
            .as_str()
            .context("invalid replacement response")?
            .into())
    }

    pub async fn retire_channel(&mut self, route: &str) -> Result<()> {
        self.ensure_session().await?;
        self.flush().await?;
        self.queue(
            format!("/api/v1/channels/{route}/retire"),
            id(),
            "retire",
            route,
        )
        .await?;
        self.fetch_channel(route).await?;
        Ok(())
    }

    /// Partial history actually received by this vault, always read-only and sealed at rest.
    pub fn channel_archives(&self) -> Vec<Channel> {
        self.durable
            .channel_identities
            .values()
            .filter(|l| l.archived)
            .map(|l| {
                let mut channel = l.channel.clone();
                channel.locally_archived = true;
                channel
            })
            .collect()
    }
    pub fn archived_channel_history(&self, server: &str, route: &str) -> Result<Vec<Message>> {
        let local = self
            .durable
            .channel_identities
            .get(&format!("{server}/{route}"))
            .context("channel archive unavailable")?;
        ensure!(local.archived, "channel is not archived");
        Ok(local.retained_history.clone())
    }
    /// List pending requests visible to a server manager.
    /// # Errors
    /// Returns an error when the session lacks server management permission or transport fails.
    pub async fn membership_requests(&mut self) -> Result<Vec<MembershipRequest>> {
        self.ensure_session().await?;
        let mut requests: Vec<MembershipRequest> = Vec::new();
        let mut after = String::new();
        loop {
            let page: Vec<MembershipRequest> = self
                .get(&format!("/api/v1/membership-requests?after={after}"))
                .await?;
            ensure!(page.len() <= 16, "oversized membership request page");
            for request in &page {
                ensure!(
                    request.device_id > after
                        && request.device_id.len() <= 128
                        && request
                            .device_id
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b == b'-'),
                    "invalid membership page cursor"
                );
                after.clone_from(&request.device_id);
            }
            let complete = page.len() < 16;
            requests.extend(page);
            ensure!(
                requests.len() <= 1000,
                "membership queue exceeds supported client size"
            );
            if complete {
                break;
            }
        }
        for request in &requests {
            if let Some(card) = &request.user_card {
                let authorization = request
                    .authorization
                    .as_ref()
                    .context("user card lacks its certified device identity")?;
                cords_identity::validate_user_card(authorization, card, now()?)?;
                ensure!(
                    request.account_id == card.value.account_id
                        && request.device_id == card.value.device_id,
                    "membership request user card identity mismatch"
                );
            }
        }
        Ok(requests)
    }
    /// Approve a verified pending device request.
    /// # Errors
    /// Returns an error when approval is unauthorized or the request is no longer pending.
    pub async fn approve_membership(
        &mut self,
        device: &str,
    ) -> Result<Signed<cords_protocol::messaging::Membership>> {
        self.ensure_session().await?;
        let result: Signed<cords_protocol::messaging::Membership> = self
            .post(
                &format!("/api/v1/membership-requests/{device}/approve"),
                &serde_json::json!({}),
                true,
            )
            .await?;
        result.verify(&self.durable.server_key)?;
        ensure!(
            result.value.server_id == self.durable.server_id
                && result.value.device_id == device
                && result.value.status == "active",
            "invalid approved membership"
        );
        Ok(result)
    }
    /// Reject a pending device request.
    /// # Errors
    /// Returns an error when rejection is unauthorized or the request is no longer pending.
    pub async fn reject_membership(&mut self, device: &str) -> Result<()> {
        self.ensure_session().await?;
        let session = self.durable.session.as_ref().context("not authenticated")?;
        let response = self
            .http
            .post(format!(
                "{}/api/v1/membership-requests/{device}/reject",
                self.durable.origin
            ))
            .bearer_auth(&session.token)
            .send()
            .await?;
        ensure!(
            response.status() == reqwest::StatusCode::NO_CONTENT,
            "membership rejection failed: HTTP {}",
            response.status()
        );
        Ok(())
    }
    fn observe_contact(&mut self, contact: &Contact) -> Result<()> {
        validate_contact(contact, now()?)?;
        let account = &contact.authorization.value.account_id;
        if let Some(previous) = self.durable.contacts.get(account) {
            validate_transition(previous, &contact.authorization)?;
        }
        self.durable
            .contacts
            .insert(account.clone(), contact.authorization.clone());
        Ok(())
    }
    async fn validate_roster(&mut self, route: &str) -> Result<()> {
        let epoch = self.crypto.epoch(route)?;
        let channel: Channel = self
            .get(&format!("/api/v1/channels/{route}?epoch={epoch}"))
            .await?;
        self.observe_channel(channel.clone()).await?;
        ensure!(
            channel.confidentiality_mode == ConfidentialityMode::Encrypted
                && channel.channel_id == route
                && channel.epoch == epoch,
            "channel binding mismatch"
        );
        let binding = channel
            .binding
            .as_ref()
            .context("channel has no signed group binding")?;
        let creator = channel
            .members
            .iter()
            .find(|c| c.authorization.value.device_id == channel.creator_device_id)
            .context("missing creator identity")?;
        binding.verify(&creator.authorization.value.device_public_key)?;
        ensure!(
            binding.value.server_id == self.durable.server_id
                && binding.value.channel_id == route
                && binding.value.group_id == route
                && binding.value.creator_device_id == channel.creator_device_id,
            "invalid group binding"
        );
        let actual = self.crypto.roster(route)?;
        ensure!(
            actual.len() == channel.members.len(),
            "undisclosed MLS participant"
        );
        for contact in &channel.members {
            self.observe_contact(contact)?;
            ensure!(
                actual.iter().any(
                    |(device, key)| device == &contact.authorization.value.device_id
                        && encode(key) == contact.mls_binding.value.mls_public_key
                ),
                "MLS identity substitution"
            );
        }
        Ok(())
    }
    /// # Errors
    /// Returns an error on invalid trust or protocol state, unavailable transport, or failed authenticated storage. Reload durable state after a failed operation.
    pub async fn add_member(&mut self, route: &str, device: &str) -> Result<()> {
        self.change_member(route, device, RosterAction::Add).await
    }
    /// # Errors
    /// Returns an error if the current device cannot manage the channel or removal cannot be committed.
    pub async fn remove_member(&mut self, route: &str, device: &str) -> Result<()> {
        self.change_member(route, device, RosterAction::Remove)
            .await
    }
    /// Return root-authenticated removal requests awaiting an MLS commit.
    /// # Errors
    /// Returns an error if permission, signature, or channel identity validation fails.
    pub async fn pending_removals(&mut self, route: &str) -> Result<Vec<PolicyRemoval>> {
        self.ensure_session().await?;
        let channel: Channel = self.get(&format!("/api/v1/channels/{route}")).await?;
        let records: Vec<PolicyRemoval> = self
            .get(&format!("/api/v1/channels/{route}/pending-removals"))
            .await?;
        for record in &records {
            let device = match record {
                PolicyRemoval::Revocation(record) => &record.value.revoked_device_id,
                PolicyRemoval::Account { device_id, .. } => device_id,
            };
            let contact = channel
                .members
                .iter()
                .find(|c| &c.authorization.value.device_id == device)
                .context("revoked device is absent from the MLS roster")?;
            self.observe_contact(contact)?;
            if let PolicyRemoval::Account { proof, .. } = record {
                let root = &contact.authorization.value.root_public_key;
                let (version, server, account, issued_at) = match proof {
                    DepartureProof::Drop(record) => {
                        record.verify(root)?;
                        let v = &record.value;
                        (v.version, &v.server_id, &v.account_id, v.issued_at)
                    }
                    DepartureProof::Burn(record) => {
                        record.verify(root)?;
                        let v = &record.value;
                        (v.version, &v.server_id, &v.account_id, v.issued_at)
                    }
                };
                ensure!(
                    version == 1
                        && server == &self.durable.server_id
                        && account == &contact.authorization.value.account_id
                        && issued_at
                            <= now()?.saturating_add(cords_identity::STATEMENT_CLOCK_SKEW_SECONDS),
                    "invalid root departure evidence"
                );
                continue;
            }
            let PolicyRemoval::Revocation(record) = record else {
                unreachable!()
            };
            record.verify(&contact.authorization.value.root_public_key)?;
            ensure!(
                record.value.version == 1
                    && record.value.account_id == contact.authorization.value.account_id
                    && record.value.generation > contact.authorization.value.generation
                    && record.value.issued_at
                        <= now()?.saturating_add(cords_identity::STATEMENT_CLOCK_SKEW_SECONDS),
                "invalid channel device revocation"
            );
        }
        Ok(records)
    }
    /// Permanently revoke this device using its account root; local history remains readable.
    /// # Errors
    /// Returns an error if the root statement or persistent server transition is rejected.
    pub async fn revoke_device(&mut self) -> Result<()> {
        self.flush().await?;
        if self.durable.revoked {
            return Ok(());
        }
        let request = RevocationRequest {
            record: self.identity.revoke(now()?)?,
            idempotency_key: id(),
        };
        self.queue("/api/v1/devices/revoke".into(), request, "revoke", "")
            .await?;
        Ok(())
    }
    async fn change_member(
        &mut self,
        route: &str,
        device: &str,
        action: RosterAction,
    ) -> Result<()> {
        self.ensure_session().await?;
        self.flush().await?;
        self.sync_route(route).await?;
        if self.durable.reserved_roster.is_none() {
            self.queue(
                format!("/api/v1/channels/{route}/roster-requests"),
                RosterRequest {
                    action,
                    target_device_id: device.into(),
                    idempotency_key: id(),
                },
                "roster",
                route,
            )
            .await?;
        }
        let (reserved_route, operation) = self
            .durable
            .reserved_roster
            .clone()
            .context("missing reserved roster operation")?;
        ensure!(
            reserved_route == route
                && operation.target.authorization.value.device_id == device
                && operation.action == action,
            "finish the previously approved roster operation before adding another device"
        );
        self.observe_contact(&operation.target)?;
        ensure!(
            operation.base_epoch == self.crypto.epoch(route)?,
            "MLS epoch differs from server epoch"
        );
        let commit = if action == RosterAction::Add {
            let package = decode(&operation.key_package)?;
            ConversationCrypto::validate_key_package(
                &package,
                device,
                &decode(&operation.target.mls_binding.value.mls_public_key)?,
            )?;
            self.crypto.add(route, &package)?
        } else {
            cords_crypto::conversation::Commit {
                commit: self.crypto.remove(route, device)?,
                welcome: Vec::new(),
            }
        };
        let binding = self.identity.sign_device(GroupBinding {
            version: 1,
            server_id: self.durable.server_id.clone(),
            channel_id: route.into(),
            creator_device_id: self.identity.authorization.value.device_id.clone(),
            group_id: route.into(),
        })?;
        self.queue(
            format!("/api/v1/channels/{route}/commits"),
            CommitUpload {
                operation_id: operation.operation_id,
                base_epoch: operation.base_epoch,
                commit: encode(commit.commit),
                welcome: encode(commit.welcome),
                binding,
                idempotency_key: id(),
            },
            "commit",
            route,
        )
        .await?;
        self.validate_roster(route).await?;
        self.persist(&[], &[]).await?;
        self.sync_route(route).await?;
        Ok(())
    }
    /// # Errors
    /// Returns an error on invalid trust or protocol state, unavailable transport, or failed authenticated storage. Reload durable state after a failed operation.
    pub async fn join_channel(&mut self, route: &str) -> Result<()> {
        self.ensure_session().await?;
        let channel = self.fetch_channel(route).await?;
        if channel.locally_archived {
            return Ok(());
        }
        if channel.confidentiality_mode == ConfidentialityMode::Public {
            self.durable.cursors.entry(route.into()).or_insert(0);
            self.persist(&[], &[]).await?;
            self.sync_route(route).await?;
            return Ok(());
        }
        if self.durable.cursors.contains_key(route) {
            return Ok(());
        }
        let welcome: Welcome = self
            .get(&format!("/api/v1/channels/{route}/welcome"))
            .await?;
        self.crypto.join(route, &decode(&welcome.welcome)?)?;
        self.validate_roster(route).await?;
        self.durable.cursors.insert(route.into(), welcome.sequence);
        self.persist(&[], &[]).await?;
        Ok(())
    }
    /// # Errors
    /// Returns an error on invalid trust or protocol state, unavailable transport, or failed authenticated storage. Reload durable state after a failed operation.
    pub async fn send(&mut self, route: &str, body: &str) -> Result<String> {
        self.ensure_session().await?;
        self.flush().await?;
        self.sync_route(route).await?;
        let channel = self.fetch_channel(route).await?;
        ensure!(
            !channel.locally_archived,
            "CHANNEL_RETIRED: archived channels are read-only"
        );
        ensure!(
            !channel.requires_public_acknowledgement,
            "PUBLIC_ACKNOWLEDGEMENT_REQUIRED: future messages will not be end-to-end encrypted"
        );
        ensure!(body.len() <= MAX_TEXT, "message exceeds text limit");
        let message = Message {
            schema_version: 1,
            message_id: id(),
            sender_account_id: self.identity.authorization.value.account_id.clone(),
            sender_device_id: self.identity.authorization.value.device_id.clone(),
            client_timestamp: now()?,
            event_kind: "message.create".into(),
            body: body.into(),
        };
        let session = self
            .durable
            .session
            .as_ref()
            .context("missing membership")?;
        let mut request = EventUpload {
            protocol_version: 1,
            event_id: id(),
            route_kind: "channel".into(),
            route_id: route.into(),
            sender_member_id: session.membership.value.member_id.clone(),
            sender_device_id: message.sender_device_id.clone(),
            client_created_at: message.client_timestamp,
            content_encoding: "mls.application".into(),
            ciphertext: String::new(),
            idempotency_key: id(),
            public_message: None,
        };
        if channel.confidentiality_mode == ConfidentialityMode::Public {
            request.content_encoding = "public.signed".into();
            request.public_message = Some(self.identity.sign_device(PublicMessage {
                version: 1,
                server_id: self.durable.server_id.clone(),
                channel_id: route.into(),
                event_id: request.event_id.clone(),
                idempotency_key: request.idempotency_key.clone(),
                contact: self.identity.contact(&self.crypto.public_key())?,
                membership: session.membership.clone(),
                message: message.clone(),
            })?);
        } else {
            request.ciphertext = encode(
                self.crypto
                    .encrypt(route, &Zeroizing::new(serde_json::to_vec(&message)?))?,
            );
        }
        self.durable
            .own_events
            .insert(request.event_id.clone(), message.message_id.clone());
        self.durable.last_upload = Some(request.clone());
        self.durable.pending = Some(Pending {
            path: format!("/api/v1/routes/{route}/events"),
            body: serde_json::to_value(&request)?,
            kind: "application".into(),
            route: route.into(),
        });
        self.persist(&[(route.into(), 0, message.clone())], &[])
            .await?;
        crash_boundary("outbox_persisted", "application");
        self.flush().await?;
        self.sync_route(route).await?;
        Ok(message.message_id)
    }
    /// # Errors
    /// Returns an error on invalid trust or protocol state, unavailable transport, or failed authenticated storage. Reload durable state after a failed operation.
    pub async fn retry_last(&mut self) -> Result<RouteEvent> {
        self.ensure_session().await?;
        let request = self
            .durable
            .last_upload
            .as_ref()
            .context("no previous upload")?;
        self.post(
            &format!("/api/v1/routes/{}/events", request.route_id),
            request,
            true,
        )
        .await
    }
    /// # Errors
    /// Returns an error on invalid trust or protocol state, unavailable transport, or failed authenticated storage. Reload durable state after a failed operation.
    pub async fn sync_route(&mut self, route: &str) -> Result<SyncResult> {
        let channel = self.fetch_channel(route).await?;
        if channel.locally_archived {
            return Ok(SyncResult {
                fetched: 0,
                messages: Vec::new(),
            });
        }
        let mut fetched = 0;
        let mut messages = Vec::new();
        loop {
            let after = *self
                .durable
                .cursors
                .get(route)
                .context("channel has not been joined")?;
            let events: Vec<RouteEvent> = self
                .get(&format!("/api/v1/routes/{route}/events?after={after}"))
                .await?;
            if events.is_empty() {
                break;
            }
            for event in events {
                let cursor = *self.durable.cursors.get(route).context("missing cursor")?;
                if event.server_sequence <= cursor {
                    continue;
                }
                ensure!(
                    event.server_sequence == cursor + 1 && event.envelope.route_id == route,
                    "CORDS_MLS_EPOCH_GAP"
                );
                if channel.confidentiality_mode == ConfidentialityMode::Public {
                    let signed = event
                        .envelope
                        .public_message
                        .as_ref()
                        .context("missing signed public message")?;
                    signed.validate(&self.durable.server_id, route, &self.durable.server_key)?;
                    let v = &signed.value;
                    ensure!(
                        event.envelope.protocol_version == 1
                            && event.envelope.route_kind == "channel"
                            && event.envelope.content_encoding == "public.signed"
                            && event.envelope.ciphertext.is_empty()
                            && v.event_id == event.envelope.event_id
                            && v.idempotency_key == event.envelope.idempotency_key
                            && v.membership.value.member_id == event.envelope.sender_member_id
                            && v.message.sender_device_id == event.envelope.sender_device_id
                            && v.message.client_timestamp == event.envelope.client_created_at,
                        "invalid public envelope binding"
                    );
                    self.observe_contact(&v.contact)?;
                    let own = self.durable.own_events.get(&event.envelope.event_id);
                    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM message_cache WHERE server_id=?1 AND route_id=?2 AND message_id=?3)")
                        .bind(&self.durable.server_id).bind(route).bind(&v.message.message_id).fetch_one(self.store.pool()).await?;
                    ensure!(
                        !exists || own == Some(&v.message.message_id),
                        "replayed public message identifier"
                    );
                    if let Some(own) = own {
                        ensure!(
                            canonical(&self.cached_message(route, own).await?)?
                                == canonical(&v.message)?,
                            "altered own public message"
                        );
                    }
                    self.durable
                        .cursors
                        .insert(route.into(), event.server_sequence);
                    self.persist(
                        &[(route.into(), event.server_sequence, v.message.clone())],
                        std::slice::from_ref(&event),
                    )
                    .await?;
                    messages.push(v.message.clone());
                    fetched += 1;
                    continue;
                }
                ensure!(
                    event.envelope.public_message.is_none()
                        && matches!(
                            event.envelope.content_encoding.as_str(),
                            "mls.application" | "mls.commit"
                        ),
                    "encrypted channel received non-MLS data"
                );
                if let Some(own) = self
                    .durable
                    .own_events
                    .get(&event.envelope.event_id)
                    .cloned()
                {
                    if own == "commit" {
                        self.durable
                            .cursors
                            .insert(route.into(), event.server_sequence);
                        self.persist(&[], std::slice::from_ref(&event)).await?;
                    } else {
                        let message = self.cached_message(route, &own).await?;
                        self.durable
                            .cursors
                            .insert(route.into(), event.server_sequence);
                        self.persist(
                            &[(route.into(), event.server_sequence, message)],
                            std::slice::from_ref(&event),
                        )
                        .await?;
                    }
                } else {
                    let result = self
                        .crypto
                        .process(route, &decode(&event.envelope.ciphertext)?)?;
                    let mut saved = Vec::new();
                    match result {
                        Processed::Application {
                            device_id,
                            plaintext,
                        } => {
                            let message: Message = serde_json::from_slice(&plaintext)?;
                            ensure!(
                                message.schema_version == 1
                                    && message.event_kind == "message.create"
                                    && uuid::Uuid::parse_str(&message.message_id).is_ok()
                                    && message.body.len() <= MAX_TEXT
                                    && message.sender_device_id == device_id
                                    && event.envelope.sender_device_id == device_id,
                                "invalid encrypted message identity"
                            );
                            let epoch = self.crypto.epoch(route)?;
                            let channel: Channel = self
                                .get(&format!("/api/v1/channels/{route}?epoch={epoch}"))
                                .await?;
                            let sender = channel
                                .members
                                .iter()
                                .find(|c| c.authorization.value.device_id == device_id)
                                .context("sender is not authorized")?;
                            self.observe_contact(sender)?;
                            ensure!(
                                message.sender_account_id == sender.authorization.value.account_id,
                                "sender account mismatch"
                            );
                            let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM message_cache WHERE server_id=?1 AND route_id=?2 AND message_id=?3)")
                                .bind(&self.durable.server_id).bind(route).bind(&message.message_id).fetch_one(self.store.pool()).await?;
                            ensure!(!exists, "duplicate encrypted message identifier");
                            saved.push((route.into(), event.server_sequence, message.clone()));
                            messages.push(message);
                        }
                        Processed::Commit => {
                            self.validate_roster(route).await?;
                        }
                    }
                    self.durable
                        .cursors
                        .insert(route.into(), event.server_sequence);
                    self.persist(&saved, std::slice::from_ref(&event)).await?;
                }
                fetched += 1;
            }
        }
        Ok(SyncResult { fetched, messages })
    }
    /// # Errors
    /// Returns an error on invalid trust or protocol state, unavailable transport, or failed authenticated storage. Reload durable state after a failed operation.
    pub async fn synchronize(&mut self) -> Result<SyncResult> {
        self.ensure_session().await?;
        self.flush().await?;
        let routes: Vec<_> = self.durable.cursors.keys().cloned().collect();
        let mut result = SyncResult {
            fetched: 0,
            messages: Vec::new(),
        };
        for route in routes {
            let next = self.sync_route(&route).await?;
            result.fetched += next.fetched;
            result.messages.extend(next.messages);
            let channel = self.fetch_channel(&route).await?;
            if channel.locally_archived
                || channel.confidentiality_mode == ConfidentialityMode::Public
            {
                continue;
            }
            let manage = self.durable.session.as_ref().is_some_and(|session| {
                session
                    .membership
                    .value
                    .capabilities
                    .iter()
                    .any(|cap| cap == "channel.manage")
            });
            if channel.creator_device_id == self.identity.authorization.value.device_id || manage {
                let removals = self.pending_removals(&route).await?;
                let mut targets = std::collections::BTreeSet::new();
                for removal in removals {
                    let device = match removal {
                        PolicyRemoval::Revocation(record) => record.value.revoked_device_id,
                        PolicyRemoval::Account { device_id, .. } => device_id,
                    };
                    targets.insert(device);
                }
                for target in targets {
                    self.remove_member(&route, &target).await?;
                }
            }
        }
        Ok(result)
    }
    async fn cached_message(&self, route: &str, message: &str) -> Result<Message> {
        let sealed: Vec<u8> = sqlx::query_scalar(
            "SELECT sealed FROM message_cache WHERE server_id=?1 AND route_id=?2 AND message_id=?3",
        )
        .bind(&self.durable.server_id)
        .bind(route)
        .bind(message)
        .fetch_one(self.store.pool())
        .await?;
        Ok(serde_json::from_slice(&protection::open(
            &self.key,
            format!(
                "{}/cache/{}/{route}/{message}/v2",
                self.installation, self.durable.server_id
            )
            .as_bytes(),
            &sealed,
        )?)?)
    }
    /// # Errors
    /// Returns an error on invalid trust or protocol state, unavailable transport, or failed authenticated storage. Reload durable state after a failed operation.
    pub async fn history(&self, route: &str) -> Result<Vec<Message>> {
        if let Some(local) = self
            .durable
            .channel_identities
            .get(&format!("{}/{}", self.durable.server_id, route))
        {
            if local.archived {
                return Ok(local.retained_history.clone());
            }
        }
        let ids: Vec<String> = sqlx::query_scalar(
            "SELECT message_id FROM message_cache WHERE server_id=?1 AND route_id=?2 ORDER BY sequence,message_id",
        )
        .bind(&self.durable.server_id)
        .bind(route)
        .fetch_all(self.store.pool())
        .await?;
        let mut messages = Vec::new();
        for id in ids {
            messages.push(self.cached_message(route, &id).await?);
        }
        Ok(messages)
    }
    /// # Errors
    /// Returns an error on invalid trust or protocol state, unavailable transport, or failed authenticated storage. Reload durable state after a failed operation.
    pub async fn notifications(
        &mut self,
    ) -> Result<(
        tokio::task::JoinHandle<()>,
        tokio::sync::mpsc::Receiver<Notification>,
    )> {
        self.ensure_session().await?;
        let url = format!(
            "{}/api/v1/events",
            self.durable.origin.replacen("https://", "wss://", 1)
        );
        let mut request = url.into_client_request()?;
        request.headers_mut().insert(
            "authorization",
            format!(
                "Bearer {}",
                self.durable
                    .session
                    .as_ref()
                    .context("not authenticated")?
                    .token
            )
            .parse()?,
        );
        request
            .headers_mut()
            .insert("sec-websocket-protocol", WS_PROTOCOL.parse()?);
        let mut socket_config =
            tokio_tungstenite::tungstenite::protocol::WebSocketConfig::default();
        socket_config.max_message_size = Some(4096);
        socket_config.max_frame_size = Some(4096);
        let (mut socket, response) = connect_async_tls_with_config(
            request,
            Some(socket_config),
            false,
            Some(Connector::Rustls(self.tls.clone())),
        )
        .await?;
        ensure!(
            response
                .headers()
                .get("sec-websocket-protocol")
                .is_some_and(|v| v == WS_PROTOCOL),
            "WebSocket subprotocol not negotiated"
        );
        let (send, receive) = tokio::sync::mpsc::channel(32);
        let task = tokio::spawn(async move {
            while let Some(frame) = socket.next().await {
                match frame {
                    Ok(SocketMessage::Binary(bytes)) => {
                        if bytes.len() > 4096 {
                            break;
                        }
                        let mut decoder = minicbor::Decoder::new(&bytes);
                        let valid = (|| -> Result<()> {
                            ensure!(
                                decoder.array()? == Some(4)
                                    && decoder.u16()? == 1
                                    && decoder.str()? == "route.advanced",
                                "invalid notification"
                            );
                            decoder.str()?;
                            decoder.u64()?;
                            ensure!(
                                decoder.position() == bytes.len(),
                                "trailing notification bytes"
                            );
                            Ok(())
                        })();
                        if valid.is_err() {
                            break;
                        }
                        if send.send(Notification::Advanced).await.is_err() {
                            break;
                        }
                    }
                    Ok(SocketMessage::Ping(payload)) => {
                        if socket.send(SocketMessage::Pong(payload)).await.is_err() {
                            break;
                        }
                    }
                    Ok(SocketMessage::Pong(_)) => {}
                    _ => break,
                }
            }
            let _ = send.send(Notification::Disconnected).await;
        });
        Ok((task, receive))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn migrations() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../migrations/sqlite")
    }
    const PASS: &[u8] = b"independent test installation passphrase";
    async fn seed_server(client: &mut Client, server: &str, body: &str) -> Result<()> {
        client.clear_active_server()?;
        client.durable.server_id = server.into();
        client.durable.origin = "https://127.0.0.1:9".into();
        client.durable.server_key = "unreachable-test-pin".into();
        client.durable.ownership_state = "CLAIMED".into();
        client.durable.ownership_generation = 2;
        client.crypto.create("same-route")?;
        client.durable.cursors.insert("same-route".into(), 1);
        let message = Message {
            schema_version: 1,
            message_id: "same-message".into(),
            sender_account_id: client.status().account_id,
            sender_device_id: client.status().device_id,
            client_timestamp: now()?,
            event_kind: "message.create".into(),
            body: body.into(),
        };
        client
            .persist(&[("same-route".into(), 1, message)], &[])
            .await
    }
    #[tokio::test]
    async fn legacy_cache_is_rekeyed_under_the_original_pin_before_server_switching() -> Result<()>
    {
        let temp = tempfile::tempdir()?;
        let mut client = Client::open(temp.path(), &migrations(), Some(PASS), None).await?;
        seed_server(&mut client, "original-pin", "original history").await?;
        let message = Message {
            schema_version: 1,
            message_id: "legacy-message".into(),
            sender_account_id: client.status().account_id,
            sender_device_id: client.status().device_id,
            client_timestamp: now()?,
            event_kind: "message.create".into(),
            body: "retained legacy encrypted history".into(),
        };
        let context = format!(
            "{}/cache/legacy-route/legacy-message/v1",
            client.installation
        );
        let sealed = protection::seal(
            &client.key,
            context.as_bytes(),
            &serde_json::to_vec(&message)?,
        )?;
        sqlx::query("INSERT INTO message_cache(server_id,route_id,message_id,sequence,sealed) VALUES('','legacy-route','legacy-message',1,?1)")
            .bind(sealed).execute(client.store.pool()).await?;
        client.shutdown().await;
        let client = Client::open(temp.path(), &migrations(), Some(PASS), None).await?;
        assert_eq!(client.history("legacy-route").await?[0].body, message.body);
        let server: String = sqlx::query_scalar(
            "SELECT server_id FROM message_cache WHERE message_id='legacy-message'",
        )
        .fetch_one(client.store.pool())
        .await?;
        assert_eq!(server, "original-pin");
        Ok(())
    }
    #[tokio::test]
    async fn multiserver_cache_restart_local_remove_and_archive_are_isolated() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let mut client = Client::open(temp.path(), &migrations(), Some(PASS), None).await?;
        seed_server(&mut client, "server-a", "only server A").await?;
        seed_server(&mut client, "server-b", "only server B").await?;
        client.select_server("server-a").await?;
        assert_eq!(client.history("same-route").await?[0].body, "only server A");
        client.shutdown().await;
        let mut client = Client::open(temp.path(), &migrations(), Some(PASS), None).await?;
        assert_eq!(client.servers().len(), 2);
        client.select_server("server-b").await?;
        assert_eq!(client.history("same-route").await?[0].body, "only server B");
        assert!(
            !client
                .remove_server("server-a", true)
                .await?
                .remote_confirmed
        );
        assert_eq!(client.history("same-route").await?[0].body, "only server B");
        let outcome = client.archive_server("server-b").await?;
        assert!(!outcome.remote_confirmed);
        assert!(outcome.warning.is_some());
        assert!(client.status().server_id.is_empty());
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM message_cache")
            .fetch_one(client.store.pool())
            .await?;
        assert_eq!(count, 0);
        client.shutdown().await;
        let mut client = Client::open(temp.path(), &migrations(), Some(PASS), None).await?;
        assert_eq!(client.servers().len(), 1);
        assert!(client.servers()[0].archived);
        assert_eq!(client.servers()[0].server_id, "server-b");
        assert!(client.select_server("server-b").await.is_err());
        assert!(client.status().server_id.is_empty());
        Ok(())
    }
    #[tokio::test]
    async fn deselected_server_remains_trusted_and_can_be_reselected_after_restart() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let mut client = Client::open(temp.path(), &migrations(), Some(PASS), None).await?;
        seed_server(&mut client, "server-a", "retained server A history").await?;

        let status = client.deselect_server().await?;
        assert!(status.server_id.is_empty());
        assert_eq!(client.servers().len(), 1);
        assert!(!client.servers()[0].active);
        client.shutdown().await;

        let mut client = Client::open(temp.path(), &migrations(), Some(PASS), None).await?;
        assert!(client.status().server_id.is_empty());
        assert!(!client.servers()[0].active);
        client.select_server("server-a").await?;
        assert_eq!(client.status().server_id, "server-a");
        assert_eq!(
            client.history("same-route").await?[0].body,
            "retained server A history"
        );
        Ok(())
    }
    #[tokio::test]
    async fn user_card_publishes_avatar_url_without_local_png_cache() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let mut client = Client::open(temp.path(), &migrations(), Some(PASS), None).await?;
        client
            .save_ui_preferences(serde_json::json!({
                "version": 1,
                "displayName": "URL profile",
                "avatar": {
                    "url": "https://images.example/profile.webp",
                    "data": "data:image/png;base64,local-only-cache",
                    "shape": "square",
                    "x": 25,
                    "y": 75,
                    "zoom": 1.5
                }
            }))
            .await?;

        let contact = client.self_contact()?;
        let avatar = contact
            .user_card
            .context("missing user card")?
            .value
            .avatar
            .context("missing avatar")?;
        assert_eq!(avatar.url, "https://images.example/profile.webp");
        assert!(avatar.data.is_empty());
        let encoded = serde_json::to_string(&avatar)?;
        assert!(!encoded.contains("data:image"));
        assert!(!encoded.contains("\"data\""));
        assert_eq!((avatar.x, avatar.y, avatar.zoom_milli), (25, 75, 1500));
        Ok(())
    }
    #[tokio::test]
    async fn burn_delivery_failures_persist_backoff_and_disable_ordinary_requests() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let mut client = Client::open(temp.path(), &migrations(), Some(PASS), None).await?;
        seed_server(&mut client, "server-a", "A").await?;
        seed_server(&mut client, "server-b", "B").await?;
        let result = client.burn_identity().await?;
        assert_eq!(result.confirmed, 0);
        assert_eq!(result.pending.len(), 2);
        let notice = &client.durable.pending_burns["server-a"];
        assert_eq!(notice.attempts, 1);
        assert!(notice.last_error.is_some());
        assert!(notice.next_retry_at > now()?);
        assert!(client.authenticate().await.is_err());
        assert!(client.flush().await.is_err());
        assert!(client.select_server("server-a").await.is_err());
        client.shutdown().await;
        let mut client = Client::open(temp.path(), &migrations(), Some(PASS), None).await?;
        assert!(client.is_burned());
        client.retry_pending_burns().await?;
        assert!(
            client
                .durable
                .pending_burns
                .values()
                .all(|notice| notice.attempts == 1)
        );
        assert_eq!(
            client.identity_view()["burn_deliveries"]
                .as_array()
                .context("missing burn delivery state")?
                .len(),
            2
        );
        Ok(())
    }
    #[tokio::test]
    async fn persisted_identity_lock_wrong_passphrase_and_missing_state() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let mut a = Client::open(temp.path(), &migrations(), Some(PASS), None).await?;
        let expected = a.status();
        let preferences = serde_json::json!({"version":1,"displayName":"protected local profile","jewelOpensDms":false});
        a.save_ui_preferences(preferences.clone()).await?;
        assert!(
            a.save_ui_preferences(serde_json::json!([1, 2]))
                .await
                .is_err()
        );
        let sealed: Vec<u8> = sqlx::query_scalar("SELECT sealed FROM client_state")
            .fetch_one(a.store.pool())
            .await?;
        assert!(!sealed.windows(23).any(|v| v == b"protected local profile"));
        assert!(a.identity_view().get("token").is_none());
        assert!(
            Client::open(temp.path(), &migrations(), Some(PASS), None)
                .await
                .is_err()
        );
        a.store.pool().close().await;
        drop(a);
        assert!(
            Client::open(
                temp.path(),
                &migrations(),
                Some(b"incorrect passphrase"),
                None
            )
            .await
            .is_err()
        );
        let a = Client::open(temp.path(), &migrations(), Some(PASS), None).await?;
        assert_eq!(a.status().device_id, expected.device_id);
        assert_eq!(a.status().account_id, expected.account_id);
        assert_eq!(a.ui_preferences(), preferences);
        sqlx::query("DELETE FROM client_state")
            .execute(a.store.pool())
            .await?;
        a.store.pool().close().await;
        drop(a);
        assert!(
            Client::open(temp.path(), &migrations(), Some(PASS), None)
                .await
                .is_err()
        );
        Ok(())
    }
    #[tokio::test]
    async fn failed_sqlite_commit_restores_cursor_cache_and_mls_together() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let mut a = Client::open(temp.path(), &migrations(), Some(PASS), None).await?;
        let route = "atomic-test";
        a.crypto.create(route)?;
        a.durable.cursors.insert(route.into(), 0);
        a.persist(&[], &[]).await?;
        let before = a.crypto.snapshot()?;
        let message = Message {
            schema_version: 1,
            message_id: id(),
            sender_account_id: a.status().account_id,
            sender_device_id: a.status().device_id,
            client_timestamp: now()?,
            event_kind: "message.create".into(),
            body: "independently protected cache".into(),
        };
        sqlx::raw_sql("CREATE TRIGGER injected_failure BEFORE INSERT ON message_cache BEGIN SELECT RAISE(ABORT,'injected storage failure'); END;").execute(a.store.pool()).await?;
        a.crypto.encrypt(route, b"tentative message")?;
        a.durable.cursors.insert(route.into(), 1);
        assert!(
            a.persist(&[(route.into(), 1, message.clone())], &[])
                .await
                .is_err()
        );
        a.reload().await?;
        assert_eq!(a.durable.cursors[route], 0);
        assert_eq!(*before, *a.crypto.snapshot()?);
        assert!(a.history(route).await?.is_empty());
        sqlx::raw_sql("DROP TRIGGER injected_failure")
            .execute(a.store.pool())
            .await?;
        a.persist(&[(route.into(), 1, message.clone())], &[])
            .await?;
        let encrypted: Vec<u8> = sqlx::query_scalar("SELECT sealed FROM message_cache")
            .fetch_one(a.store.pool())
            .await?;
        assert!(
            !encrypted
                .windows(message.body.len())
                .any(|v| v == message.body.as_bytes())
        );
        // A cache entry still opens under the installation key after all MLS state is discarded.
        a.crypto = ConversationCrypto::new(a.status().device_id)?;
        assert_eq!(a.history(route).await?[0].body, message.body);
        let mut damaged = encrypted;
        damaged[30] ^= 1;
        sqlx::query("UPDATE message_cache SET sealed=?1")
            .bind(damaged)
            .execute(a.store.pool())
            .await?;
        assert!(a.history(route).await.is_err());
        Ok(())
    }
    #[tokio::test]
    async fn channel_identity_conflict_tombstone_survives_restart() -> Result<()> {
        use cords_protocol::messaging::{ChannelIdentity, ChannelTransition};
        let temp = tempfile::tempdir()?;
        let server = Identity::generate(now()?)?;
        let mut client = Client::open(temp.path(), &migrations(), Some(PASS), None).await?;
        client.durable.server_id = "test-server".into();
        client.durable.server_key = server.authorization.value.root_public_key.clone();
        client.durable.origin = "https://example.com".into();
        let route = id();
        let mut channel = Channel {
            channel_id: route.clone(),
            name: "same display name".into(),
            creator_device_id: client.status().device_id,
            epoch: 0,
            members: Vec::new(),
            binding: None,
            confidentiality_mode: ConfidentialityMode::Encrypted,
            identity: None,
            transition: None,
            requires_public_acknowledgement: false,
            locally_archived: false,
        };
        channel.identity = Some(server.sign_root(ChannelIdentity {
            version: 1,
            server_id: "test-server".into(),
            channel_id: route.clone(),
            name: channel.name.clone(),
            creator_device_id: channel.creator_device_id.clone(),
            confidentiality_mode: ConfidentialityMode::Encrypted,
            created_at: 1,
        })?);
        channel.transition = Some(server.sign_root(ChannelTransition {
            version: 1,
            server_id: "test-server".into(),
            channel_id: route.clone(),
            retired: false,
            generation: 0,
            succession: None,
        })?);
        client.observe_channel(channel.clone()).await?;
        let message = Message {
            schema_version: 1,
            message_id: id(),
            sender_account_id: client.status().account_id,
            sender_device_id: client.status().device_id,
            client_timestamp: now()?,
            event_kind: "message.create".into(),
            body: "SEALED_ARCHIVE_SECRET".into(),
        };
        client.persist(&[(route.clone(), 1, message)], &[]).await?;
        channel.transition = Some(server.sign_root(ChannelTransition {
            version: 1,
            server_id: "test-server".into(),
            channel_id: route.clone(),
            retired: true,
            generation: 2,
            succession: None,
        })?);
        assert!(
            client
                .observe_channel(channel.clone())
                .await?
                .locally_archived
        );
        assert_eq!(
            client
                .archived_channel_history("test-server", &route)?
                .len(),
            1
        );
        client.shutdown().await;
        let mut client = Client::open(temp.path(), &migrations(), Some(PASS), None).await?;
        let mut conflicting = channel.clone();
        conflicting.confidentiality_mode = ConfidentialityMode::Public;
        let mut identity = conflicting
            .identity
            .as_ref()
            .context("identity")?
            .value
            .clone();
        identity.confidentiality_mode = ConfidentialityMode::Public;
        conflicting.identity = Some(server.sign_root(identity)?);
        let error = client
            .observe_channel(conflicting)
            .await
            .err()
            .context("accepted identity reuse")?;
        assert!(error.to_string().contains("Channel Identity Conflict"));
        let mut rollback = channel.clone();
        rollback.transition = Some(server.sign_root(ChannelTransition {
            version: 1,
            server_id: "test-server".into(),
            channel_id: route.clone(),
            retired: false,
            generation: 0,
            succession: None,
        })?);
        assert!(client.observe_channel(rollback).await.is_err());
        assert_eq!(
            client.channel_archives()[0].confidentiality_mode,
            ConfidentialityMode::Encrypted
        );
        assert_eq!(
            client.history(&route).await?[0].body,
            "SEALED_ARCHIVE_SECRET"
        );
        Ok(())
    }
}
