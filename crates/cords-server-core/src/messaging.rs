//! Authenticated encrypted-channel services. `PostgreSQL` is authoritative.
use axum::{
    Json, Router,
    extract::{
        Path, Query, State, WebSocketUpgrade,
        ws::{Message as WsMessage, WebSocket},
    },
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use cords_identity::{validate_contact, validate_transition};
use cords_protocol::messaging::{
    Challenge, ChallengeRequest, Channel, ChannelCreate, CommitUpload, Contact, EventUpload,
    InvalidObject, KeyPackageUpload, MAX_ENVELOPE, Membership, OwnershipClaim, RosterAction,
    RosterOperation, RosterRequest, RouteEvent, Session, SessionRequest, Signed, Statement,
    WS_PROTOCOL, canonical, decode, encode, hash, notification,
};
use cords_storage::PostgresStore;
use rand_core::{OsRng, RngCore as _};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sqlx::{Postgres, Row as _, Transaction};
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use subtle::ConstantTimeEq as _;
use tokio::sync::broadcast;

type Tx<'a> = Transaction<'a, Postgres>;
type Result<T> = std::result::Result<T, ApiError>;

#[derive(Clone, Debug)]
pub struct Service(Arc<Inner>);
#[derive(Debug)]
struct Inner {
    store: PostgresStore,
    identity: crate::ServerIdentity,
    notifications: broadcast::Sender<(String, u64)>,
    challenge_seconds: u64,
    session_seconds: u64,
    ownership_claim_hash: Option<String>,
    authentication_budget: Mutex<(Instant, u32)>,
}

#[derive(Debug, thiserror::Error)]
#[error("{1}")]
pub struct ApiError(StatusCode, &'static str);
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.0,
            Json(serde_json::json!({"code":self.1,"message":self.1,"retry":"never"})),
        )
            .into_response()
    }
}
impl From<sqlx::Error> for ApiError {
    fn from(error: sqlx::Error) -> Self {
        tracing::error!(kind = ?error.as_database_error().map(sqlx::error::DatabaseError::code), "persistent operation failed");
        Self(StatusCode::SERVICE_UNAVAILABLE, "CORDS_STORAGE_UNAVAILABLE")
    }
}
impl From<serde_json::Error> for ApiError {
    fn from(_: serde_json::Error) -> Self {
        bad()
    }
}
impl From<InvalidObject> for ApiError {
    fn from(_: InvalidObject) -> Self {
        bad()
    }
}
impl From<cords_crypto::conversation::CryptoError> for ApiError {
    fn from(_: cords_crypto::conversation::CryptoError) -> Self {
        bad()
    }
}
fn bad() -> ApiError {
    ApiError(StatusCode::BAD_REQUEST, "CORDS_INVALID_OBJECT")
}
fn denied() -> ApiError {
    ApiError(StatusCode::FORBIDDEN, "CORDS_PERMISSION_DENIED")
}
fn conflict() -> ApiError {
    ApiError(StatusCode::CONFLICT, "CORDS_STATE_CONFLICT")
}
fn now() -> Result<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|v| v.as_secs())
        .map_err(|_| bad())
}
fn int(v: u64) -> Result<i64> {
    i64::try_from(v).map_err(|_| bad())
}
fn uint(v: i64) -> Result<u64> {
    u64::try_from(v).map_err(|_| bad())
}
fn id() -> String {
    uuid::Uuid::now_v7().to_string()
}
fn json<T: Serialize>(v: &T) -> Result<String> {
    Ok(serde_json::to_string(v)?)
}
fn parse<T: DeserializeOwned>(v: &str) -> Result<T> {
    Ok(serde_json::from_str(v)?)
}

