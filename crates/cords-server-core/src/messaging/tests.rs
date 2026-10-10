//! Real `PostgreSQL` rejection and durability tests. No substitute in-memory database.
use super::*;
use anyhow::{Context as _, Result};
use axum::{
    body::{Body, to_bytes},
    http::Request,
};
use cords_crypto::conversation::ConversationCrypto;
use cords_identity::Identity;
use cords_protocol::messaging::OwnershipProof;
use std::str::FromStr as _;
use tower::ServiceExt as _;

async fn fixture() -> Result<(Service, tempfile::TempDir)> {
    let (service, directory, _) = fixture_with_state(true).await?;
    Ok((service, directory))
}

async fn fixture_with_state(
    claimed: bool,
) -> Result<(Service, tempfile::TempDir, zeroize::Zeroizing<String>)> {
    let url = std::env::var("CORDS_TEST_DATABASE_URL")
        .context("set CORDS_TEST_DATABASE_URL to an isolated PostgreSQL service")?;
    let admin = sqlx::PgPool::connect(&url).await?;
    let schema = format!("cords_test_{}", uuid::Uuid::now_v7().simple());
    sqlx::query(&format!("CREATE SCHEMA {schema}"))
        .execute(&admin)
        .await?;
    let options = sqlx::postgres::PgConnectOptions::from_str(&url)?
        .options([("search_path", schema.as_str())]);
    let store = PostgresStore::from_pool(
        sqlx::postgres::PgPoolOptions::new()
            .connect_with(options)
            .await?,
    );
    let migrations =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../migrations/postgres");
    assert!(store.require_current().await.is_err());
    store.migrate(&migrations).await?;
    store.migrate(&migrations).await?;
    store.require_current().await?;
    let directory = tempfile::tempdir()?;
    let (identity, _) =
        crate::ServerIdentity::load_or_create(&directory.path().join("server.key"))?;
    store.bind_server_identity(&identity.server_id()).await?;
    store.bind_server_identity(&identity.server_id()).await?;
    assert!(store.bind_server_identity("replacement").await.is_err());
    let (loaded, created) =
        crate::ServerIdentity::load_or_create(&directory.path().join("server.key"))?;
    assert!(!created);
    assert_eq!(loaded.server_id(), identity.server_id());
    let crate::ownership::BootstrapOutcome::Generated(code) =
        crate::ownership::ensure(&store, &identity.server_id()).await?
    else {
        anyhow::bail!("fresh fixture did not generate an ownership code")
    };
    if claimed {
        sqlx::query("UPDATE server_ownership_bootstrap SET state='CLAIMED',claim_code_hash=NULL,claimed_at=1 WHERE singleton=TRUE")
            .execute(store.pool()).await?;
    }
    Ok((Service::new(store, identity, 60, 900)?, directory, code))
}
fn identity() -> Result<(Identity, Contact)> {
    let identity = Identity::generate(now()?)?;
    let crypto = ConversationCrypto::new(identity.authorization.value.device_id.clone())?;
    let contact = identity.contact(&crypto.public_key())?;
    Ok((identity, contact))
}
async fn join(s: &Service, identity: &Identity, contact: Contact) -> Result<Session> {
    let challenge = challenge(
        s,
        ChallengeRequest {
            contact,
            purpose: "join".into(),
            idempotency_key: id(),
        },
    )
    .await?;
    let request = SessionRequest {
        proof: identity.sign_device(challenge)?,
        idempotency_key: id(),
    };
    Ok(session(State(s.clone()), Json(request)).await?.0)
}

async fn ownership_request(
    s: &Service,
    identity: &Identity,
    contact: Contact,
    code: &str,
) -> Result<OwnershipClaim> {
    let challenge = ownership_challenge(
        State(s.clone()),
        Json(ChallengeRequest {
            contact: contact.clone(),
            purpose: "claim_ownership".into(),
            idempotency_key: id(),
        }),
    )
    .await?
    .0;
    Ok(OwnershipClaim {
        claim_code: code.into(),
        contact,
        device_proof: identity.sign_device(challenge.clone())?,
        root_proof: identity.sign_root(OwnershipProof {
            version: 1,
            server_id: challenge.server_id.clone(),
            account_id: challenge.account_id.clone(),
            device_id: challenge.device_id.clone(),
            challenge_hash: hash(canonical(&challenge)?),
        })?,
        idempotency_key: id(),
    })
}

