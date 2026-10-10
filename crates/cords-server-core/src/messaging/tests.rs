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

#[tokio::test]
#[ignore = "requires CORDS_TEST_DATABASE_URL; real PostgreSQL"]
async fn public_channel_signatures_identity_and_succession() -> Result<()> {
    use cords_protocol::messaging::{ChannelSuccession, Message, PublicMessage};
    let (s, _directory, code) = fixture_with_state(false).await?;
    let (owner, contact) = identity()?;
    let session = claim_ownership(
        State(s.clone()),
        Json(ownership_request(&s, &owner, contact.clone(), &code).await?),
    )
    .await?
    .0;
    let route = create_channel(
        State(s.clone()),
        headers(&session)?,
        Json(ChannelCreate {
            name: "same name".into(),
            confidentiality_mode: ConfidentialityMode::Public,
            idempotency_key: id(),
        }),
    )
    .await?
    .0;
    let payload = owner.sign_device(PublicMessage {
        version: 1,
        server_id: s.0.identity.server_id(),
        channel_id: route.clone(),
        event_id: id(),
        idempotency_key: id(),
        contact: contact.clone(),
        membership: session.membership.clone(),
        message: Message {
            schema_version: 1,
            message_id: id(),
            sender_account_id: owner.authorization.value.account_id.clone(),
            sender_device_id: owner.authorization.value.device_id.clone(),
            client_timestamp: now()?,
            event_kind: "message.create".into(),
            body: "PUBLIC_PLAINTEXT_MARKER".into(),
        },
    })?;
    let upload = EventUpload {
        protocol_version: 1,
        event_id: payload.value.event_id.clone(),
        route_kind: "channel".into(),
        route_id: route.clone(),
        sender_member_id: session.membership.value.member_id.clone(),
        sender_device_id: owner.authorization.value.device_id.clone(),
        client_created_at: payload.value.message.client_timestamp,
        content_encoding: "public.signed".into(),
        ciphertext: String::new(),
        idempotency_key: payload.value.idempotency_key.clone(),
        public_message: Some(payload.clone()),
    };
    let event = upload_event(
        State(s.clone()),
        headers(&session)?,
        Path(route.clone()),
        Json(upload.clone()),
    )
    .await?
    .0;
    let retry = upload_event(
        State(s.clone()),
        headers(&session)?,
        Path(route.clone()),
        Json(upload.clone()),
    )
    .await?
    .0;
    assert_eq!(event.server_sequence, retry.server_sequence);
    let stored: String = sqlx::query_scalar("SELECT envelope FROM route_events WHERE route_id=$1")
        .bind(&route)
        .fetch_one(s.0.store.pool())
        .await?;
    assert!(stored.contains("PUBLIC_PLAINTEXT_MARKER"));
    let mut tampered = upload.clone();
    tampered
        .public_message
        .as_mut()
        .context("missing public record")?
        .value
        .message
        .body = "tampered".into();
    assert!(
        upload_event(
            State(s.clone()),
            headers(&session)?,
            Path(route.clone()),
            Json(tampered)
        )
        .await
        .is_err()
    );
    let other = create_channel(
        State(s.clone()),
        headers(&session)?,
        Json(ChannelCreate {
            name: "same name".into(),
            confidentiality_mode: ConfidentialityMode::Public,
            idempotency_key: id(),
        }),
    )
    .await?
    .0;
    let mut replay = upload.clone();
    replay.route_id = other.clone();
    assert!(
        upload_event(
            State(s.clone()),
            headers(&session)?,
            Path(other),
            Json(replay)
        )
        .await
        .is_err()
    );
    for mode in ["encrypted", "public"] {
        let target = if mode == "encrypted" {
            "public"
        } else {
            "encrypted"
        };
        let r = if mode == "public" {
            route.clone()
        } else {
            create_channel(
                State(s.clone()),
                headers(&session)?,
                Json(ChannelCreate {
                    name: "encrypted".into(),
                    confidentiality_mode: ConfidentialityMode::Encrypted,
                    idempotency_key: id(),
                }),
            )
            .await?
            .0
        };
        assert!(
            sqlx::query("UPDATE channels SET confidentiality_mode=$1 WHERE id=$2")
                .bind(target)
                .bind(r)
                .execute(s.0.store.pool())
                .await
                .is_err()
        );
    }
    assert!(
        sqlx::query("DELETE FROM channels WHERE id=$1")
            .bind(&route)
            .execute(s.0.store.pool())
            .await
            .is_err()
    );
    let predecessor = channel_view(&s, &route)
        .await?
        .identity
        .context("missing identity")?;
    let successor = id();
    let request = ChannelReplace {
        record: owner.sign_device(ChannelSuccession {
            version: 1,
            server_id: s.0.identity.server_id(),
            predecessor,
            successor_channel_id: successor.clone(),
            name: "same name".into(),
            confidentiality_mode: ConfidentialityMode::Encrypted,
            initiator: contact,
            membership: session.membership.clone(),
            issued_at: now()?,
            nonce: id(),
        })?,
        idempotency_key: id(),
    };
    assert_eq!(
        replace_channel(
            State(s.clone()),
            headers(&session)?,
            Path(route.clone()),
            Json(request.clone())
        )
        .await?
        .0,
        successor
    );
    assert_eq!(
        replace_channel(
            State(s.clone()),
            headers(&session)?,
            Path(route.clone()),
            Json(request)
        )
        .await?
        .0,
        successor
    );
    let new = channel_view(&s, &successor).await?;
    assert!(
        new.transition
            .context("transition")?
            .value
            .succession
            .is_some()
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM route_events WHERE route_id=$1")
        .bind(&successor)
        .fetch_one(s.0.store.pool())
        .await?;
    assert_eq!(count, 0);
    assert!(
        upload_event(
            State(s.clone()),
            headers(&session)?,
            Path(route),
            Json(upload)
        )
        .await
        .is_err()
    );
    Ok(())
}

#[tokio::test]
#[ignore = "requires CORDS_TEST_DATABASE_URL; real TLS and PostgreSQL"]
async fn public_successor_two_clients_restart_archive_and_acknowledgement() -> Result<()> {
    use cords_client_core::client::Client;
    let (base, _directory, code) = fixture_with_state(false).await?;
    let service = Service::with_policy(
        base.0.store.clone(),
        base.0.identity.clone(),
        60,
        900,
        JoinPolicy::ModeratorApproval,
    )?;
    let certificate = rcgen::generate_simple_self_signed(vec!["localhost".into()])?;
    let tls = https_fixture(&service, &certificate).await?;
    let root = tempfile::tempdir()?;
    let ca = root.path().join("test-ca.pem");
    std::fs::write(&ca, certificate.cert.pem())?;
    let migrations =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../migrations/sqlite");
    let pass = b"public succession independent vault test";
    let a = root.path().join("a");
    let b = root.path().join("b");
    let mut owner = Client::open(&a, &migrations, Some(pass), Some(&ca)).await?;
    let mut member = Client::open(&b, &migrations, Some(pass), Some(&ca)).await?;
    assert_ne!(owner.status().account_id, member.status().account_id);
    owner.trust(&tls.origin).await?;
    owner.claim_ownership(&code).await?;
    member.trust(&tls.origin).await?;
    assert!(member.authenticate().await.is_err());
    owner.approve_membership(&member.status().device_id).await?;
    member.authenticate().await?;
    let predecessor = owner.create_channel("reusable name").await?;
    member.publish_key_package().await?;
    owner
        .add_member(&predecessor, &member.status().device_id)
        .await?;
    member.join_channel(&predecessor).await?;
    owner.send(&predecessor, "ENCRYPTED_ARCHIVE_MARKER").await?;
    member.sync_route(&predecessor).await?;
    let old = member
        .channels()
        .await?
        .into_iter()
        .find(|c| c.channel_id == predecessor)
        .context("predecessor")?;
    assert_eq!(old.confidentiality_mode, ConfidentialityMode::Encrypted);
    member.shutdown().await;
    let successor = owner
        .replace_channel(&predecessor, "reusable name", ConfidentialityMode::Public)
        .await?;
    assert_ne!(predecessor, successor);
    assert!(owner.send(&successor, "must not be sent").await.is_err());
    owner.acknowledge_public_successor(&successor).await?;
    owner.send(&successor, "PUBLIC_NETWORK_MARKER").await?;
    let mut member = Client::open(&b, &migrations, Some(pass), Some(&ca)).await?;
    member.authenticate().await?;
    let channels = member.channels().await?;
    assert!(
        channels
            .iter()
            .find(|c| c.channel_id == predecessor)
            .context("original")?
            .locally_archived
    );
    assert!(
        channels
            .iter()
            .find(|c| c.channel_id == successor)
            .context("successor")?
            .requires_public_acknowledgement
    );
    assert_eq!(
        member.history(&predecessor).await?[0].body,
        "ENCRYPTED_ARCHIVE_MARKER"
    );
    member.join_channel(&successor).await?;
    assert_eq!(member.history(&successor).await?.len(), 1);
    assert_eq!(
        member.history(&successor).await?[0].body,
        "PUBLIC_NETWORK_MARKER"
    );
    assert!(member.send(&successor, "not acknowledged").await.is_err());
    assert!(member.send(&predecessor, "archive write").await.is_err());
    assert!(
        member
            .replace_channel(&successor, "unauthorized", ConfidentialityMode::Encrypted)
            .await
            .is_err()
    );
    member.acknowledge_public_successor(&successor).await?;
    member.shutdown().await;
    let mut member = Client::open(&b, &migrations, Some(pass), Some(&ca)).await?;
    member.authenticate().await?;
    member
        .send(&successor, "PUBLIC_SECOND_CLIENT_MARKER")
        .await?;
    owner.synchronize().await?;
    assert_eq!(owner.history(&successor).await?.len(), 2);
    let raw: Vec<String> =
        sqlx::query_scalar("SELECT envelope FROM route_events WHERE route_id=$1 ORDER BY sequence")
            .bind(&successor)
            .fetch_all(service.0.store.pool())
            .await?;
    assert!(
        raw.iter()
            .any(|v| v.contains("PUBLIC_SECOND_CLIENT_MARKER"))
    );
    assert!(!raw.iter().any(|v| v.contains("ENCRYPTED_ARCHIVE_MARKER")));
    let snapshot = member.channel_archives();
    assert_eq!(snapshot.len(), 1);
    assert_eq!(
        snapshot[0].confidentiality_mode,
        ConfidentialityMode::Encrypted
    );
    let server = member.status().server_id;
    member.remove_server(&server, true).await?;
    assert_eq!(
        member.archived_channel_history(&server, &predecessor)?[0].body,
        "ENCRYPTED_ARCHIVE_MARKER"
    );
    member.shutdown().await;
    owner.shutdown().await;
    for path in [&a, &b] {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                let bytes = std::fs::read(entry.path())?;
                for marker in [
                    "ENCRYPTED_ARCHIVE_MARKER",
                    "PUBLIC_NETWORK_MARKER",
                    "PUBLIC_SECOND_CLIENT_MARKER",
                ] {
                    assert!(
                        !bytes.windows(marker.len()).any(|b| b == marker.as_bytes()),
                        "unsealed local message data"
                    );
                }
            }
        }
    }
    Ok(())
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
    Ok((
        Service::with_policy(store, identity, 60, 900, JoinPolicy::Public)?,
        directory,
        code,
    ))
}
fn identity() -> Result<(Identity, Contact)> {
    let identity = Identity::generate(now()?)?;
    let crypto = ConversationCrypto::new(identity.authorization.value.device_id.clone())?;
    let contact = identity.contact(&crypto.public_key())?;
    Ok((identity, contact))
}