impl Service {
    /// # Errors
    /// Returns an error when authentication lifetime settings are outside supported bounds.
    pub fn new(
        store: PostgresStore,
        identity: crate::ServerIdentity,
        challenge_seconds: u64,
        session_seconds: u64,
        ownership_claim_code: Option<&str>,
    ) -> Result<Self> {
        if !(1..=600).contains(&challenge_seconds) || !(1..=86400).contains(&session_seconds) {
            return Err(bad());
        }
        if ownership_claim_code.is_some_and(|code| {
            !(32..=256).contains(&code.len()) || code.chars().any(char::is_whitespace)
        }) {
            return Err(bad());
        }
        Ok(Self(Arc::new(Inner {
            store,
            identity,
            notifications: broadcast::channel(256).0,
            challenge_seconds,
            session_seconds,
            ownership_claim_hash: ownership_claim_code.map(hash),
            authentication_budget: Mutex::new((Instant::now(), 0)),
        })))
    }
    fn limit_authentication(&self) -> Result<()> {
        let mut budget = self
            .0
            .authentication_budget
            .lock()
            .map_err(|_| ApiError(StatusCode::SERVICE_UNAVAILABLE, "CORDS_AUTH_UNAVAILABLE"))?;
        if budget.0.elapsed() >= Duration::from_secs(1) {
            *budget = (Instant::now(), 0);
        }
        if budget.1 >= 64 {
            return Err(ApiError(
                StatusCode::TOO_MANY_REQUESTS,
                "CORDS_RATE_LIMITED",
            ));
        }
        budget.1 += 1;
        Ok(())
    }
    async fn authenticate(
        &self,
        headers: &HeaderMap,
        capability: &str,
    ) -> Result<Signed<Membership>> {
        let token = headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or_else(denied)?;
        self.authenticate_token(token, capability).await
    }
    async fn authenticate_token(
        &self,
        token: &str,
        capability: &str,
    ) -> Result<Signed<Membership>> {
        if token.len() > 128 {
            return Err(denied());
        }
        let row = sqlx::query("SELECT m.credential,d.contact,c.challenge FROM sessions s JOIN memberships m ON m.device_id=s.device_id JOIN device_contacts d ON d.device_id=m.device_id JOIN auth_challenges c ON c.id=s.challenge_id WHERE s.token_hash=$1 AND s.active AND m.active AND s.expires_at>$2")
            .bind(hash(token)).bind(int(now()?)?).fetch_optional(self.0.store.pool()).await?.ok_or_else(denied)?;
        let contact: Contact = parse(row.try_get("contact")?)?;
        validate_contact(&contact, now()?)?;
        let challenge: Challenge = parse(row.try_get("challenge")?)?;
        if challenge.authorization_hash != hash(canonical(&contact.authorization)?) {
            return Err(denied());
        }
        let member: Signed<Membership> = parse(row.try_get("credential")?)?;
        if member.value.status != "active"
            || !member.value.capabilities.iter().any(|v| v == capability)
        {
            return Err(denied());
        }
        Ok(member)
    }
    async fn route_access(&self, route: &str, member: &Membership, manage: bool) -> Result<()> {
        let row = sqlx::query("SELECT c.creator FROM channels c JOIN channel_members m ON m.channel_id=c.id WHERE c.id=$1 AND m.device_id=$2 AND m.active AND m.delivery_active")
            .bind(route).bind(&member.device_id).fetch_optional(self.0.store.pool()).await?.ok_or_else(denied)?;
        if manage
            && row.try_get::<String, _>("creator")? != member.device_id
            && !member
                .capabilities
                .iter()
                .any(|value| value == "channel.manage")
        {
            return Err(denied());
        }
        Ok(())
    }
    // Hold account/session authorization until the mutation commits. Account
    // certification and revocation take the matching exclusive advisory lock.
    async fn authorize_mutation(
        &self,
        tx: &mut Tx<'_>,
        headers: &HeaderMap,
        member: &Signed<Membership>,
        route: Option<(&str, bool)>,
    ) -> Result<()> {
        sqlx::query("SELECT pg_advisory_xact_lock_shared(hashtextextended($1,0))")
            .bind(&member.value.account_id)
            .execute(&mut **tx)
            .await?;
        let token = headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or_else(denied)?;
        let row = sqlx::query("SELECT m.credential,d.contact,c.challenge FROM sessions s JOIN memberships m ON m.device_id=s.device_id JOIN device_contacts d ON d.device_id=m.device_id JOIN auth_challenges c ON c.id=s.challenge_id WHERE s.token_hash=$1 AND s.active AND m.active AND s.expires_at>$2 FOR SHARE OF s,m,d")
            .bind(hash(token)).bind(int(now()?)?).fetch_optional(&mut **tx).await?.ok_or_else(denied)?;
        let current: Signed<Membership> = parse(row.try_get("credential")?)?;
        let contact: Contact = parse(row.try_get("contact")?)?;
        let challenge: Challenge = parse(row.try_get("challenge")?)?;
        validate_contact(&contact, now()?)?;
        if canonical(&current)? != canonical(member)?
            || challenge.authorization_hash != hash(canonical(&contact.authorization)?)
        {
            return Err(denied());
        }
        if let Some((route, manage)) = route {
            let creator: String =
                sqlx::query_scalar("SELECT creator FROM channels WHERE id=$1 FOR UPDATE")
                    .bind(route)
                    .fetch_optional(&mut **tx)
                    .await?
                    .ok_or_else(denied)?;
            let permitted: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM channel_members WHERE channel_id=$1 AND device_id=$2 AND active AND delivery_active)")
                .bind(route).bind(&member.value.device_id).fetch_one(&mut **tx).await?;
            if !permitted || (manage && creator != member.value.device_id) {
                return Err(denied());
            }
        }
        Ok(())
    }
    fn sign<T: Statement>(&self, value: T) -> Result<Signed<T>> {
        Ok(cords_identity::sign(&self.0.identity.signing_key, value)?)
    }
}

// A per-scope advisory transaction lock serializes retries including simultaneous first requests.
async fn previous<T: Serialize, R: DeserializeOwned>(
    tx: &mut Tx<'_>,
    scope: &str,
    key: &str,
    request: &T,
) -> Result<Option<R>> {
    if key.is_empty() || key.len() > 128 {
        return Err(bad());
    }
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!("{scope}:{key}"))
        .execute(&mut **tx)
        .await?;
    let row = sqlx::query(
        "SELECT request_hash,result FROM request_results WHERE scope=$1 AND request_key=$2",
    )
    .bind(scope)
    .bind(key)
    .fetch_optional(&mut **tx)
    .await?;
    if let Some(row) = row {
        if row.try_get::<String, _>("request_hash")? != hash(canonical(request)?) {
            return Err(conflict());
        }
        Ok(Some(parse(row.try_get("result")?)?))
    } else {
        Ok(None)
    }
}
async fn remember<T: Serialize, R: Serialize>(
    tx: &mut Tx<'_>,
    scope: &str,
    key: &str,
    request: &T,
    result: &R,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO request_results(scope,request_key,request_hash,result) VALUES($1,$2,$3,$4)",
    )
    .bind(scope)
    .bind(key)
    .bind(hash(canonical(request)?))
    .bind(json(result)?)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub fn router(service: Service) -> Router {
    Router::new()
        .route("/api/v1/join/request", post(join_challenge))
        .route("/api/v1/auth/challenge", post(auth_challenge))
        .route("/api/v1/auth/session", post(session))
        .route("/api/v1/ownership/claim", post(claim_ownership))
        .route("/api/v1/devices/revoke", post(lifecycle::revoke))
        .route("/api/v1/members", get(members))
        .route("/api/v1/key-packages", post(upload_package))
        .route("/api/v1/key-packages/{account}", get(packages))
        .route("/api/v1/channels", get(channels).post(create_channel))
        .route("/api/v1/channels/{route}", get(channel))
        .route("/api/v1/channels/{route}/binding", post(lifecycle::bind))
        .route(
            "/api/v1/channels/{route}/pending-removals",
            get(lifecycle::pending_removals),
        )
        .route("/api/v1/channels/{route}/roster-requests", post(roster))
        .route("/api/v1/channels/{route}/commits", post(commit))
        .route("/api/v1/channels/{route}/welcome", get(welcome))
        .route(
            "/api/v1/routes/{route}/events",
            get(history).post(upload_event),
        )
        .route("/api/v1/events", get(websocket))
        .with_state(service)
        .layer(tower_http::limit::RequestBodyLimitLayer::new(MAX_ENVELOPE))
}