#[tokio::test]
#[ignore = "requires CORDS_TEST_DATABASE_URL; run explicitly against real PostgreSQL"]
async fn ownership_bootstrap_is_pre_membership_atomic_and_one_time() -> Result<()> {
    let (s, _directory, code) = fixture_with_state(false).await?;
    let signed = ownership_state(State(s.clone())).await?.0;
    signed.verify(&encode(s.0.identity.signing_key.verifying_key().to_bytes()))?;
    assert_eq!(signed.value.state, "UNCLAIMED");

    let (owner, contact) = identity()?;
    assert!(join(&s, &owner, contact.clone()).await.is_err());
    let request = ownership_request(&s, &owner, contact, &code).await?;
    let mut wrong_code = request.clone();
    wrong_code.claim_code = "A".repeat(43);
    assert_eq!(
        claim_ownership(State(s.clone()), Json(wrong_code))
            .await
            .err()
            .context("wrong bootstrap code was accepted")?
            .0,
        StatusCode::FORBIDDEN
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM memberships")
        .fetch_one(s.0.store.pool())
        .await?;
    assert_eq!(count, 0);

    let (attacker, _) = identity()?;
    let mut wrong_root = request.clone();
    wrong_root.root_proof = attacker.sign_root(wrong_root.root_proof.value.clone())?;
    assert!(
        claim_ownership(State(s.clone()), Json(wrong_root))
            .await
            .is_err()
    );
    let claimed = claim_ownership(State(s.clone()), Json(request.clone()))
        .await?
        .0;
    claimed
        .membership
        .verify(&encode(s.0.identity.signing_key.verifying_key().to_bytes()))?;
    assert!(
        claimed
            .membership
            .value
            .capabilities
            .iter()
            .any(|capability| capability == "server.manage")
    );
    assert!(
        claim_ownership(State(s.clone()), Json(request))
            .await
            .is_err()
    );
    let stored: (String, Option<String>) = sqlx::query_as(
        "SELECT state,claim_code_hash FROM server_ownership_bootstrap WHERE singleton=TRUE",
    )
    .fetch_one(s.0.store.pool())
    .await?;
    assert_eq!(stored, ("CLAIMED".into(), None));

    let reloaded = Service::new(s.0.store.clone(), s.0.identity.clone(), 60, 900)?;
    assert_eq!(
        ownership_state(State(reloaded)).await?.0.value.state,
        "CLAIMED"
    );
    sqlx::query("UPDATE memberships SET active=FALSE WHERE device_id=$1")
        .bind(&owner.authorization.value.device_id)
        .execute(s.0.store.pool())
        .await?;
    assert!(matches!(
        crate::ownership::rotate(&s.0.store, &s.0.identity.server_id()).await,
        Err(crate::ownership::BootstrapError::AlreadyClaimed)
    ));
    assert_eq!(
        ownership_state(State(s.clone())).await?.0.value.state,
        "CLAIMED"
    );
    let (member, member_contact) = identity()?;
    let member = join(&s, &member, member_contact).await?;
    assert!(
        !member
            .membership
            .value
            .capabilities
            .iter()
            .any(|capability| { capability == "server.manage" || capability == "channel.manage" })
    );
    Ok(())
}

#[tokio::test]
#[ignore = "requires CORDS_TEST_DATABASE_URL; run explicitly against real PostgreSQL"]
async fn simultaneous_valid_ownership_claims_have_one_winner() -> Result<()> {
    let (s, _directory, code) = fixture_with_state(false).await?;
    let (a, contact_a) = identity()?;
    let (b, contact_b) = identity()?;
    let request_a = ownership_request(&s, &a, contact_a, &code).await?;
    let request_b = ownership_request(&s, &b, contact_b, &code).await?;
    let (a, b) = tokio::join!(
        claim_ownership(State(s.clone()), Json(request_a)),
        claim_ownership(State(s.clone()), Json(request_b))
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    let owners: i64 = sqlx::query_scalar("SELECT count(*) FROM server_ownership")
        .fetch_one(s.0.store.pool())
        .await?;
    let memberships: i64 = sqlx::query_scalar("SELECT count(*) FROM memberships")
        .fetch_one(s.0.store.pool())
        .await?;
    assert_eq!((owners, memberships), (1, 1));
    Ok(())
}

#[tokio::test]
#[ignore = "requires CORDS_TEST_DATABASE_URL; run explicitly against real PostgreSQL"]
async fn bootstrap_restart_rotation_and_legacy_initialization_are_explicit() -> Result<()> {
    let (s, _directory, original) = fixture_with_state(false).await?;
    assert_eq!(original.len(), 43);
    let before: (String, i64) = sqlx::query_as(
        "SELECT claim_code_hash,generation FROM server_ownership_bootstrap WHERE singleton=TRUE",
    )
    .fetch_one(s.0.store.pool())
    .await?;
    assert!(matches!(
        crate::ownership::ensure(&s.0.store, &s.0.identity.server_id()).await?,
        crate::ownership::BootstrapOutcome::Existing
    ));
    let unchanged: (String, i64) = sqlx::query_as(
        "SELECT claim_code_hash,generation FROM server_ownership_bootstrap WHERE singleton=TRUE",
    )
    .fetch_one(s.0.store.pool())
    .await?;
    assert_eq!(before, unchanged);
    let rotated = crate::ownership::rotate(&s.0.store, &s.0.identity.server_id()).await?;
    let after: (String, i64) = sqlx::query_as(
        "SELECT claim_code_hash,generation FROM server_ownership_bootstrap WHERE singleton=TRUE",
    )
    .fetch_one(s.0.store.pool())
    .await?;
    assert_ne!(original.as_str(), rotated.as_str());
    assert_ne!(before.0, after.0);
    assert_eq!(after.1, 2);

    sqlx::query("UPDATE server_ownership_bootstrap SET state='CLAIMED',claim_code_hash=NULL,claimed_at=1 WHERE singleton=TRUE")
        .execute(s.0.store.pool()).await?;
    let (member, contact) = identity()?;
    join(&s, &member, contact).await?;
    sqlx::query("DELETE FROM server_ownership_bootstrap")
        .execute(s.0.store.pool())
        .await?;
    assert!(matches!(
        crate::ownership::ensure(&s.0.store, &s.0.identity.server_id()).await,
        Err(crate::ownership::BootstrapError::LegacyInitializationRequired)
    ));
    assert!(matches!(
        crate::ownership::initialize_legacy(&s.0.store, &s.0.identity.server_id()).await?,
        crate::ownership::BootstrapOutcome::Generated(_)
    ));
    let owners: i64 = sqlx::query_scalar("SELECT count(*) FROM server_ownership")
        .fetch_one(s.0.store.pool())
        .await?;
    assert_eq!(owners, 0);
    Ok(())
}

#[tokio::test]
#[ignore = "requires CORDS_TEST_DATABASE_URL; run explicitly against real PostgreSQL"]
async fn websocket_requires_normal_session_protocol_and_enforces_limits() -> Result<()> {
    use futures_util::{SinkExt as _, StreamExt as _};
    use tokio_tungstenite::{connect_async, tungstenite};
    use tungstenite::{Message, client::IntoClientRequest as _};

    let (s, _directory) = fixture().await?;
    let (identity, contact) = identity()?;
    let session = join(&s, &identity, contact).await?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let url = format!("ws://{}/api/v1/events", listener.local_addr()?);
    let app = router(s.clone());
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let server = tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = stopped.await;
            })
            .await
    });
    // Dropping stop on any early error also shuts down the temporary listener.
    let request = |token: Option<&str>, protocol: Option<&str>| -> Result<_> {
        let mut request = url.as_str().into_client_request()?;
        if let Some(token) = token {
            request
                .headers_mut()
                .insert("authorization", format!("Bearer {token}").parse()?);
        }
        if let Some(protocol) = protocol {
            request
                .headers_mut()
                .insert("sec-websocket-protocol", protocol.parse()?);
        }
        Ok(request)
    };
    for (token, protocol, expected) in [
        (None, Some(WS_PROTOCOL), StatusCode::FORBIDDEN),
        (Some("invalid"), Some(WS_PROTOCOL), StatusCode::FORBIDDEN),
        (Some(session.token.as_str()), None, StatusCode::BAD_REQUEST),
        (
            Some(session.token.as_str()),
            Some("cords.invalid"),
            StatusCode::BAD_REQUEST,
        ),
    ] {
        let result = connect_async(request(token, protocol)?).await;
        let Err(tungstenite::Error::Http(response)) = result else {
            anyhow::bail!("invalid WebSocket handshake was not rejected by HTTP");
        };
        assert_eq!(response.status(), expected);
    }
    let (mut oversized, response) =
        connect_async(request(Some(&session.token), Some(WS_PROTOCOL))?).await?;
    assert_eq!(response.headers()["sec-websocket-protocol"], WS_PROTOCOL);
    oversized
        .send(Message::Binary(vec![0; 4097].into()))
        .await?;
    let closed = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match oversized.next().await {
                Some(Ok(Message::Ping(_))) => oversized.flush().await?,
                Some(Ok(Message::Close(_)) | Err(_)) | None => break,
                _ => anyhow::bail!("oversized WebSocket remained usable"),
            }
        }
        Ok::<_, anyhow::Error>(())
    })
    .await;
    closed.context("oversized socket did not close")??;
    let (mut socket, _) = connect_async(request(Some(&session.token), Some(WS_PROTOCOL))?).await?;
    sqlx::query("UPDATE sessions SET active=false")
        .execute(s.0.store.pool())
        .await?;
    // A queued notification must recheck normal session authorization before delivery.
    s.0.notifications.send((id(), 1))?;
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match socket.next().await {
                Some(Ok(Message::Ping(_))) => socket.flush().await?,
                Some(Ok(Message::Close(_))) | None => break,
                Some(Err(error)) => return Err(error.into()),
                _ => anyhow::bail!("revoked session received an application notification"),
            }
        }
        Ok::<_, anyhow::Error>(())
    })
    .await
    .context("revoked socket did not close")??;
    assert!(
        matches!(connect_async(request(Some(&session.token), Some(WS_PROTOCOL))?).await,
        Err(tungstenite::Error::Http(response)) if response.status() == StatusCode::FORBIDDEN)
    );
    drop(socket);
    drop(oversized);
    drop(stop);
    server.await??;
    Ok(())
}
fn headers(session: &Session) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        format!("Bearer {}", session.token).parse()?,
    );
    Ok(headers)
}

