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
        Challenge, ChallengeRequest, Channel, ChannelBind, ChannelCreate, CommitUpload, Contact,
        DeviceAuthorization, DeviceRevocation, EventUpload, GroupBinding, KeyPackageUpload,
        MAX_TEXT, MembershipRequest, Message, OwnershipClaim, OwnershipProof, OwnershipState,
        RevocationRequest, RosterAction, RosterOperation, RosterRequest, RouteEvent, Session,
        SessionRequest, Signed, WS_PROTOCOL, canonical, decode, encode, hash,
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
    join_policy: Vec<String>,
    session: Option<Session>,
    cursors: BTreeMap<String, u64>,
    contacts: BTreeMap<String, Signed<DeviceAuthorization>>,
    pending: Option<Pending>,
    own_events: BTreeMap<String, String>,
    last_upload: Option<EventUpload>,
    #[serde(default)]
    reserved_roster: Option<(String, RosterOperation)>,
}
impl Drop for Durable {
    fn drop(&mut self) {
        self.identity.zeroize();
        self.crypto.zeroize();
        if let Some(session) = self.session.as_mut() {
            session.token.zeroize();
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
        })
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
            join_policy: self.durable.join_policy.clone(),
            cursors: self.durable.cursors.clone(),
        }
    }
    async fn persist(
        &mut self,
        messages: &[(String, u64, Message)],
        events: &[RouteEvent],
    ) -> Result<()> {
        self.durable.identity = encode(self.identity.export_secret()?.as_slice());
        self.durable.crypto = encode(self.crypto.snapshot()?.as_slice());
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
                "{}/cache/{route}/{}/v1",
                self.installation, message.message_id
            );
            let sealed = protection::seal(
                &self.key,
                context.as_bytes(),
                &Zeroizing::new(serde_json::to_vec(message)?),
            )?;
            sqlx::query("INSERT INTO message_cache(route_id,message_id,sequence,sealed) VALUES(?1,?2,?3,?4) ON CONFLICT(route_id,message_id) DO UPDATE SET sequence=excluded.sequence,sealed=excluded.sealed")
                .bind(route).bind(&message.message_id).bind(i64::try_from(*sequence)?).bind(sealed).execute(&mut *tx).await?;
        }
        for event in events {
            sqlx::query("INSERT INTO ciphertext_cache(route_id,sequence,envelope) VALUES(?1,?2,?3) ON CONFLICT(route_id,sequence) DO NOTHING")
                .bind(&event.envelope.route_id).bind(i64::try_from(event.server_sequence)?).bind(serde_json::to_vec(event)?).execute(&mut *tx).await?;
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
                _ => "CORDS_HTTP_ERROR",
            };
            bail!("{code}: HTTP {status}");
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
            .map_err(|error| anyhow::anyhow!("{path}: {error}"))
    }
    /// # Errors
    /// Returns an error on invalid trust or protocol state, unavailable transport, or failed authenticated storage. Reload durable state after a failed operation.
    pub async fn trust(&mut self, origin: &str) -> Result<Status> {
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
        if !self.durable.server_id.is_empty() {
            ensure!(
                self.durable.origin == origin.to_string() && self.durable.server_id == discovered,
                "CORDS_SERVER_KEY_CHANGED"
            );
        }
        self.durable.origin = origin.to_string();
        self.durable.server_id = discovered;
        self.durable.server_key = signed.metadata.server_signing_key;
        self.durable.join_policy = signed.metadata.join_policy;
        let ownership: Signed<OwnershipState> = Self::response(
            self.http
                .get(origin.join("/api/v1/ownership")?)
                .send()
                .await?,
        )
        .await?;
        ownership.verify(&self.durable.server_key)?;
        ensure!(
            ownership.value.version == 1
                && ownership.value.server_id == self.durable.server_id
                && matches!(ownership.value.state.as_str(), "UNCLAIMED" | "CLAIMED")
                && ownership.value.generation > 0,
            "invalid ownership state"
        );
        self.durable.ownership_state = ownership.value.state;
        self.persist(&[], &[]).await?;
        Ok(self.status())
    }
    /// # Errors
    /// Returns an error on invalid trust or protocol state, unavailable transport, or failed authenticated storage. Reload durable state after a failed operation.
    pub async fn authenticate(&mut self) -> Result<Status> {
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
            "CORDS_SERVER_UNCLAIMED"
        );
        let purpose = if self.durable.session.is_some() {
            "authenticate"
        } else {
            "join"
        };
        let request = ChallengeRequest {
            contact: self.identity.contact(&self.crypto.public_key())?,
            purpose: purpose.into(),
            idempotency_key: id(),
        };
        validate_contact(&request.contact, now()?).context("local identity validation failed")?;
        let challenge: Challenge = self
            .post(
                if purpose == "join" {
                    "/api/v1/join/request"
                } else {
                    "/api/v1/auth/challenge"
                },
                &request,
                false,
            )
            .await?;
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
        let session: Session = self.post("/api/v1/auth/session", &request, false).await?;
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
        self.persist(&[], &[]).await?;
        self.flush().await?;
        Ok(self.status())
    }

    /// Claim persistently unclaimed server ownership with its server-generated one-time code.
    /// # Errors
    /// Returns an error when the claim is unavailable, invalid, already owned by another account,
    /// or the signed replacement membership is invalid.
    pub async fn claim_ownership(&mut self, claim_code: &str) -> Result<Status> {
        ensure!(claim_code.len() == 43, "invalid ownership claim code");
        ensure!(!self.durable.server_id.is_empty(), "trust the server first");
        self.trust(&self.durable.origin.clone()).await?;
        ensure!(
            self.durable.ownership_state == "UNCLAIMED",
            "server is not unclaimed"
        );
        let contact = self.identity.contact(&self.crypto.public_key())?;
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
        self.persist(&[], &[]).await?;
        self.flush().await?;
        Ok(self.status())
    }
    async fn ensure_session(&mut self) -> Result<()> {
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
            self.authenticate().await?;
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
        let mut first_result = None;
        loop {
            let Some(pending) = self.durable.pending.clone() else {
                return Ok(first_result.unwrap_or(serde_json::Value::Null));
            };
            let value: serde_json::Value = self
                .post(&pending.path, &pending.body, pending.kind != "revoke")
                .await?;
            crash_boundary("response_received", &pending.kind);
            first_result.get_or_insert_with(|| value.clone());
            let mut followup = None;
            match pending.kind.as_str() {
                "revoke" => {
                    self.durable.revoked = true;
                    self.durable.session = None;
                }
                "create" => {
                    let route = value.as_str().context("invalid channel response")?;
                    self.crypto.create(route)?;
                    self.durable.cursors.insert(route.into(), 0);
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
        self.ensure_session().await?;
        self.flush().await?;
        let value = self
            .queue(
                "/api/v1/channels".into(),
                ChannelCreate {
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
        self.get("/api/v1/members").await
    }
    /// # Errors
    /// Returns an error on invalid trust or protocol state, unavailable transport, or failed authenticated storage. Reload durable state after a failed operation.
    pub async fn channels(&mut self) -> Result<Vec<Channel>> {
        self.ensure_session().await?;
        self.get("/api/v1/channels").await
    }
    /// List pending requests visible to a server manager.
    /// # Errors
    /// Returns an error when the session lacks server management permission or transport fails.
    pub async fn membership_requests(&mut self) -> Result<Vec<MembershipRequest>> {
        self.ensure_session().await?;
        self.get("/api/v1/membership-requests").await
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
        ensure!(
            channel.channel_id == route && channel.epoch == epoch,
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
    pub async fn pending_removals(&mut self, route: &str) -> Result<Vec<Signed<DeviceRevocation>>> {
        self.ensure_session().await?;
        let channel: Channel = self.get(&format!("/api/v1/channels/{route}")).await?;
        let records: Vec<Signed<DeviceRevocation>> = self
            .get(&format!("/api/v1/channels/{route}/pending-removals"))
            .await?;
        for record in &records {
            let contact = channel
                .members
                .iter()
                .find(|c| c.authorization.value.device_id == record.value.revoked_device_id)
                .context("revoked device is absent from the MLS roster")?;
            self.observe_contact(contact)?;
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
        let bytes = self
            .crypto
            .encrypt(route, &Zeroizing::new(serde_json::to_vec(&message)?))?;
        let request = EventUpload {
            protocol_version: 1,
            event_id: id(),
            route_kind: "channel".into(),
            route_id: route.into(),
            sender_member_id: self
                .durable
                .session
                .as_ref()
                .context("missing membership")?
                .membership
                .value
                .member_id
                .clone(),
            sender_device_id: message.sender_device_id.clone(),
            client_created_at: message.client_timestamp,
            content_encoding: "mls.application".into(),
            ciphertext: encode(bytes),
            idempotency_key: id(),
        };
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
                            let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM message_cache WHERE route_id=?1 AND message_id=?2)")
                                .bind(route).bind(&message.message_id).fetch_one(self.store.pool()).await?;
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
        }
        Ok(result)
    }
    async fn cached_message(&self, route: &str, message: &str) -> Result<Message> {
        let sealed: Vec<u8> = sqlx::query_scalar(
            "SELECT sealed FROM message_cache WHERE route_id=?1 AND message_id=?2",
        )
        .bind(route)
        .bind(message)
        .fetch_one(self.store.pool())
        .await?;
        Ok(serde_json::from_slice(&protection::open(
            &self.key,
            format!("{}/cache/{route}/{message}/v1", self.installation).as_bytes(),
            &sealed,
        )?)?)
    }
    /// # Errors
    /// Returns an error on invalid trust or protocol state, unavailable transport, or failed authenticated storage. Reload durable state after a failed operation.
    pub async fn history(&self, route: &str) -> Result<Vec<Message>> {
        let ids: Vec<String> = sqlx::query_scalar(
            "SELECT message_id FROM message_cache WHERE route_id=?1 ORDER BY sequence,message_id",
        )
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
}