fn member_capabilities(owner: bool) -> Vec<String> {
    let mut capabilities = vec![
        "channel.read".into(),
        "channel.write".into(),
        "channel.create".into(),
        "channel.mls.commit".into(),
        "keypackage.publish".into(),
    ];
    if owner {
        capabilities.extend([
            "server.manage".into(),
            "server.member.ban".into(),
            "server.invite.create".into(),
            "channel.manage".into(),
        ]);
    }
    capabilities
}

async fn join_challenge(
    State(s): State<Service>,
    Json(request): Json<ChallengeRequest>,
) -> Result<Json<Challenge>> {
    if request.purpose != "join" {
        return Err(bad());
    }
    challenge(&s, request).await.map(Json)
}
async fn auth_challenge(
    State(s): State<Service>,
    Json(request): Json<ChallengeRequest>,
) -> Result<Json<Challenge>> {
    if request.purpose != "authenticate" {
        return Err(bad());
    }
    challenge(&s, request).await.map(Json)
}
async fn challenge(s: &Service, request: ChallengeRequest) -> Result<Challenge> {
    s.limit_authentication()?;
    let time = now()?;
    if request.contact.authorization.value.created_at
        > time.saturating_add(cords_identity::STATEMENT_CLOCK_SKEW_SECONDS)
    {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "CORDS_IDENTITY_NOT_YET_VALID",
        ));
    }
    validate_contact(&request.contact, time)?;
    let a = &request.contact.authorization.value;
    let mut tx = s.0.store.pool().begin().await?;
    let scope = format!("challenge:{}", a.device_id);
    if let Some(result) = previous(&mut tx, &scope, &request.idempotency_key, &request).await? {
        return Ok(result);
    }
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM auth_challenges WHERE device_id=$1 AND expires_at>$2",
    )
    .bind(&a.device_id)
    .bind(int(time)?)
    .fetch_one(&mut *tx)
    .await?;
    if count >= 8 {
        return Err(ApiError(
            StatusCode::TOO_MANY_REQUESTS,
            "CORDS_RATE_LIMITED",
        ));
    }
    if request.purpose == "authenticate" {
        let active: Option<bool> =
            sqlx::query_scalar("SELECT active FROM memberships WHERE device_id=$1")
                .bind(&a.device_id)
                .fetch_optional(&mut *tx)
                .await?;
        if active != Some(true) {
            return Err(denied());
        }
    }
    let mut nonce = [0; 32];
    OsRng.fill_bytes(&mut nonce);
    let result = Challenge {
        version: 1,
        challenge_id: id(),
        server_id: s.0.identity.server_id(),
        account_id: a.account_id.clone(),
        device_id: a.device_id.clone(),
        authorization_hash: hash(canonical(&request.contact.authorization)?),
        purpose: request.purpose.clone(),
        nonce: encode(nonce),
        expires_at: time + s.0.challenge_seconds,
    };
    sqlx::query("INSERT INTO auth_challenges(id,device_id,challenge,contact,expires_at) VALUES($1,$2,$3,$4,$5)")
        .bind(&result.challenge_id).bind(&a.device_id).bind(json(&result)?).bind(json(&request.contact)?).bind(int(result.expires_at)?).execute(&mut *tx).await?;
    remember(&mut tx, &scope, &request.idempotency_key, &request, &result).await?;
    tx.commit().await?;
    Ok(result)
}

#[derive(Serialize)]
struct SessionSeed<'a> {
    challenge_id: &'a str,
    expires_at: u64,
}
impl Statement for SessionSeed<'_> {
    const DOMAIN: &'static str = "CORDS-PRIVATE-SESSION-SEED-V1";
}
fn session_token(s: &Service, challenge_id: &str, expires_at: u64) -> Result<String> {
    Ok(hash(
        s.sign(SessionSeed {
            challenge_id,
            expires_at,
        })?
        .signature,
    ))
}