#[tokio::test]
#[ignore = "requires CORDS_TEST_DATABASE_URL; run explicitly against real PostgreSQL"]
async fn device_id_collision_cannot_inherit_membership() -> Result<()> {
    let (s, _directory) = fixture().await?;
    let (victim, contact) = identity()?;
    let original = join(&s, &victim, contact.clone()).await?;
    let (attacker, _) = identity()?;
    let exported = attacker.export_secret()?;
    let mut stored: serde_json::Value = serde_json::from_slice(&exported)?;
    let root: [u8; 32] = serde_json::from_value(stored["root"].clone())?;
    let mut authorization = attacker.authorization.value.clone();
    authorization.device_id = victim.authorization.value.device_id.clone();
    stored["authorization"] = serde_json::to_value(cords_identity::sign(
        &ed25519_dalek::SigningKey::from_bytes(&root),
        authorization,
    )?)?;
    let attacker = Identity::import_secret(&serde_json::to_vec(&stored)?, now()?)?;
    let crypto = ConversationCrypto::new(attacker.authorization.value.device_id.clone())?;
    let collision = attacker.contact(&crypto.public_key())?;
    assert!(join(&s, &attacker, collision).await.is_err());
    let current = s
        .authenticate_token(&original.token, "channel.read")
        .await?;
    assert_eq!(
        current.value.account_id,
        victim.authorization.value.account_id
    );
    let saved: String =
        sqlx::query_scalar("SELECT contact FROM device_contacts WHERE device_id=$1")
            .bind(&victim.authorization.value.device_id)
            .fetch_one(s.0.store.pool())
            .await?;
    assert_eq!(saved, json(&contact)?);
    let unauthorized: i64 =
        sqlx::query_scalar("SELECT count(*) FROM account_heads WHERE account_id=$1")
            .bind(&attacker.authorization.value.account_id)
            .fetch_one(s.0.store.pool())
            .await?;
    assert_eq!(unauthorized, 0);
    Ok(())
}