struct TestHttps {
    origin: String,
    task: tokio::task::JoinHandle<Result<()>>,
}
impl Drop for TestHttps {
    fn drop(&mut self) {
        self.task.abort();
    }
}
async fn https_fixture(
    service: &Service,
    certificate: &rcgen::CertifiedKey<rcgen::KeyPair>,
) -> Result<TestHttps> {
    use hyper_util::{
        rt::{TokioExecutor, TokioIo},
        server::conn::auto::Builder,
        service::TowerToHyperService,
    };
    use tokio_rustls::{TlsAcceptor, rustls};
    let config = rustls::ServerConfig::builder_with_provider(std::sync::Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_protocol_versions(&[&rustls::version::TLS13])?
    .with_no_client_auth()
    .with_single_cert(
        vec![certificate.cert.der().clone()],
        rustls::pki_types::PrivatePkcs8KeyDer::from(certificate.signing_key.serialize_der()).into(),
    )?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let origin = format!("https://localhost:{}", listener.local_addr()?.port());
    let acceptor = TlsAcceptor::from(std::sync::Arc::new(config));
    let app = crate::router(
        crate::AppState::new(
            service
                .0
                .identity
                .signed_metadata_with_policy("TLS acceptance", "moderator_approval")?,
        )
        .with_store(service.0.store.clone()),
    )
    .merge(router(service.clone()));
    let task = tokio::spawn(async move {
        let mut connections = tokio::task::JoinSet::new();
        loop {
            tokio::select! {
                accepted=listener.accept() => {
                    let (stream,_)=accepted?;
                    let acceptor=acceptor.clone();let app=app.clone();
                    connections.spawn(async move {
                        let stream=acceptor.accept(stream).await?;
                        Builder::new(TokioExecutor::new()).serve_connection_with_upgrades(TokioIo::new(stream),TowerToHyperService::new(app)).await
                            .map_err(|error| anyhow::anyhow!("TLS acceptance connection: {error}"))
                    });
                }
                result=connections.join_next(), if !connections.is_empty() => {result.context("connection task missing")???;}
            }
        }
    });
    Ok(TestHttps { origin, task })
}

#[tokio::test]
#[ignore = "requires CORDS_TEST_DATABASE_URL; independent clients, real TLS and PostgreSQL"]
#[allow(clippy::too_many_lines)] // One uninterrupted independent-client acceptance scenario.
async fn independent_tls_clients_admission_mls_multiserver_departure_archive_and_burn() -> Result<()>
{
    use cords_client_core::client::Client;
    let (base1, _server1, code1) = fixture_with_state(false).await?;
    let (base2, _server2, code2) = fixture_with_state(false).await?;
    let service1 = Service::with_policy(
        base1.0.store.clone(),
        base1.0.identity.clone(),
        60,
        900,
        JoinPolicy::ModeratorApproval,
    )?;
    let service2 = Service::with_policy(
        base2.0.store.clone(),
        base2.0.identity.clone(),
        60,
        900,
        JoinPolicy::ModeratorApproval,
    )?;
    let certificate = rcgen::generate_simple_self_signed(vec!["localhost".into()])?;
    let tls1 = https_fixture(&service1, &certificate).await?;
    let tls2 = https_fixture(&service2, &certificate).await?;
    let root = tempfile::tempdir()?;
    let ca = root.path().join("test-ca.pem");
    std::fs::write(&ca, certificate.cert.pem())?;
    let migrations =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../migrations/sqlite");
    let pass1 = b"independent acceptance vault A";
    let pass2 = b"independent acceptance vault B";
    let mut owner =
        Client::open(&root.path().join("a"), &migrations, Some(pass1), Some(&ca)).await?;
    let mut member =
        Client::open(&root.path().join("b"), &migrations, Some(pass2), Some(&ca)).await?;
    assert_ne!(owner.status().account_id, member.status().account_id);
    owner.trust(&tls1.origin).await?;
    owner.claim_ownership(&code1).await?;
    member
        .save_ui_preferences(serde_json::json!({"version":1,"displayName":"Second network user", "avatar": {
            "url": "https://images.example/second-user.webp",
            "data": "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+a6X0AAAAASUVORK5CYII=",
            "shape":"square", "x":25, "y":75, "zoom":1.5
        }}))
        .await?;
    member.trust(&tls1.origin).await?;
    assert!(member.authenticate().await.is_err());
    assert_eq!(member.status().admission_state, "pending");
    let requests = owner.membership_requests().await?;
    assert_eq!(
        requests[0]
            .user_card
            .as_ref()
            .context("missing published card")?
            .value
            .nickname,
        "Second network user"
    );
    let avatar = requests[0]
        .user_card
        .as_ref()
        .context("missing card")?
        .value
        .avatar
        .as_ref()
        .context("missing avatar resolution data")?;
    assert_eq!(avatar.url, "https://images.example/second-user.webp");
    assert!(avatar.data.is_empty());
    assert_eq!(
        (&avatar.shape, avatar.x, avatar.y, avatar.zoom_milli),
        (&"square".to_string(), 25, 75, 1500)
    );
    owner.approve_membership(&member.status().device_id).await?;
    member.authenticate().await?;
    assert!(member.create_channel("not permitted").await.is_err());
    let route = owner.create_channel("real encrypted acceptance").await?;
    member.publish_key_package().await?;
    owner.add_member(&route, &member.status().device_id).await?;
    member.join_channel(&route).await?;
    let (socket, mut notices) = member.notifications().await?;
    let message = owner
        .send(&route, "network-only encrypted acceptance marker")
        .await?;
    let _ = tokio::time::timeout(Duration::from_secs(5), notices.recv())
        .await?
        .context("missing websocket notification")?;
    member.synchronize().await?;
    let envelopes: Vec<String> = sqlx::query_scalar("SELECT envelope FROM route_events")
        .fetch_all(service1.0.store.pool())
        .await?;
    assert!(
        envelopes
            .iter()
            .all(|envelope| !envelope.contains("network-only encrypted acceptance marker"))
    );
    assert!(
        member
            .history(&route)
            .await?
            .iter()
            .any(|item| item.message_id == message)
    );
    socket.abort();
    let pin1 = owner.status().server_id;
    owner.trust(&tls2.origin).await?;
    owner.claim_ownership(&code2).await?;
    assert_eq!(owner.servers().len(), 2);
    owner.select_server(&pin1).await?;
    owner.authenticate().await?;
    assert_eq!(
        owner
            .history(&route)
            .await?
            .last()
            .context("missing original cache")?
            .body,
        "network-only encrypted acceptance marker"
    );
    assert!(member.remove_server(&pin1, false).await?.remote_confirmed);
    assert!(member.servers().is_empty());
    assert!(member.history(&route).await?.is_empty());
    owner.synchronize().await?;
    let active: bool = sqlx::query_scalar(
        "SELECT active FROM channel_members WHERE channel_id=$1 AND device_id=$2",
    )
    .bind(&route)
    .bind(member.status().device_id)
    .fetch_one(service1.0.store.pool())
    .await?;
    assert!(!active);
    assert!(owner.archive_server(&pin1).await?.remote_confirmed);
    assert_eq!(
        ownership_state(State(service1.clone()))
            .await?
            .0
            .value
            .state,
        "OWNER_LOCKDOWN"
    );
    assert!(!owner.status().server_id.is_empty());
    owner
        .create_channel("second server remains independent")
        .await?;
    let outcome = owner.burn_identity().await?;
    assert_eq!(outcome.confirmed, 1);
    assert!(outcome.pending.is_empty());
    assert_eq!(
        ownership_state(State(service2.clone()))
            .await?
            .0
            .value
            .state,
        "OWNER_LOCKDOWN"
    );
    assert!(!tls1.task.is_finished() && !tls2.task.is_finished());
    owner.shutdown().await;
    member.shutdown().await;
    let restored =
        Client::open(&root.path().join("a"), &migrations, Some(pass1), Some(&ca)).await?;
    assert!(restored.is_burned());
    assert!(
        restored
            .servers()
            .iter()
            .any(|server| server.server_id == pin1 && server.archived)
    );
    Ok(())
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
#[ignore = "requires CORDS_TEST_DATABASE_URL; real PostgreSQL"]
async fn pending_admission_retains_signed_self_declared_card_and_denies_permissions() -> Result<()>
{
    use cords_protocol::messaging::UserCard;
    let (base, _directory, code) = fixture_with_state(false).await?;
    let s = Service::with_policy(
        base.0.store.clone(),
        base.0.identity.clone(),
        60,
        900,
        JoinPolicy::ModeratorApproval,
    )?;
    let (owner, owner_contact) = identity()?;
    let owner_session = claim_ownership(
        State(s.clone()),
        Json(ownership_request(&s, &owner, owner_contact, &code).await?),
    )
    .await?
    .0;
    let (other, mut contact) = identity()?;
    contact.user_card = Some(other.sign_device(UserCard {
        version: 1,
        account_id: other.authorization.value.account_id.clone(),
        device_id: other.authorization.value.device_id.clone(),
        nickname: "Second user".into(),
        avatar: None,
        issued_at: now()?,
    })?);
    assert_eq!(
        join(&s, &other, contact.clone())
            .await
            .err()
            .context("pending admission succeeded")?
            .downcast_ref::<ApiError>()
            .context("unexpected admission error")?
            .1,
        "CORDS_APPROVAL_PENDING"
    );
    let requests = membership_requests(
        State(s.clone()),
        headers(&owner_session)?,
        Query(MembershipPage::default()),
    )
    .await?
    .0;
    assert_eq!(requests.len(), 1);
    assert_eq!(json(&requests[0].user_card)?, json(&contact.user_card)?);
    assert_eq!(requests[0].account_id, other.authorization.value.account_id);
    let _ = approve_membership(
        State(s.clone()),
        headers(&owner_session)?,
        Path(other.authorization.value.device_id.clone()),
    )
    .await?;
    let member = join(&s, &other, contact).await?;
    assert!(
        !member
            .membership
            .value
            .capabilities
            .iter()
            .any(|c| c == "server.manage" || c == "channel.create")
    );
    assert!(
        membership_requests(
            State(s.clone()),
            headers(&member)?,
            Query(MembershipPage::default())
        )
        .await
        .is_err()
    );
    assert!(
        create_channel(
            State(s),
            headers(&member)?,
            Json(ChannelCreate {
                confidentiality_mode: Default::default(),
                name: "unauthorized".into(),
                idempotency_key: id()
            })
        )
        .await
        .is_err()
    );
    Ok(())
}

#[tokio::test]
#[ignore = "requires CORDS_TEST_DATABASE_URL; real PostgreSQL"]
#[allow(clippy::too_many_lines)] // Validate the complete irreversible burn and concurrent recovery scenario.
async fn owner_burn_lockdown_recovery_is_atomic_one_time_and_revokes_old_authority() -> Result<()> {
    use cords_protocol::messaging::{
        DepartureRequest, IdentityBurn, OwnershipRecoveryClaim, OwnershipRecoveryProof,
    };
    let (s, _directory, code) = fixture_with_state(false).await?;
    let (owner, contact) = identity()?;
    let old_session = claim_ownership(
        State(s.clone()),
        Json(ownership_request(&s, &owner, contact, &code).await?),
    )
    .await?
    .0;
    let burn = DepartureRequest {
        record: owner.sign_root(IdentityBurn {
            version: 1,
            server_id: s.0.identity.server_id(),
            account_id: owner.authorization.value.account_id.clone(),
            issued_at: now()?,
            nonce: "A".repeat(43),
        })?,
        idempotency_key: id(),
    };
    let _ = departure::burn_identity(State(s.clone()), Json(burn.clone())).await?;
    let _ = departure::burn_identity(State(s.clone()), Json(burn)).await?;
    assert_eq!(
        ownership_state(State(s.clone())).await?.0.value.state,
        "OWNER_LOCKDOWN"
    );
    assert!(
        crate::ownership::rotate(&s.0.store, &s.0.identity.server_id())
            .await
            .is_err()
    );
    assert!(
        create_channel(
            State(s.clone()),
            headers(&old_session)?,
            Json(ChannelCreate {
                confidentiality_mode: Default::default(),
                name: "blocked".into(),
                idempotency_key: id()
            })
        )
        .await
        .is_err()
    );
    let (next, contact) = identity()?;
    assert_eq!(
        join(&s, &next, contact.clone())
            .await
            .err()
            .context("lockdown admitted member")?
            .downcast_ref::<ApiError>()
            .context("unexpected admission error")?
            .1,
        "OWNER_RECOVERY_REQUIRED"
    );
    let recovery_code =
        crate::ownership::rotate_recovery_code(&s.0.store, &s.0.identity.server_id()).await?;
    let ch = departure::recovery_challenge(
        State(s.clone()),
        Json(ChallengeRequest {
            contact: contact.clone(),
            purpose: "recover_ownership".into(),
            idempotency_key: id(),
        }),
    )
    .await?
    .0;
    let request = OwnershipRecoveryClaim {
        recovery_code: recovery_code.to_string(),
        contact,
        device_proof: next.sign_device(ch.clone())?,
        root_proof: next.sign_root(OwnershipRecoveryProof {
            version: 1,
            server_id: ch.server_id.clone(),
            account_id: ch.account_id.clone(),
            device_id: ch.device_id.clone(),
            challenge_hash: hash(canonical(&ch)?),
        })?,
        idempotency_key: id(),
    };
    let mut wrong = request.clone();
    wrong.recovery_code = "B".repeat(43);
    assert!(
        departure::recover_owner(State(s.clone()), Json(wrong))
            .await
            .is_err()
    );
    let (a, b) = tokio::join!(
        departure::recover_owner(State(s.clone()), Json(request.clone())),
        departure::recover_owner(State(s.clone()), Json(request.clone()))
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    let recovered = match (a, b) {
        (Ok(session), _) | (_, Ok(session)) => session.0,
        _ => anyhow::bail!("no recovery winner"),
    };
    assert!(
        departure::recover_owner(State(s.clone()), Json(request))
            .await
            .is_err()
    );
    assert_eq!(
        ownership_state(State(s.clone())).await?.0.value.state,
        "CLAIMED"
    );
    let old: String = sqlx::query_scalar("SELECT credential FROM memberships WHERE device_id=$1")
        .bind(&owner.authorization.value.device_id)
        .fetch_one(s.0.store.pool())
        .await?;
    assert!(
        !parse::<Signed<Membership>>(&old)?
            .value
            .capabilities
            .iter()
            .any(|c| c == "server.manage")
    );
    let tombstones: i64 = sqlx::query_scalar("SELECT count(*) FROM identity_burns")
        .fetch_one(s.0.store.pool())
        .await?;
    assert_eq!(tombstones, 1);
    let crypto = ConversationCrypto::new(owner.authorization.value.device_id.clone())?;
    assert!(
        join(&s, &owner, owner.contact(&crypto.public_key())?)
            .await
            .is_err()
    );
    let requests = membership_requests(
        State(s.clone()),
        headers(&recovered)?,
        Query(MembershipPage::default()),
    )
    .await?
    .0;
    assert_eq!(requests.len(), 1);
    assert!(requests[0].identity_burned);
    // Operator lockdown is repeatable and never reopens the first-owner bootstrap.
    crate::ownership::begin_recovery(&s.0.store, &s.0.identity.server_id()).await?;
    let generation = ownership_state(State(s.clone())).await?.0.value.generation;
    crate::ownership::begin_recovery(&s.0.store, &s.0.identity.server_id()).await?;
    assert_eq!(
        ownership_state(State(s.clone())).await?.0.value.generation,
        generation
    );
    Ok(())
}

#[tokio::test]
#[ignore = "requires CORDS_TEST_DATABASE_URL; real PostgreSQL"]
async fn succession_matures_on_server_time_and_owner_cannot_replace_it() -> Result<()> {
    use cords_protocol::messaging::{
        DepartureRequest, IdentityBurn, SuccessorAcceptance, SuccessorDesignation, SuccessorRequest,
    };
    for mature in [false, true] {
        let (s, _directory, code) = fixture_with_state(false).await?;
        let (owner, contact) = identity()?;
        let _ = claim_ownership(
            State(s.clone()),
            Json(ownership_request(&s, &owner, contact, &code).await?),
        )
        .await?;
        let (next, contact) = identity()?;
        join(&s, &next, contact).await?;
        let request = SuccessorRequest {
            record: owner.sign_root(SuccessorDesignation {
                version: 1,
                server_id: s.0.identity.server_id(),
                owner_account_id: owner.authorization.value.account_id.clone(),
                successor_account_id: next.authorization.value.account_id.clone(),
                nonce: "A".repeat(43),
            })?,
            idempotency_key: id(),
        };
        let designation = departure::designate_successor(State(s.clone()), Json(request.clone()))
            .await?
            .0;
        assert_eq!(
            departure::designate_successor(State(s.clone()), Json(request.clone()))
                .await?
                .0,
            designation
        );
        let mut replacement = request;
        replacement.idempotency_key = id();
        replacement.record.value.nonce = "B".repeat(43);
        replacement.record = owner.sign_root(replacement.record.value)?;
        assert!(
            departure::designate_successor(State(s.clone()), Json(replacement))
                .await
                .is_err()
        );
        let acceptance = SuccessorRequest {
            record: next.sign_root(SuccessorAcceptance {
                version: 1,
                server_id: s.0.identity.server_id(),
                successor_account_id: next.authorization.value.account_id.clone(),
                designation_hash: designation.clone(),
                nonce: "A".repeat(43),
            })?,
            idempotency_key: id(),
        };
        for _ in 0..2 {
            assert!(
                departure::accept_successor(State(s.clone()), Json(acceptance.clone()))
                    .await?
                    .0
            );
        }
        sqlx::query("UPDATE server_successor_designations SET accepted_at=$1 WHERE id=$2")
            .bind(int(now()?.saturating_sub(if mature {
                30 * 86400
            } else {
                30 * 86400 - 60
            }))?)
            .bind(designation)
            .execute(s.0.store.pool())
            .await?;
        let burn = DepartureRequest {
            record: owner.sign_root(IdentityBurn {
                version: 1,
                server_id: s.0.identity.server_id(),
                account_id: owner.authorization.value.account_id.clone(),
                issued_at: now()?,
                nonce: "A".repeat(43),
            })?,
            idempotency_key: id(),
        };
        let _ = departure::burn_identity(State(s.clone()), Json(burn)).await?;
        assert_eq!(
            ownership_state(State(s.clone())).await?.0.value.state,
            if mature { "CLAIMED" } else { "OWNER_LOCKDOWN" }
        );
        let current: String =
            sqlx::query_scalar("SELECT account_id FROM server_ownership WHERE singleton=TRUE")
                .fetch_one(s.0.store.pool())
                .await?;
        assert_eq!(
            current,
            if mature {
                next.authorization.value.account_id
            } else {
                owner.authorization.value.account_id
            }
        );
    }
    Ok(())
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
async fn grant_fixture_owner(s: &Service, mut session: Session) -> Result<Session> {
    let member = session.membership.value.clone();
    sqlx::query("INSERT INTO server_ownership(singleton,account_id,claimed_by_device_id,claimed_at) VALUES(TRUE,$1,$2,$3)")
        .bind(&member.account_id).bind(&member.device_id).bind(int(now()?)?)
        .execute(s.0.store.pool()).await?;
    let mut elevated = member.clone();
    elevated.capabilities = member_capabilities(true);
    elevated.generation += 1;
    session.membership = s.sign(elevated)?;
    sqlx::query("UPDATE memberships SET credential=$2 WHERE device_id=$1")
        .bind(&member.device_id)
        .bind(json(&session.membership)?)
        .execute(s.0.store.pool())
        .await?;
    Ok(session)
}

#[tokio::test]
#[ignore = "requires CORDS_TEST_DATABASE_URL; run explicitly against real PostgreSQL"]
#[allow(clippy::too_many_lines)] // The full two-identity lifecycle is one regression scenario.
async fn approval_and_shared_channel_identity_survive_restart() -> Result<()> {
    let (base, _directory, code) = fixture_with_state(false).await?;
    let s = Service::with_policy(
        base.0.store.clone(),
        base.0.identity.clone(),
        60,
        900,
        JoinPolicy::ModeratorApproval,
    )?;
    let discovery =
        s.0.identity
            .signed_metadata_with_policy("Test", s.0.join_policy.as_str())?;
    discovery.metadata.verify(&discovery.signature)?;
    assert_eq!(discovery.metadata.join_policy, ["moderator_approval"]);
    let (owner, owner_contact) = identity()?;
    let owner_session = claim_ownership(
        State(s.clone()),
        Json(ownership_request(&s, &owner, owner_contact, &code).await?),
    )
    .await?
    .0;
    let (other, other_contact) = identity()?;
    assert_ne!(
        owner.authorization.value.account_id,
        other.authorization.value.account_id
    );
    let route = create_channel(
        State(s.clone()),
        headers(&owner_session)?,
        Json(ChannelCreate {
            confidentiality_mode: Default::default(),
            name: "test".into(),
            idempotency_key: id(),
        }),
    )
    .await?
    .0;
    let pending_challenge = join_challenge(
        State(s.clone()),
        Json(ChallengeRequest {
            contact: other_contact.clone(),
            purpose: "join".into(),
            idempotency_key: id(),
        }),
    )
    .await?
    .0;
    let pending_request = SessionRequest {
        proof: other.sign_device(pending_challenge)?,
        idempotency_key: id(),
    };
    let pending = session(State(s.clone()), Json(pending_request.clone()))
        .await
        .err()
        .context("unapproved account got a session")?;
    assert_eq!(pending.1, "CORDS_APPROVAL_PENDING");
    assert_eq!(
        session(State(s.clone()), Json(pending_request))
            .await
            .err()
            .context("pending replay succeeded")?
            .1,
        "CORDS_APPROVAL_PENDING"
    );
    let membership_count: i64 = sqlx::query_scalar("SELECT count(*) FROM memberships")
        .fetch_one(s.0.store.pool())
        .await?;
    assert_eq!(membership_count, 1);
    assert_eq!(
        membership_requests(
            State(s.clone()),
            headers(&owner_session)?,
            Query(MembershipPage::default())
        )
        .await?
        .0
        .len(),
        1
    );
    let unauthenticated = create_channel(
        State(s.clone()),
        HeaderMap::new(),
        Json(ChannelCreate {
            confidentiality_mode: Default::default(),
            name: "test".into(),
            idempotency_key: id(),
        }),
    )
    .await
    .err()
    .context("unauthenticated channel creation succeeded")?;
    assert_eq!(unauthenticated.0, StatusCode::FORBIDDEN);
    let _approved = approve_membership(
        State(s.clone()),
        headers(&owner_session)?,
        Path(other.authorization.value.device_id.clone()),
    )
    .await?;
    let other_session = join(&s, &other, other_contact).await?;
    assert_eq!(
        owner_session.membership.value.server_id,
        discovery.metadata.server_id
    );
    assert_eq!(
        other_session.membership.value.server_id,
        discovery.metadata.server_id
    );
    assert!(
        !other_session
            .membership
            .value
            .capabilities
            .iter()
            .any(|v| v == "channel.create" || v == "server.manage")
    );
    let denied = create_channel(
        State(s.clone()),
        headers(&other_session)?,
        Json(ChannelCreate {
            confidentiality_mode: Default::default(),
            name: "test".into(),
            idempotency_key: id(),
        }),
    )
    .await
    .err()
    .context("ordinary member created a channel")?;
    assert_eq!(denied.0, StatusCode::FORBIDDEN);
    let mut legacy = other_session.membership.value.clone();
    legacy.capabilities.push("channel.create".into());
    let legacy = s.sign(legacy)?;
    sqlx::query("UPDATE memberships SET credential=$2 WHERE device_id=$1")
        .bind(&other.authorization.value.device_id)
        .bind(json(&legacy)?)
        .execute(s.0.store.pool())
        .await?;
    let denied_legacy = create_channel(
        State(s.clone()),
        headers(&other_session)?,
        Json(ChannelCreate {
            confidentiality_mode: Default::default(),
            name: "test".into(),
            idempotency_key: id(),
        }),
    )
    .await
    .err()
    .context("legacy member capability bypassed owner gate")?;
    assert_eq!(denied_legacy.0, StatusCode::FORBIDDEN);
    let owner_channels = channels(State(s.clone()), headers(&owner_session)?)
        .await?
        .0;
    let other_channels = channels(State(s.clone()), headers(&other_session)?)
        .await?
        .0;
    assert_eq!(owner_channels.len(), 1);
    assert_eq!(other_channels.len(), 1);
    assert_eq!(owner_channels[0].channel_id, route);
    assert_eq!(other_channels[0].channel_id, route);
    assert!(other_channels[0].members.is_empty());
    assert!(
        channel(
            State(s.clone()),
            headers(&other_session)?,
            Path(route.clone()),
            Query(ChannelQuery { epoch: None })
        )
        .await
        .is_err()
    );
    let persisted: (String, String) = sqlx::query_as("SELECT id,creator FROM channels WHERE id=$1")
        .bind(&route)
        .fetch_one(s.0.store.pool())
        .await?;
    assert_eq!(
        persisted,
        (route.clone(), owner.authorization.value.device_id.clone())
    );
    let restarted = Service::with_policy(
        s.0.store.clone(),
        s.0.identity.clone(),
        60,
        900,
        JoinPolicy::ModeratorApproval,
    )?;
    assert_eq!(
        channels(State(restarted.clone()), headers(&other_session)?)
            .await?
            .0[0]
            .channel_id,
        route
    );
    assert_eq!(
        ownership_state(State(restarted)).await?.0.value.state,
        "CLAIMED"
    );
    Ok(())
}

#[tokio::test]
#[ignore = "requires CORDS_TEST_DATABASE_URL; run explicitly against real PostgreSQL"]
#[allow(clippy::too_many_lines)] // Rejection and explicit public admission share one persistent fixture.
async fn rejected_request_stays_nonmember_and_public_policy_is_explicit() -> Result<()> {
    let (base, _directory, code) = fixture_with_state(false).await?;
    let approval = Service::with_policy(
        base.0.store.clone(),
        base.0.identity.clone(),
        60,
        900,
        JoinPolicy::ModeratorApproval,
    )?;
    let (owner, owner_contact) = identity()?;
    let owner_session = claim_ownership(
        State(approval.clone()),
        Json(ownership_request(&approval, &owner, owner_contact, &code).await?),
    )
    .await?
    .0;
    let (other, contact) = identity()?;
    let ch = join_challenge(
        State(approval.clone()),
        Json(ChallengeRequest {
            contact: contact.clone(),
            purpose: "join".into(),
            idempotency_key: id(),
        }),
    )
    .await?
    .0;
    assert_eq!(
        session(
            State(approval.clone()),
            Json(SessionRequest {
                proof: other.sign_device(ch)?,
                idempotency_key: id(),
            })
        )
        .await
        .err()
        .context("unapproved account got a session")?
        .1,
        "CORDS_APPROVAL_PENDING"
    );
    assert_eq!(
        reject_membership(
            State(approval.clone()),
            headers(&owner_session)?,
            Path(other.authorization.value.device_id.clone())
        )
        .await?,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        membership_requests(
            State(approval.clone()),
            headers(&owner_session)?,
            Query(MembershipPage::default())
        )
        .await?
        .0[0]
            .status,
        "rejected"
    );
    let ch = join_challenge(
        State(approval.clone()),
        Json(ChallengeRequest {
            contact: contact.clone(),
            purpose: "join".into(),
            idempotency_key: id(),
        }),
    )
    .await?
    .0;
    assert_eq!(
        session(
            State(approval.clone()),
            Json(SessionRequest {
                proof: other.sign_device(ch)?,
                idempotency_key: id(),
            })
        )
        .await
        .err()
        .context("rejected account got a session")?
        .1,
        "CORDS_JOIN_REJECTED"
    );
    assert!(
        membership_requests(
            State(approval.clone()),
            headers(&owner_session)?,
            Query(MembershipPage::default())
        )
        .await?
        .0
        .is_empty()
    );
    let retry_challenge = join_challenge(
        State(approval.clone()),
        Json(ChallengeRequest {
            contact: contact.clone(),
            purpose: "join".into(),
            idempotency_key: id(),
        }),
    )
    .await?
    .0;
    assert_eq!(
        session(
            State(approval.clone()),
            Json(SessionRequest {
                proof: other.sign_device(retry_challenge)?,
                idempotency_key: id(),
            })
        )
        .await
        .err()
        .context("reapplication got a session")?
        .1,
        "CORDS_APPROVAL_PENDING"
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM memberships WHERE device_id=$1")
        .bind(&other.authorization.value.device_id)
        .fetch_one(approval.0.store.pool())
        .await?;
    assert_eq!(count, 0);
    let public = Service::with_policy(
        base.0.store.clone(),
        base.0.identity.clone(),
        60,
        900,
        JoinPolicy::Public,
    )?;
    let (public_account, public_contact) = identity()?;
    let issued = join(&public, &public_account, public_contact).await?;
    assert!(
        !issued
            .membership
            .value
            .capabilities
            .iter()
            .any(|v| v == "channel.create" || v == "server.manage")
    );
    Ok(())
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
    let sa = grant_fixture_owner(&s, join(&s, &a, ac).await?).await?;
    let sb = join(&s, &b, bc).await?;
    let route = create_channel(
        State(s.clone()),
        headers(&sa)?,
        Json(ChannelCreate {
            confidentiality_mode: Default::default(),
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
        public_message: None,
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
    let session = grant_fixture_owner(&s, join(&s, &a, contact).await?).await?;
    let route = create_channel(
        State(s.clone()),
        headers(&session)?,
        Json(ChannelCreate {
            confidentiality_mode: Default::default(),
            name: "rollback".into(),
            idempotency_key: id(),
        }),
    )
    .await?
    .0;
    let upload = EventUpload {
        public_message: None,
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
    let issued = grant_fixture_owner(&s, join(&s, &a, contact).await?).await?;
    let route = create_channel(
        State(s.clone()),
        headers(&issued)?,
        Json(ChannelCreate {
            confidentiality_mode: Default::default(),
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