async fn session(
    State(s): State<Service>,
    Json(request): Json<SessionRequest>,
) -> Result<Json<Session>> {
    s.limit_authentication()?;
    let time = now()?;
    let mut tx = s.0.store.pool().begin().await?;
    let row = sqlx::query(
        "SELECT challenge,contact,consumed FROM auth_challenges WHERE id=$1 FOR UPDATE",
    )
    .bind(&request.proof.value.challenge_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(denied)?;
    let expected: Challenge = parse(row.try_get("challenge")?)?;
    let contact: Contact = parse(row.try_get("contact")?)?;
    validate_contact(&contact, time)?;
    request
        .proof
        .verify(&contact.authorization.value.device_public_key)?;
    if canonical(&expected)? != canonical(&request.proof.value)?
        || expected.server_id != s.0.identity.server_id()
    {
        return Err(denied());
    }
    let device = &expected.device_id;
    if row.try_get::<bool, _>("consumed")? {
        return recover_session(&s, &mut tx, &request, &expected, time).await;
    }
    if expected.expires_at <= time {
        return Err(ApiError(
            StatusCode::UNAUTHORIZED,
            "CORDS_AUTH_CHALLENGE_EXPIRED",
        ));
    }
    persist_contact(&mut tx, &contact).await?;
    let old = sqlx::query("SELECT credential,active FROM memberships WHERE device_id=$1")
        .bind(device)
        .fetch_optional(&mut *tx)
        .await?;
    let membership = if let Some(old) = old {
        if !old.try_get::<bool, _>("active")? {
            return Err(denied());
        }
        parse(old.try_get("credential")?)?
    } else {
        if expected.purpose != "join" {
            return Err(denied());
        }
        let owner: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM server_ownership WHERE account_id=$1)")
                .bind(&expected.account_id)
                .fetch_one(&mut *tx)
                .await?;
        let credential = s.sign(Membership {
            version: 1,
            server_id: s.0.identity.server_id(),
            member_id: id(),
            account_id: expected.account_id.clone(),
            device_id: device.clone(),
            generation: 1,
            issued_at: time,
            capabilities: member_capabilities(owner),
            status: "active".into(),
        })?;
        sqlx::query("INSERT INTO memberships(device_id,credential) VALUES($1,$2)")
            .bind(device)
            .bind(json(&credential)?)
            .execute(&mut *tx)
            .await?;
        credential
    };
    let expires_at = time + s.0.session_seconds;
    let token = session_token(&s, &expected.challenge_id, expires_at)?;
    sqlx::query("INSERT INTO sessions(token_hash,device_id,expires_at,challenge_id,request_hash,idempotency_key) VALUES($1,$2,$3,$4,$5,$6)").bind(hash(&token)).bind(device).bind(int(expires_at)?).bind(&expected.challenge_id).bind(hash(canonical(&request)?)).bind(&request.idempotency_key).execute(&mut *tx).await?;
    sqlx::query("UPDATE auth_challenges SET consumed=TRUE WHERE id=$1")
        .bind(&expected.challenge_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(Session {
        token,
        expires_at,
        membership,
    }))
}

async fn claim_ownership(
    State(s): State<Service>,
    headers: HeaderMap,
    Json(request): Json<OwnershipClaim>,
) -> Result<Json<Signed<Membership>>> {
    if !(32..=256).contains(&request.claim_code.len())
        || request.claim_code.chars().any(char::is_whitespace)
        || request.idempotency_key.is_empty()
        || request.idempotency_key.len() > 128
    {
        return Err(bad());
    }
    let member = s.authenticate(&headers, "channel.read").await?;
    let mut tx = s.0.store.pool().begin().await?;
    s.authorize_mutation(&mut tx, &headers, &member, None)
        .await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended('server-ownership',0))")
        .execute(&mut *tx)
        .await?;
    let owner: Option<String> = sqlx::query_scalar(
        "SELECT account_id FROM server_ownership WHERE singleton=TRUE FOR UPDATE",
    )
    .fetch_optional(&mut *tx)
    .await?;
    if let Some(owner) = owner {
        if owner != member.value.account_id {
            return Err(conflict());
        }
        tx.commit().await?;
        return Ok(Json(member));
    }
    let expected = s.0.ownership_claim_hash.as_ref().ok_or_else(denied)?;
    let presented = hash(request.claim_code.as_bytes());
    if !bool::from(expected.as_bytes().ct_eq(presented.as_bytes())) {
        return Err(denied());
    }
    let time = now()?;
    sqlx::query("INSERT INTO server_ownership(singleton,account_id,claimed_by_device_id,claimed_at) VALUES(TRUE,$1,$2,$3)")
        .bind(&member.value.account_id)
        .bind(&member.value.device_id)
        .bind(int(time)?)
        .execute(&mut *tx)
        .await?;
    let rows = sqlx::query("SELECT m.device_id,m.credential FROM memberships m JOIN device_contacts d ON d.device_id=m.device_id WHERE d.account_id=$1 AND m.active FOR UPDATE OF m")
        .bind(&member.value.account_id)
        .fetch_all(&mut *tx)
        .await?;
    let mut claimed = None;
    for row in rows {
        let device_id: String = row.try_get("device_id")?;
        let mut credential: Signed<Membership> = parse(row.try_get("credential")?)?;
        credential.value.generation = credential.value.generation.saturating_add(1);
        credential.value.issued_at = time;
        credential.value.capabilities = member_capabilities(true);
        let credential = s.sign(credential.value)?;
        sqlx::query("UPDATE memberships SET credential=$1 WHERE device_id=$2")
            .bind(json(&credential)?)
            .bind(&device_id)
            .execute(&mut *tx)
            .await?;
        if device_id == member.value.device_id {
            claimed = Some(credential);
        }
    }
    let claimed = claimed.ok_or_else(denied)?;
    tx.commit().await?;
    Ok(Json(claimed))
}