#[tokio::test]
#[ignore = "requires CORDS_TEST_DATABASE_URL; run explicitly against real PostgreSQL"]
async fn authentication_rejections_and_idempotent_session_recovery() -> Result<()> {
    let (s, _directory) = fixture().await?;
    let (a, contact) = identity()?;
    let (b, _) = identity()?;
    assert!(
        challenge(
            &s,
            ChallengeRequest {
                contact: contact.clone(),
                purpose: "authenticate".into(),
                idempotency_key: id()
            }
        )
        .await
        .is_err()
    );
    let challenge = challenge(
        &s,
        ChallengeRequest {
            contact: contact.clone(),
            purpose: "join".into(),
            idempotency_key: id(),
        },
    )
    .await?;
    let bad = SessionRequest {
        proof: b.sign_device(challenge.clone())?,
        idempotency_key: id(),
    };
    assert!(session(State(s.clone()), Json(bad)).await.is_err());
    let mut altered = challenge.clone();
    altered.nonce.push('a');
    assert!(
        session(
            State(s.clone()),
            Json(SessionRequest {
                proof: a.sign_device(altered)?,
                idempotency_key: id()
            })
        )
        .await
        .is_err()
    );
    let request = SessionRequest {
        proof: a.sign_device(challenge.clone())?,
        idempotency_key: id(),
    };
    let granted = session(State(s.clone()), Json(request.clone())).await?.0;
    let repeated = session(State(s.clone()), Json(request.clone())).await?.0;
    assert_eq!(granted.token, repeated.token);
    let mut replay = request;
    replay.idempotency_key = id();
    assert!(session(State(s.clone()), Json(replay)).await.is_err());
    let mut invalid = contact.clone();
    invalid.authorization.signature = encode([0; 64]);
    assert!(
        super::challenge(
            &s,
            ChallengeRequest {
                contact: invalid,
                purpose: "join".into(),
                idempotency_key: id()
            }
        )
        .await
        .is_err()
    );
    reject_expired_challenge(&s, &a, contact).await?;
    sqlx::query("UPDATE memberships SET active=FALSE WHERE device_id=$1")
        .bind(&a.authorization.value.device_id)
        .execute(s.0.store.pool())
        .await?;
    assert!(
        s.authenticate_token(&granted.token, "channel.read")
            .await
            .is_err()
    );
    Ok(())
}