async fn persist_contact(tx: &mut Tx<'_>, contact: &Contact) -> Result<()> {
    let authorization = &contact.authorization.value;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(&authorization.account_id)
        .execute(&mut **tx)
        .await?;
    let head: Option<String> =
        sqlx::query_scalar("SELECT authorization_record FROM account_heads WHERE account_id=$1")
            .bind(&authorization.account_id)
            .fetch_optional(&mut **tx)
            .await?;
    if let Some(head) = head {
        lifecycle::validate_current_head(tx, &parse(&head)?, &contact.authorization).await?;
    } else if authorization.generation != 1 || authorization.previous_record_hash.is_some() {
        return Err(denied());
    }
    let revoked: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM device_revocations WHERE device_id=$1)")
            .bind(&authorization.device_id)
            .fetch_one(&mut **tx)
            .await?;
    if revoked {
        return Err(denied());
    }
    sqlx::query("INSERT INTO account_heads(account_id,authorization_record) VALUES($1,$2) ON CONFLICT(account_id) DO UPDATE SET authorization_record=EXCLUDED.authorization_record").bind(&authorization.account_id).bind(json(&contact.authorization)?).execute(&mut **tx).await?;
    // A conflicting global device ID must not inherit another account's membership.
    let affected = sqlx::query("INSERT INTO device_contacts(device_id,account_id,contact) VALUES($1,$2,$3) ON CONFLICT(device_id) DO UPDATE SET contact=EXCLUDED.contact WHERE device_contacts.account_id=EXCLUDED.account_id")
        .bind(&authorization.device_id).bind(&authorization.account_id).bind(json(contact)?).execute(&mut **tx).await?.rows_affected();
    if affected != 1 {
        return Err(denied());
    }
    Ok(())
}

async fn recover_session(
    s: &Service,
    tx: &mut Tx<'_>,
    request: &SessionRequest,
    expected: &Challenge,
    time: u64,
) -> Result<Json<Session>> {
    let old=sqlx::query("SELECT s.expires_at,s.request_hash,s.idempotency_key,m.credential FROM sessions s JOIN memberships m ON m.device_id=s.device_id WHERE s.challenge_id=$1 AND s.active AND m.active").bind(&expected.challenge_id).fetch_optional(&mut **tx).await?.ok_or_else(denied)?;
    let expiry = uint(old.try_get("expires_at")?)?;
    if old.try_get::<String, _>("request_hash")? != hash(canonical(&request)?)
        || old.try_get::<String, _>("idempotency_key")? != request.idempotency_key
        || expiry <= time
    {
        return Err(denied());
    }
    let token = session_token(s, &expected.challenge_id, expiry)?;
    s.authenticate_token(&token, "channel.read").await?;
    Ok(Json(Session {
        token,
        expires_at: expiry,
        membership: parse(old.try_get("credential")?)?,
    }))
}

async fn members(State(s): State<Service>, headers: HeaderMap) -> Result<Json<Vec<Contact>>> {
    s.authenticate(&headers, "channel.read").await?;
    let values:Vec<String>=sqlx::query_scalar("SELECT d.contact FROM device_contacts d JOIN memberships m ON m.device_id=d.device_id WHERE m.active ORDER BY d.device_id LIMIT 1000").fetch_all(s.0.store.pool()).await?;
    Ok(Json(
        values.iter().map(|v| parse(v)).collect::<Result<_>>()?,
    ))
}
async fn upload_package(
    State(s): State<Service>,
    headers: HeaderMap,
    Json(request): Json<KeyPackageUpload>,
) -> Result<Json<String>> {
    let member = s.authenticate(&headers, "keypackage.publish").await?;
    let device = &member.value.device_id;
    let stored: String =
        sqlx::query_scalar("SELECT contact FROM device_contacts WHERE device_id=$1")
            .bind(device)
            .fetch_one(s.0.store.pool())
            .await?;
    let contact: Contact = parse(&stored)?;
    let bytes = decode(&request.key_package)?;
    cords_crypto::conversation::ConversationCrypto::validate_key_package(
        &bytes,
        device,
        &decode(&contact.mls_binding.value.mls_public_key)?,
    )?;
    let mut tx = s.0.store.pool().begin().await?;
    let scope = format!("keypackage:{device}");
    s.authorize_mutation(&mut tx, &headers, &member, None)
        .await?;
    if let Some(result) = previous(&mut tx, &scope, &request.idempotency_key, &request).await? {
        return Ok(Json(result));
    }
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM key_packages WHERE device_id=$1 AND NOT consumed")
            .bind(device)
            .fetch_one(&mut *tx)
            .await?;
    if count >= 32 {
        return Err(conflict());
    }
    let package_id = hash(bytes);
    sqlx::query("INSERT INTO key_packages(id,device_id,package) VALUES($1,$2,$3)")
        .bind(&package_id)
        .bind(device)
        .bind(&request.key_package)
        .execute(&mut *tx)
        .await?;
    remember(
        &mut tx,
        &scope,
        &request.idempotency_key,
        &request,
        &package_id,
    )
    .await?;
    tx.commit().await?;
    Ok(Json(package_id))
}
async fn packages(
    State(s): State<Service>,
    headers: HeaderMap,
    Path(account): Path<String>,
) -> Result<Json<Vec<String>>> {
    s.authenticate(&headers, "channel.read").await?;
    Ok(Json(sqlx::query_scalar("SELECT k.package FROM key_packages k JOIN device_contacts d ON d.device_id=k.device_id JOIN memberships m ON m.device_id=d.device_id WHERE d.account_id=$1 AND m.active AND NOT k.consumed AND k.reserved_by IS NULL LIMIT 32").bind(account).fetch_all(s.0.store.pool()).await?))
}

async fn create_channel(
    State(s): State<Service>,
    headers: HeaderMap,
    Json(request): Json<ChannelCreate>,
) -> Result<Json<String>> {
    let member = s.authenticate(&headers, "channel.create").await?;
    if request.name.is_empty() || request.name.chars().count() > 100 {
        return Err(bad());
    }
    let mut tx = s.0.store.pool().begin().await?;
    let scope = format!("channel:{}", member.value.device_id);
    s.authorize_mutation(&mut tx, &headers, &member, None)
        .await?;
    if let Some(result) = previous(&mut tx, &scope, &request.idempotency_key, &request).await? {
        return Ok(Json(result));
    }
    let route = id();
    sqlx::query("INSERT INTO channels(id,name,creator) VALUES($1,$2,$3)")
        .bind(&route)
        .bind(&request.name)
        .bind(&member.value.device_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO channel_members(channel_id,device_id) VALUES($1,$2)")
        .bind(&route)
        .bind(&member.value.device_id)
        .execute(&mut *tx)
        .await?;
    remember(&mut tx, &scope, &request.idempotency_key, &request, &route).await?;
    tx.commit().await?;
    Ok(Json(route))
}
async fn channel_view(s: &Service, route: &str) -> Result<Channel> {
    let mut tx = s.0.store.pool().begin().await?;
    let view = channel_view_in(&mut tx, route).await?;
    tx.commit().await?;
    Ok(view)
}
async fn channel_view_in(tx: &mut Tx<'_>, route: &str) -> Result<Channel> {
    let row = sqlx::query("SELECT name,creator,epoch,binding FROM channels WHERE id=$1 FOR SHARE")
        .bind(route)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(denied)?;
    let contacts:Vec<String>=sqlx::query_scalar("SELECT d.contact FROM channel_members m JOIN device_contacts d ON d.device_id=m.device_id WHERE m.channel_id=$1 AND m.active ORDER BY d.device_id").bind(route).fetch_all(&mut **tx).await?;
    Ok(Channel {
        channel_id: route.into(),
        name: row.try_get("name")?,
        creator_device_id: row.try_get("creator")?,
        epoch: uint(row.try_get("epoch")?)?,
        members: contacts.iter().map(|v| parse(v)).collect::<Result<_>>()?,
        binding: row
            .try_get::<Option<String>, _>("binding")?
            .map(|v| parse(&v))
            .transpose()?,
    })
}
async fn channels(State(s): State<Service>, headers: HeaderMap) -> Result<Json<Vec<Channel>>> {
    let member = s.authenticate(&headers, "channel.read").await?;
    let ids:Vec<String>=sqlx::query_scalar("SELECT channel_id FROM channel_members WHERE device_id=$1 AND active AND delivery_active ORDER BY channel_id LIMIT 1000").bind(&member.value.device_id).fetch_all(s.0.store.pool()).await?;
    let mut result = Vec::new();
    for route in ids {
        result.push(channel_view(&s, &route).await?);
    }
    Ok(Json(result))
}
#[derive(Deserialize)]
struct ChannelQuery {
    epoch: Option<u64>,
}
async fn channel(
    State(s): State<Service>,
    headers: HeaderMap,
    Path(route): Path<String>,
    Query(query): Query<ChannelQuery>,
) -> Result<Json<Channel>> {
    let member = s.authenticate(&headers, "channel.read").await?;
    s.route_access(&route, &member.value, false).await?;
    if let Some(epoch) = query.epoch {
        let view: Option<String> =
            sqlx::query_scalar("SELECT view FROM channel_epochs WHERE channel_id=$1 AND epoch=$2")
                .bind(&route)
                .bind(int(epoch)?)
                .fetch_optional(s.0.store.pool())
                .await?;
        if let Some(view) = view {
            return Ok(Json(parse(&view)?));
        }
        let current = channel_view(&s, &route).await?;
        if current.epoch != epoch {
            return Err(conflict());
        }
        return Ok(Json(current));
    }
    Ok(Json(channel_view(&s, &route).await?))
}

async fn roster(
    State(s): State<Service>,
    headers: HeaderMap,
    Path(route): Path<String>,
    Json(request): Json<RosterRequest>,
) -> Result<Json<RosterOperation>> {
    let member = s.authenticate(&headers, "channel.mls.commit").await?;
    s.route_access(&route, &member.value, true).await?;
    let mut tx = s.0.store.pool().begin().await?;
    let scope = format!("roster:{route}:{}", member.value.device_id);
    s.authorize_mutation(&mut tx, &headers, &member, Some((&route, true)))
        .await?;
    if let Some(result) = previous(&mut tx, &scope, &request.idempotency_key, &request).await? {
        return Ok(Json(result));
    }
    let epoch: i64 = sqlx::query_scalar("SELECT epoch FROM channels WHERE id=$1 FOR UPDATE")
        .bind(&route)
        .fetch_one(&mut *tx)
        .await?;
    let exists:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM channel_members WHERE channel_id=$1 AND device_id=$2 AND active)").bind(&route).bind(&request.target_device_id).fetch_one(&mut *tx).await?;
    if request.action == RosterAction::Remove {
        if !exists || request.target_device_id == member.value.device_id {
            return Err(denied());
        }
        let result = lifecycle::reserve_removal(
            &mut tx,
            &route,
            &member.value.device_id,
            &request.target_device_id,
            epoch,
        )
        .await?;
        remember(&mut tx, &scope, &request.idempotency_key, &request, &result).await?;
        tx.commit().await?;
        return Ok(Json(result));
    }
    if exists {
        return Err(conflict());
    }
    let contact:String=sqlx::query_scalar("SELECT d.contact FROM device_contacts d JOIN memberships m ON m.device_id=d.device_id WHERE d.device_id=$1 AND m.active").bind(&request.target_device_id).fetch_optional(&mut *tx).await?.ok_or_else(denied)?;
    let target: Contact = parse(&contact)?;
    validate_contact(&target, now()?)?;
    let package=sqlx::query("SELECT id,package FROM key_packages WHERE device_id=$1 AND NOT consumed AND reserved_by IS NULL ORDER BY id LIMIT 1 FOR UPDATE").bind(&request.target_device_id).fetch_optional(&mut *tx).await?.ok_or_else(conflict)?;
    let operation_id = id();
    sqlx::query("UPDATE key_packages SET reserved_by=$1 WHERE id=$2")
        .bind(&operation_id)
        .bind(package.try_get::<String, _>("id")?)
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO roster_operations(id,channel_id,requester,target,base_epoch,package_id) VALUES($1,$2,$3,$4,$5,$6)").bind(&operation_id).bind(&route).bind(&member.value.device_id).bind(&request.target_device_id).bind(epoch).bind(package.try_get::<String,_>("id")?).execute(&mut *tx).await?;
    let result = RosterOperation {
        action: RosterAction::Add,
        operation_id,
        base_epoch: uint(epoch)?,
        target,
        key_package: package.try_get("package")?,
    };
    remember(&mut tx, &scope, &request.idempotency_key, &request, &result).await?;
    tx.commit().await?;
    Ok(Json(result))
}