async fn reject_expired_challenge(s: &Service, a: &Identity, contact: Contact) -> Result<()> {
    let mut expired = super::challenge(
        s,
        ChallengeRequest {
            contact,
            purpose: "authenticate".into(),
            idempotency_key: id(),
        },
    )
    .await?;
    expired.expires_at = now()?.saturating_sub(1);
    sqlx::query("UPDATE auth_challenges SET challenge=$1,expires_at=$2 WHERE id=$3")
        .bind(json(&expired)?)
        .bind(int(expired.expires_at)?)
        .bind(&expired.challenge_id)
        .execute(s.0.store.pool())
        .await?;
    assert_eq!(
        session(
            State(s.clone()),
            Json(SessionRequest {
                proof: a.sign_device(expired)?,
                idempotency_key: id()
            })
        )
        .await
        .err()
        .context("expired challenge accepted")?
        .0,
        StatusCode::UNAUTHORIZED
    );
    Ok(())
}

#[tokio::test]
#[ignore = "requires CORDS_TEST_DATABASE_URL; run explicitly against real PostgreSQL"]
async fn route_permissions_sequence_concurrency_and_restart() -> Result<()> {
    let (s, _directory) = fixture().await?;
    let (a, ac) = identity()?;
    let (b, bc) = identity()?;
    let sa = join(&s, &a, ac).await?;
    let sb = join(&s, &b, bc).await?;
    let route = create_channel(
        State(s.clone()),
        headers(&sa)?,
        Json(ChannelCreate {
            name: "test".into(),
            idempotency_key: id(),
        }),
    )
    .await?
    .0;
    assert!(
        history(
            State(s.clone()),
            headers(&sb)?,
            Path(route.clone()),
            Query(HistoryQuery { after: 0 })
        )
        .await
        .is_err()
    );
    let upload = EventUpload {
        protocol_version: 1,
        event_id: id(),
        route_kind: "channel".into(),
        route_id: route.clone(),
        sender_member_id: sa.membership.value.member_id.clone(),
        sender_device_id: sa.membership.value.device_id.clone(),
        client_created_at: now()?,
        content_encoding: "mls.application".into(),
        ciphertext: encode(b"opaque test bytes"),
        idempotency_key: id(),
    };
    assert!(
        upload_event(
            State(s.clone()),
            headers(&sb)?,
            Path(route.clone()),
            Json(upload.clone())
        )
        .await
        .is_err()
    );
    let (first, second) = tokio::join!(
        upload_event(
            State(s.clone()),
            headers(&sa)?,
            Path(route.clone()),
            Json(upload.clone())
        ),
        upload_event(
            State(s.clone()),
            headers(&sa)?,
            Path(route.clone()),
            Json(upload.clone())
        )
    );
    let first = first?.0;
    let second = second?.0;
    assert_eq!(first.server_sequence, 1);
    assert_eq!(json(&first)?, json(&second)?);
    let mut changed = upload;
    changed.ciphertext = encode(b"different");
    assert_eq!(
        upload_event(
            State(s.clone()),
            headers(&sa)?,
            Path(route.clone()),
            Json(changed)
        )
        .await
        .err()
        .context("conflicting retry accepted")?
        .0,
        StatusCode::CONFLICT
    );
    let reloaded = Service::new(s.0.store.clone(), s.0.identity.clone(), 60, 900)?;
    let events = history(
        State(reloaded),
        headers(&sa)?,
        Path(route),
        Query(HistoryQuery { after: 0 }),
    )
    .await?
    .0;
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].server_sequence, 1);
    Ok(())
}