async fn append(tx: &mut Tx<'_>, envelope: EventUpload) -> Result<RouteEvent> {
    let sequence: i64 = sqlx::query_scalar(
        "UPDATE channels SET next_sequence=next_sequence+1 WHERE id=$1 RETURNING next_sequence-1",
    )
    .bind(&envelope.route_id)
    .fetch_one(&mut **tx)
    .await?;
    let received = now()?;
    sqlx::query("INSERT INTO route_events(route_id,sequence,event_id,envelope,received_at) VALUES($1,$2,$3,$4,$5)").bind(&envelope.route_id).bind(sequence).bind(&envelope.event_id).bind(json(&envelope)?).bind(int(received)?).execute(&mut **tx).await?;
    Ok(RouteEvent {
        envelope,
        server_sequence: uint(sequence)?,
        server_received_at: received,
    })
}
async fn commit(
    State(s): State<Service>,
    headers: HeaderMap,
    Path(route): Path<String>,
    Json(request): Json<CommitUpload>,
) -> Result<Json<RouteEvent>> {
    let member = s.authenticate(&headers, "channel.mls.commit").await?;
    s.route_access(&route, &member.value, true).await?;
    let current = channel_view(&s, &route).await?;
    let creator = current
        .members
        .iter()
        .find(|c| c.authorization.value.device_id == member.value.device_id)
        .ok_or_else(denied)?;
    request
        .binding
        .verify(&creator.authorization.value.device_public_key)?;
    let binding = &request.binding.value;
    if binding.version != 1
        || binding.server_id != s.0.identity.server_id()
        || binding.channel_id != route
        || binding.group_id != route
        || binding.creator_device_id != member.value.device_id
        || decode(&request.commit)?.is_empty()
    {
        return Err(bad());
    }
    let mut tx = s.0.store.pool().begin().await?;
    let scope = format!("commit:{route}:{}", member.value.device_id);
    s.authorize_mutation(&mut tx, &headers, &member, Some((&route, true)))
        .await?;
    if let Some(result) = previous(&mut tx, &scope, &request.idempotency_key, &request).await? {
        return Ok(Json(result));
    }
    let epoch: i64 = sqlx::query_scalar("SELECT epoch FROM channels WHERE id=$1 FOR UPDATE")
        .bind(&route)
        .fetch_one(&mut *tx)
        .await?;
    if uint(epoch)? != request.base_epoch {
        return Err(conflict());
    }
    lifecycle::snapshot_epoch(&mut tx, &route).await?;
    let operation=sqlx::query("SELECT target,package_id,action FROM roster_operations WHERE id=$1 AND channel_id=$2 AND requester=$3 AND base_epoch=$4 AND NOT complete FOR UPDATE").bind(&request.operation_id).bind(&route).bind(&member.value.device_id).bind(epoch).fetch_optional(&mut *tx).await?.ok_or_else(conflict)?;
    let target: String = operation.try_get("target")?;
    let active: bool =
        sqlx::query_scalar("SELECT active FROM memberships WHERE device_id=$1 FOR SHARE")
            .bind(&target)
            .fetch_one(&mut *tx)
            .await?;
    let removing = operation.try_get::<String, _>("action")? == "remove";
    if !removing && (!active || decode(&request.welcome)?.is_empty()) {
        return Err(denied());
    }
    if removing && !request.welcome.is_empty() {
        return Err(bad());
    }
    let event = append(
        &mut tx,
        EventUpload {
            protocol_version: 1,
            event_id: id(),
            route_kind: "channel".into(),
            route_id: route.clone(),
            sender_member_id: member.value.member_id,
            sender_device_id: member.value.device_id,
            client_created_at: now()?,
            content_encoding: "mls.commit".into(),
            ciphertext: request.commit.clone(),
            idempotency_key: request.idempotency_key.clone(),
        },
    )
    .await?;
    sqlx::query("UPDATE channels SET epoch=epoch+1,binding=$1 WHERE id=$2")
        .bind(json(&request.binding)?)
        .bind(&route)
        .execute(&mut *tx)
        .await?;
    if removing {
        lifecycle::apply_remove(&mut tx, &route, &target).await?;
    } else {
        lifecycle::apply_add(
            &mut tx,
            &route,
            &target,
            event.server_sequence,
            &request.welcome,
            operation.try_get("package_id")?,
        )
        .await?;
    }
    sqlx::query("UPDATE roster_operations SET complete=TRUE WHERE id=$1")
        .bind(&request.operation_id)
        .execute(&mut *tx)
        .await?;
    lifecycle::snapshot_epoch(&mut tx, &route).await?;
    remember(&mut tx, &scope, &request.idempotency_key, &request, &event).await?;
    tx.commit().await?;
    let _ = s.0.notifications.send((route, event.server_sequence));
    Ok(Json(event))
}
async fn welcome(
    State(s): State<Service>,
    headers: HeaderMap,
    Path(route): Path<String>,
) -> Result<Json<serde_json::Value>> {
    let member = s.authenticate(&headers, "channel.read").await?;
    s.route_access(&route, &member.value, false).await?;
    let row =
        sqlx::query("SELECT sequence,welcome FROM welcomes WHERE channel_id=$1 AND device_id=$2")
            .bind(route)
            .bind(&member.value.device_id)
            .fetch_optional(s.0.store.pool())
            .await?
            .ok_or_else(denied)?;
    Ok(Json(
        serde_json::json!({"sequence":row.try_get::<i64,_>("sequence")?,"welcome":row.try_get::<String,_>("welcome")?}),
    ))
}
async fn upload_event(
    State(s): State<Service>,
    headers: HeaderMap,
    Path(route): Path<String>,
    Json(request): Json<EventUpload>,
) -> Result<Json<RouteEvent>> {
    let member = s.authenticate(&headers, "channel.write").await?;
    s.route_access(&route, &member.value, false).await?;
    if request.protocol_version != 1
        || request.route_kind != "channel"
        || request.route_id != route
        || request.sender_device_id != member.value.device_id
        || request.sender_member_id != member.value.member_id
        || request.content_encoding != "mls.application"
        || uuid::Uuid::parse_str(&request.event_id).is_err()
        || decode(&request.ciphertext)?.is_empty()
    {
        return Err(bad());
    }
    let mut tx = s.0.store.pool().begin().await?;
    let scope = format!("event:{route}:{}", member.value.device_id);
    s.authorize_mutation(&mut tx, &headers, &member, Some((&route, false)))
        .await?;
    if let Some(result) = previous(&mut tx, &scope, &request.idempotency_key, &request).await? {
        return Ok(Json(result));
    }
    let event = append(&mut tx, request.clone()).await?;
    remember(&mut tx, &scope, &request.idempotency_key, &request, &event).await?;
    tx.commit().await?;
    let _ = s.0.notifications.send((route, event.server_sequence));
    Ok(Json(event))
}
#[derive(Deserialize)]
struct HistoryQuery {
    #[serde(default)]
    after: u64,
}
async fn history(
    State(s): State<Service>,
    headers: HeaderMap,
    Path(route): Path<String>,
    Query(query): Query<HistoryQuery>,
) -> Result<Json<Vec<RouteEvent>>> {
    let member = s.authenticate(&headers, "channel.read").await?;
    s.route_access(&route, &member.value, false).await?;
    let rows=sqlx::query("SELECT e.sequence,e.envelope,e.received_at FROM route_events e JOIN channel_members m ON m.channel_id=e.route_id WHERE e.route_id=$1 AND m.device_id=$2 AND m.active AND m.delivery_active AND e.sequence>$3 AND e.sequence>m.joined_sequence ORDER BY e.sequence LIMIT 8")
        .bind(route).bind(&member.value.device_id).bind(int(query.after)?).fetch_all(s.0.store.pool()).await?;
    Ok(Json(
        rows.iter()
            .map(|row| {
                Ok(RouteEvent {
                    envelope: parse(row.try_get("envelope")?)?,
                    server_sequence: uint(row.try_get("sequence")?)?,
                    server_received_at: uint(row.try_get("received_at")?)?,
                })
            })
            .collect::<Result<_>>()?,
    ))
}

async fn websocket(
    State(s): State<Service>,
    headers: HeaderMap,
    upgrade: WebSocketUpgrade,
) -> Result<Response> {
    s.authenticate(&headers, "channel.read").await?;
    if !headers
        .get("sec-websocket-protocol")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.split(',').any(|v| v.trim() == WS_PROTOCOL))
    {
        return Err(bad());
    }
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(denied)?
        .to_owned();
    let receiver = s.0.notifications.subscribe();
    Ok(upgrade
        .protocols([WS_PROTOCOL])
        .max_message_size(4096)
        .max_frame_size(4096)
        .on_upgrade(move |socket| socket_loop(socket, s, token, receiver))
        .into_response())
}
async fn socket_loop(
    mut socket: WebSocket,
    s: Service,
    token: String,
    mut receiver: broadcast::Receiver<(String, u64)>,
) {
    let mut interval = tokio::time::interval(Duration::from_secs(15));
    let mut last_pong = Instant::now();
    loop {
        tokio::select! {
            notice=receiver.recv()=>{
                let Ok((route,sequence))=notice else {break;};
                let Ok(member)=s.authenticate_token(&token,"channel.read").await else {break;};
                if s.route_access(&route,&member.value,false).await.is_err() {continue;}
                let Ok(bytes)=notification(&route,sequence) else {break;};
                if socket.send(WsMessage::Binary(bytes.into())).await.is_err(){break;}
            }
            incoming=socket.recv()=>{match incoming {Some(Ok(WsMessage::Pong(_)))=>{last_pong = Instant::now();},Some(Ok(WsMessage::Ping(_)))=>{},_=>break}}
            _=interval.tick()=>{
                if last_pong.elapsed() > Duration::from_secs(45) || s.authenticate_token(&token,"channel.read").await.is_err() || socket.send(WsMessage::Ping(Vec::new().into())).await.is_err(){break;}
            }
        }
    }
    let _ = socket.send(WsMessage::Close(None)).await;
}

mod lifecycle;
#[cfg(test)]
mod tests;