#[tokio::test]
#[ignore = "requires CORDS_TEST_DATABASE_URL; run explicitly against real PostgreSQL"]
async fn parser_limits_and_device_session_revocation() -> Result<()> {
    let (s, _directory) = fixture().await?;
    let (a, ac) = identity()?;
    let session = join(&s, &a, ac).await?;
    let app = router(s.clone());
    let malformed = app
        .clone()
        .oneshot(
            Request::post("/api/v1/routes/no-route/events")
                .header("content-type", "application/json")
                .body(Body::from("{broken"))?,
        )
        .await?;
    assert_eq!(malformed.status(), StatusCode::BAD_REQUEST);
    let oversized = app
        .oneshot(
            Request::post("/api/v1/join/request")
                .header("content-type", "application/json")
                .body(Body::from(vec![b'x'; MAX_ENVELOPE + 1]))?,
        )
        .await?;
    assert_eq!(oversized.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert!(to_bytes(oversized.into_body(), 4096).await?.len() < 4096);
    assert!(
        s.authenticate_token(&session.token, "channel.read")
            .await
            .is_ok()
    );
    sqlx::query("UPDATE sessions SET expires_at=0")
        .execute(s.0.store.pool())
        .await?;
    assert!(
        s.authenticate_token(&session.token, "channel.read")
            .await
            .is_err()
    );
    Ok(())
}

#[tokio::test]
#[ignore = "requires CORDS_TEST_DATABASE_URL; run explicitly against real PostgreSQL"]
async fn failed_event_transaction_does_not_consume_sequence_or_idempotency() -> Result<()> {
    let (s, _directory) = fixture().await?;
    let (a, contact) = identity()?;
    let session = join(&s, &a, contact).await?;
    let route = create_channel(
        State(s.clone()),
        headers(&session)?,
        Json(ChannelCreate {
            name: "rollback".into(),
            idempotency_key: id(),
        }),
    )
    .await?
    .0;
    let upload = EventUpload {
        protocol_version: 1,
        event_id: id(),
        route_kind: "channel".into(),
        route_id: route.clone(),
        sender_member_id: session.membership.value.member_id.clone(),
        sender_device_id: session.membership.value.device_id.clone(),
        client_created_at: now()?,
        content_encoding: "mls.application".into(),
        ciphertext: encode(b"opaque transaction fixture"),
        idempotency_key: id(),
    };
    sqlx::query("CREATE FUNCTION reject_test_event() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'injected persistence failure'; END $$").execute(s.0.store.pool()).await?;
    sqlx::query("CREATE TRIGGER injected_failure BEFORE INSERT ON route_events FOR EACH ROW EXECUTE FUNCTION reject_test_event()").execute(s.0.store.pool()).await?;
    assert!(
        upload_event(
            State(s.clone()),
            headers(&session)?,
            Path(route.clone()),
            Json(upload.clone())
        )
        .await
        .is_err()
    );
    let next: i64 = sqlx::query_scalar("SELECT next_sequence FROM channels WHERE id=$1")
        .bind(&route)
        .fetch_one(s.0.store.pool())
        .await?;
    assert_eq!(next, 1);
    let results: i64 =
        sqlx::query_scalar("SELECT count(*) FROM request_results WHERE request_key=$1")
            .bind(&upload.idempotency_key)
            .fetch_one(s.0.store.pool())
            .await?;
    assert_eq!(results, 0);
    sqlx::query("DROP TRIGGER injected_failure ON route_events")
        .execute(s.0.store.pool())
        .await?;
    let accepted = upload_event(
        State(s.clone()),
        headers(&session)?,
        Path(route),
        Json(upload),
    )
    .await?
    .0;
    assert_eq!(accepted.server_sequence, 1);
    Ok(())
}

#[tokio::test]
#[ignore = "requires CORDS_TEST_DATABASE_URL; run explicitly against real PostgreSQL"]
async fn authentication_budget_is_bounded_and_recovers() -> Result<()> {
    let (s, _directory) = fixture().await?;
    for _ in 0..64 {
        s.limit_authentication()?;
    }
    assert_eq!(
        s.limit_authentication()
            .err()
            .context("missing rate limit")?
            .0,
        StatusCode::TOO_MANY_REQUESTS
    );
    s.0.authentication_budget
        .lock()
        .map_err(|_| anyhow::anyhow!("poisoned budget"))?
        .0 = Instant::now()
        .checked_sub(Duration::from_secs(2))
        .context("monotonic clock")?;
    s.limit_authentication()?;
    Ok(())
}

#[tokio::test]
#[ignore = "requires CORDS_TEST_DATABASE_URL; run explicitly against real PostgreSQL"]
async fn root_revocation_invalidates_sessions_and_rejects_rollback() -> Result<()> {
    use cords_protocol::messaging::RevocationRequest;
    let (s, _directory) = fixture().await?;
    let (a, contact) = identity()?;
    let issued = join(&s, &a, contact.clone()).await?;
    let request = RevocationRequest {
        record: a.revoke(now()?)?,
        idempotency_key: id(),
    };
    let mut tampered = request.clone();
    tampered.record.value.generation += 1;
    assert!(
        lifecycle::revoke(State(s.clone()), Json(tampered))
            .await
            .is_err()
    );
    assert!(
        s.authenticate_token(&issued.token, "channel.read")
            .await
            .is_ok()
    );
    assert!(
        lifecycle::revoke(State(s.clone()), Json(request.clone()))
            .await?
            .0
    );
    assert!(lifecycle::revoke(State(s.clone()), Json(request)).await?.0);
    assert!(
        s.authenticate_token(&issued.token, "channel.read")
            .await
            .is_err()
    );
    assert!(join(&s, &a, contact).await.is_err());
    let revoked: i64 = sqlx::query_scalar("SELECT count(*) FROM device_revocations")
        .fetch_one(s.0.store.pool())
        .await?;
    assert_eq!(revoked, 1);
    let reloaded = Service::new(s.0.store.clone(), s.0.identity.clone(), 60, 900)?;
    assert!(
        reloaded
            .authenticate_token(&issued.token, "channel.read")
            .await
            .is_err()
    );
    Ok(())
}

#[tokio::test]
#[ignore = "requires CORDS_TEST_DATABASE_URL; run explicitly against real PostgreSQL"]
async fn mutation_authorization_serializes_with_root_revocation() -> Result<()> {
    use cords_protocol::messaging::RevocationRequest;
    let (s, _directory) = fixture().await?;
    let (a, contact) = identity()?;
    let issued = join(&s, &a, contact).await?;
    let auth = headers(&issued)?;
    let mut mutation = s.0.store.pool().begin().await?;
    s.authorize_mutation(&mut mutation, &auth, &issued.membership, None)
        .await?;
    let mut contender = s.0.store.pool().begin().await?;
    let acquired: bool =
        sqlx::query_scalar("SELECT pg_try_advisory_xact_lock(hashtextextended($1,0))")
            .bind(&issued.membership.value.account_id)
            .fetch_one(&mut *contender)
            .await?;
    assert!(
        !acquired,
        "revocation must wait for the authorized mutation"
    );
    contender.rollback().await?;
    let service = s.clone();
    let request = RevocationRequest {
        record: a.revoke(now()?)?,
        idempotency_key: id(),
    };
    let mut revocation =
        tokio::spawn(async move { lifecycle::revoke(State(service), Json(request)).await });
    assert!(
        tokio::time::timeout(Duration::from_millis(100), &mut revocation)
            .await
            .is_err()
    );
    mutation.commit().await?;
    assert!(
        tokio::time::timeout(Duration::from_secs(5), revocation)
            .await???
            .0
    );
    let mut after = s.0.store.pool().begin().await?;
    assert!(
        s.authorize_mutation(&mut after, &auth, &issued.membership, None)
            .await
            .is_err()
    );
    after.rollback().await?;
    Ok(())
}

#[tokio::test]
#[ignore = "requires CORDS_TEST_DATABASE_URL; run explicitly against real PostgreSQL"]
async fn initial_binding_is_authenticated_durable_and_idempotent() -> Result<()> {
    use cords_protocol::messaging::{ChannelBind, GroupBinding};
    let (s, _directory) = fixture().await?;
    let (a, contact) = identity()?;
    let issued = join(&s, &a, contact).await?;
    let route = create_channel(
        State(s.clone()),
        headers(&issued)?,
        Json(ChannelCreate {
            name: "bound at creation".into(),
            idempotency_key: id(),
        }),
    )
    .await?
    .0;
    let request = ChannelBind {
        binding: a.sign_device(GroupBinding {
            version: 1,
            server_id: s.0.identity.server_id(),
            channel_id: route.clone(),
            group_id: route.clone(),
            creator_device_id: a.authorization.value.device_id.clone(),
        })?,
        idempotency_key: id(),
    };
    let mut tampered = request.clone();
    tampered.binding.value.group_id = id();
    assert!(
        lifecycle::bind(
            State(s.clone()),
            headers(&issued)?,
            Path(route.clone()),
            Json(tampered)
        )
        .await
        .is_err()
    );
    for _ in 0..2 {
        assert!(
            lifecycle::bind(
                State(s.clone()),
                headers(&issued)?,
                Path(route.clone()),
                Json(request.clone())
            )
            .await?
            .0
        );
    }
    let stored: String =
        sqlx::query_scalar("SELECT view FROM channel_epochs WHERE channel_id=$1 AND epoch=0")
            .bind(&route)
            .fetch_one(s.0.store.pool())
            .await?;
    let view: Channel = parse(&stored)?;
    assert_eq!(
        json(&view.binding.context("missing initial binding")?)?,
        json(&request.binding)?
    );
    assert_eq!(view.members.len(), 1);
    let mut replacement = request;
    replacement.idempotency_key = id();
    assert!(
        lifecycle::bind(
            State(s.clone()),
            headers(&issued)?,
            Path(route),
            Json(replacement)
        )
        .await
        .is_err()
    );
    Ok(())
}
