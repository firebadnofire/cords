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
#[allow(clippy::too_many_lines)] // One complete channel-security acceptance scenario.
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
        public_message: Some(Box::new(payload.clone())),
    };
    let (unauthorized, _) = identity()?;
    let mut forged = upload.clone();
    forged.public_message = Some(Box::new(unauthorized.sign_device(payload.value.clone())?));
    assert!(
        upload_event(
            State(s.clone()),
            headers(&session)?,
            Path(route.clone()),
            Json(forged)
        )
        .await
        .is_err()
    );
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
    let mut invalid = request.clone();
    invalid.record.value.name = "tampered succession".into();
    assert!(
        replace_channel(
            State(s.clone()),
            headers(&session)?,
            Path(route.clone()),
            Json(invalid)
        )
        .await
        .is_err()
    );
    let mut stale = request.clone();
    let mut statement = stale.record.value.clone();
    statement.issued_at = now()?.saturating_sub(301);
    stale.record = owner.sign_device(statement)?;
    assert!(
        replace_channel(
            State(s.clone()),
            headers(&session)?,
            Path(route.clone()),
            Json(stale)
        )
        .await
        .is_err()
    );
    let initiating_contact = request.record.value.initiator.clone();
    let replay = request.clone();
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
    let mut replay = replay;
    replay.idempotency_key = id();
    assert!(
        replace_channel(
            State(s.clone()),
            headers(&session)?,
            Path(route.clone()),
            Json(replay)
        )
        .await
        .is_err()
    );
    let new = channel_view(&s, &successor).await?;
    assert!(
        new.transition
            .context("transition")?
            .value
            .predecessor_succession
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
    let predecessor_identity = channel_view(&s, &successor)
        .await?
        .identity
        .context("successor identity")?;
    let third = id();
    let chain = ChannelReplace {
        record: owner.sign_device(ChannelSuccession {
            version: 1,
            server_id: s.0.identity.server_id(),
            predecessor: predecessor_identity,
            successor_channel_id: third.clone(),
            name: "same name".into(),
            confidentiality_mode: ConfidentialityMode::Public,
            initiator: initiating_contact,
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
            Path(successor.clone()),
            Json(chain)
        )
        .await?
        .0,
        third
    );
    let middle = channel_view(&s, &successor)
        .await?
        .transition
        .context("middle channel state")?;
    assert!(
        middle.value.retired
            && middle.value.predecessor_succession.is_some()
            && middle.value.succession.is_some()
    );
    Ok(())
}

#[tokio::test]
#[ignore = "requires CORDS_TEST_DATABASE_URL; real TLS and PostgreSQL"]
#[allow(clippy::too_many_lines)] // One complete channel-security acceptance scenario.
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
        member
            .archived_channel_history(&server, &predecessor)
            .await?[0]
            .body,
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
    https_fixture_at(service, certificate, "127.0.0.1:0").await
}

async fn https_fixture_at(
    service: &Service,
    certificate: &rcgen::CertifiedKey<rcgen::KeyPair>,
    address: &str,
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
    let listener = tokio::net::TcpListener::bind(address).await?;
    let origin = format!("https://localhost:{}", listener.local_addr()?.port());
    let acceptor = TlsAcceptor::from(std::sync::Arc::new(config));
    let app = crate::router(
        crate::AppState::new(
            service
                .0
                .identity
                .signed_metadata_with_policy("TLS acceptance", "moderator_approval")?,
        )
        .with_identity(service.0.identity.clone())
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
                result=connections.join_next(), if !connections.is_empty() => {
                    match result.context("connection task missing")? {
                        Ok(Ok(())) => {},
                        Ok(Err(error)) => eprintln!("TLS fixture connection ended with error: {error:#}"),
                        Err(error) => return Err(error.into()),
                    }
                }
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
    let card = requests[0].user_card.as_ref().context("missing card")?;
    // The current native profile contract removed URL publication. Local PNG
    // previews must not leak through admission while canonical JXL is pending.
    assert!(card.value.avatar.is_none());
    let published = serde_json::to_string(card)?;
    assert!(!published.contains("images.example"));
    assert!(!published.contains("data:image/png"));
    let before_admission = owner.account_directory("").await?;
    assert_eq!(before_admission.accounts.len(), 1);
    assert!(member.account_directory("").await.is_err());
    owner.approve_membership(&member.status().device_id).await?;
    member.authenticate().await?;
    let admitted = member.account_directory("").await?;
    assert_eq!(admitted.accounts.len(), 2);
    assert!(admitted.next_after.is_none());
    assert_eq!(
        admitted
            .accounts
            .iter()
            .map(|entry| &entry.account_id)
            .collect::<Vec<_>>(),
        owner
            .account_directory("")
            .await?
            .accounts
            .iter()
            .map(|entry| &entry.account_id)
            .collect::<Vec<_>>()
    );
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
    let foreground = owner.status().server_id;
    let background = member
        .send(&route, "background encrypted acceptance marker")
        .await?;
    let synchronized = owner.synchronize_server(&pin1).await?;
    assert!(
        synchronized
            .messages
            .iter()
            .any(|item| item.message_id == background)
    );
    assert_eq!(owner.status().server_id, foreground);
    owner.reload().await?;
    assert_eq!(owner.status().server_id, foreground);
    owner.select_server(&pin1).await?;
    owner.authenticate().await?;
    assert_eq!(
        owner
            .history(&route)
            .await?
            .last()
            .context("missing original cache")?
            .body,
        "background encrypted acceptance marker"
    );
    assert!(member.remove_server(&pin1, false).await?.remote_confirmed);
    assert!(member.servers().is_empty());
    assert!(member.history(&route).await?.is_empty());
    assert_eq!(owner.account_directory("").await?.accounts.len(), 1);
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
                confidentiality_mode: ConfidentialityMode::Encrypted,
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
                confidentiality_mode: ConfidentialityMode::Encrypted,
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
#[ignore = "requires CORDS_TEST_DATABASE_URL; real PostgreSQL audit pagination"]
async fn role_audit_pages_are_bounded_contiguous_and_owner_authorized() -> Result<()> {
    use cords_protocol::roles::{Role, RoleChange, RoleChangeRequest};
    let (s, _directory, code) = fixture_with_state(false).await?;
    let (owner, contact) = identity()?;
    let session = claim_ownership(
        State(s.clone()),
        Json(ownership_request(&s, &owner, contact, &code).await?),
    )
    .await?
    .0;
    let role_id = id();
    for revision in 0..52 {
        let change = if revision == 0 {
            RoleChange::Initialize
        } else {
            RoleChange::PutRole {
                role: Role {
                    id: role_id.clone(),
                    name: format!("Audit fixture {revision}"),
                    color: "#4488aa".into(),
                    description: String::new(),
                    priority: 1,
                    permissions: std::collections::BTreeSet::new(),
                    mentionable: false,
                },
            }
        };
        let response = roles::change(
            State(s.clone()),
            headers(&session)?,
            Json(RoleChangeRequest {
                expected_revision: revision,
                request_id: id(),
                change,
            }),
        )
        .await?
        .0;
        assert_eq!(response.value.revision, u64::try_from(revision + 1)?);
    }
    for (after, expected) in [(0, 50), (50, 2), (52, 0)] {
        let page = roles::audit(
            State(s.clone()),
            Query(serde_json::from_value(serde_json::json!({"after":after}))?),
            headers(&session)?,
        )
        .await?
        .0;
        page.verify(&encode(s.0.identity.signing_key.verifying_key().to_bytes()))?;
        assert_eq!(page.value.head_revision, 52);
        assert_eq!(page.value.entries.len(), expected);
        assert_eq!(
            page.value
                .entries
                .iter()
                .map(|entry| entry.revision)
                .collect::<Vec<_>>(),
            ((after + 1)..=(after + u64::try_from(expected)?)).collect::<Vec<_>>()
        );
    }
    let invalid = roles::audit(
        State(s.clone()),
        Query(serde_json::from_value(serde_json::json!({"after":53}))?),
        headers(&session)?,
    )
    .await;
    assert!(matches!(invalid, Err(ApiError(StatusCode::BAD_REQUEST, _))));
    let (other, contact) = identity()?;
    let other = join(&s, &other, contact).await?;
    assert!(
        roles::audit(
            State(s.clone()),
            Query(roles::AuditQuery::default()),
            headers(&other)?
        )
        .await
        .is_err()
    );
    sqlx::query("UPDATE sessions SET active=false")
        .execute(s.0.store.pool())
        .await?;
    assert!(
        roles::audit(
            State(s),
            Query(roles::AuditQuery::default()),
            headers(&session)?
        )
        .await
        .is_err()
    );
    Ok(())
}

fn websocket_peer_closed(error: &tokio_tungstenite::tungstenite::Error) -> bool {
    use tokio_tungstenite::tungstenite;
    // Revocation requires transport termination, not a graceful close handshake.
    // Windows can reset a socket dropped while the client's Pong is in flight.
    matches!(error, tungstenite::Error::Io(error) if matches!(
            error.kind(),
            std::io::ErrorKind::ConnectionReset
                | std::io::ErrorKind::ConnectionAborted
                | std::io::ErrorKind::BrokenPipe
    ))
}

#[tokio::test]
#[ignore = "requires CORDS_TEST_DATABASE_URL; run explicitly against real PostgreSQL"]
#[allow(
    clippy::too_many_lines,
    reason = "Handshake, frame-limit and revocation checks share one live listener"
)]
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
        .await
        .context("sending oversized frame")?;
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
    closed
        .context("oversized socket did not close")?
        .context("observing oversized socket rejection")?;
    let (mut socket, _) = connect_async(request(Some(&session.token), Some(WS_PROTOCOL))?).await?;
    sqlx::query("UPDATE sessions SET active=false")
        .execute(s.0.store.pool())
        .await?;
    // A queued notification must recheck normal session authorization before delivery.
    s.0.notifications.send((id(), 1))?;
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match socket.next().await {
                Some(Ok(Message::Ping(_))) => match socket.flush().await {
                    Ok(()) => (),
                    Err(error) if websocket_peer_closed(&error) => break,
                    Err(error) => return Err(error.into()),
                },
                Some(Ok(Message::Close(_))) | None => break,
                Some(Err(error)) if websocket_peer_closed(&error) => break,
                Some(Err(error)) => return Err(error.into()),
                _ => anyhow::bail!("revoked session received an application notification"),
            }
        }
        Ok::<_, anyhow::Error>(())
    })
    .await
    .context("revoked socket did not close")?
    .context("observing revoked socket rejection")?;
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

/// Manual fixture only, deliberately excluded from the automated suite. No
/// production executable gains an unlock shortcut or acceptance-mode credential.
#[tokio::test]
#[ignore = "manual packaged desktop acceptance; requires explicit fixture directory and stop file"]
async fn desktop_role_acceptance_fixture() -> Result<()> {
    use cords_client_core::accounts::AccountRegistry;
    let root = std::path::PathBuf::from(
        std::env::var("CORDS_ROLE_DESKTOP_FIXTURE")
            .context("set CORDS_ROLE_DESKTOP_FIXTURE to a new disposable directory")?,
    );
    anyhow::ensure!(
        root.is_absolute() && !root.exists(),
        "fixture directory must be new and absolute"
    );
    std::fs::create_dir(&root)?;
    let (service, _server_directory, code) = fixture_with_state(false).await?;
    let certificate = rcgen::generate_simple_self_signed(vec!["localhost".into()])?;
    let tls = https_fixture(&service, &certificate).await?;
    let ca = root.join("fixture-ca.pem");
    std::fs::write(&ca, certificate.cert.pem())?;
    let migrations =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../migrations/sqlite");
    let registry = AccountRegistry::open(&root.join("vault"), &migrations, Some(&ca)).await?;
    // Public test-only password, never used for real accounts or deployed services.
    let mut owner = registry
        .create("Role acceptance", "Test-only Role Vault 9321!", false)
        .await?;
    owner.trust(&tls.origin).await?;
    owner.claim_ownership(&code).await?;
    let route = owner
        .create_channel_with_mode("Role acceptance", ConfidentialityMode::Public)
        .await?;
    seed_desktop_member(&registry, &mut owner, &tls.origin, &route).await?;
    if std::env::var("CORDS_ROLE_DESKTOP_SEED_AUDIT").as_deref() == Ok("1") {
        use cords_protocol::roles::{Role, RoleChange, RoleChangeRequest};
        println!("Seeding three authorized role changes for manual audit-view acceptance");
        let role_id = id();
        let changes = [
            RoleChange::Initialize,
            RoleChange::PutRole {
                role: Role {
                    id: role_id.clone(),
                    name: "GUI role".into(),
                    color: "#4488aa".into(),
                    description: String::new(),
                    priority: 1,
                    permissions: std::collections::BTreeSet::new(),
                    mentionable: false,
                },
            },
            RoleChange::AssignRole {
                account_id: owner.status().account_id,
                role_id,
            },
        ];
        for (revision, change) in changes.into_iter().enumerate() {
            owner
                .change_role_policy(&RoleChangeRequest {
                    expected_revision: i64::try_from(revision)?,
                    request_id: id(),
                    change,
                })
                .await?;
        }
    }
    owner.shutdown().await;
    std::fs::write(
        root.join("ready.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "origin":tls.origin,"state":root.join("vault"),"ca":ca,
            "server_id":service.0.identity.server_id()
        }))?,
    )?;
    println!("Packaged role fixture ready at {}", root.display());
    tokio::time::timeout(Duration::from_mins(30), async {
        while !root.join("stop").exists() {
            anyhow::ensure!(
                !tls.task.is_finished(),
                "manual fixture HTTPS listener stopped"
            );
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        Ok::<(), anyhow::Error>(())
    })
    .await
    .context("manual fixture timed out without completion")??;
    if std::env::var("CORDS_ROLE_DESKTOP_DEFAULTS_ONLY").as_deref() == Ok("1") {
        verify_untouched_role_defaults(&service).await?;
        return Ok(());
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM server_roles WHERE name='GUI role'")
        .fetch_one(service.0.store.pool())
        .await?;
    anyhow::ensure!(count == 1, "GUI role was not persisted exactly once");
    let assignments: i64 = sqlx::query_scalar("SELECT count(*) FROM account_roles a JOIN server_roles r ON r.id=a.role_id WHERE r.name='GUI role'")
        .fetch_one(service.0.store.pool()).await?;
    anyhow::ensure!(
        assignments == 1,
        "GUI role was not assigned to exactly one test account"
    );
    Ok(())
}

/// Explicit, disposable real-service fixture for native approval interaction tests.
/// No production binary gains test identities, passwords or protocol shortcuts.
#[tokio::test]
#[ignore = "manual native admission acceptance; requires a new fixture directory and stop file"]
async fn desktop_admission_acceptance_fixture() -> Result<()> {
    use cords_client_core::accounts::AccountRegistry;
    let root = std::path::PathBuf::from(std::env::var("CORDS_ROLE_DESKTOP_FIXTURE")?);
    anyhow::ensure!(
        root.is_absolute() && !root.exists(),
        "fixture directory must be new and absolute"
    );
    std::fs::create_dir(&root)?;
    let (base, _server, code) = fixture_with_state(false).await?;
    let service = Service::with_policy(
        base.0.store.clone(),
        base.0.identity.clone(),
        60,
        900,
        JoinPolicy::ModeratorApproval,
    )?;
    let certificate = rcgen::generate_simple_self_signed(vec!["localhost".into()])?;
    let tls = https_fixture(&service, &certificate).await?;
    let ca = root.join("fixture-ca.pem");
    std::fs::write(&ca, certificate.cert.pem())?;
    let migrations =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../migrations/sqlite");
    let owner_registry = AccountRegistry::open(&root.join("owner"), &migrations, Some(&ca)).await?;
    let applicant_registry =
        AccountRegistry::open(&root.join("applicant"), &migrations, Some(&ca)).await?;
    let password = "Native admission fixture only 7319!";
    let mut owner = owner_registry
        .create("Admission owner", password, false)
        .await?;
    owner.trust(&tls.origin).await?;
    owner.claim_ownership(&code).await?;
    let applicant = applicant_registry
        .create("Admission applicant", password, false)
        .await?;
    // Leave admission unsubmitted: the native Add Server command must create
    // the request. Pre-seeding pending state concealed a missing UI operation.
    let owner_id = owner.status().account_id;
    let applicant_id = applicant.status().account_id;
    owner.shutdown().await;
    applicant.shutdown().await;
    std::fs::write(
        root.join("ready.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "fixture": "native-admission-v1", "origin": tls.origin, "ca": ca,
            "owner_state": root.join("owner"), "applicant_state": root.join("applicant"),
            "owner_account": owner_id, "applicant_account": applicant_id
        }))?,
    )?;
    println!("Native admission fixture ready at {}", root.display());
    tokio::time::timeout(Duration::from_mins(30), async {
        while !root.join("stop").exists() {
            anyhow::ensure!(
                !tls.task.is_finished(),
                "admission fixture TLS listener stopped"
            );
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
        Ok::<(), anyhow::Error>(())
    })
    .await
    .context("admission fixture timed out")??;
    let mut applicant = applicant_registry.unlock(&applicant_id, password).await?;
    applicant.synchronize().await?;
    anyhow::ensure!(
        applicant.status().admission_state == "active",
        "native interaction did not approve applicant"
    );
    anyhow::ensure!(
        applicant.account_directory("").await?.accounts.len() == 2,
        "approved applicant directory did not contain both accounts"
    );
    applicant.shutdown().await;
    verify_untouched_role_defaults(&service).await?;
    Ok(())
}

async fn seed_desktop_member(
    registry: &cords_client_core::accounts::AccountRegistry,
    owner: &mut cords_client_core::client::Client,
    origin: &str,
    route: &str,
) -> Result<()> {
    let mut member = registry
        .create("Member acceptance", "Test-only Member Vault 5682!", false)
        .await?;
    member.trust(origin).await?;
    member.authenticate().await?;
    if std::env::var("CORDS_ROLE_DESKTOP_PROFILE_PICTURES").as_deref() == Ok("1") {
        seed_desktop_profile_pictures(owner, &mut member).await?;
    }
    owner.join_channel(route).await?;
    member.join_channel(route).await?;
    owner.send(route, "Packaged review: owner message").await?;
    member
        .send(route, "Packaged review: member message")
        .await?;
    member.shutdown().await;
    Ok(())
}

async fn seed_desktop_profile_pictures(
    owner: &mut cords_client_core::client::Client,
    member: &mut cords_client_core::client::Client,
) -> Result<()> {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use cords_protocol::profiles::{AccountProfile, ProfileAvatar};
    // Cold Wasmtime compilation is outside the HTTP request budget. Warm the
    // exact pinned decoder before this manual fixture publishes its first image.
    tokio::task::spawn_blocking(|| {
        cords_avatar_codec::validate(include_bytes!(
            "../../../cords-avatar-codec/tests/fixtures/looping-a.jxl"
        ))
    })
    .await??;
    for (client, name, shape, bytes) in [
        (
            owner,
            "Alice motion",
            "circle",
            include_bytes!("../../../cords-avatar-codec/tests/fixtures/looping-a.jxl").as_slice(),
        ),
        (
            member,
            "Bob motion",
            "square",
            include_bytes!("../../../cords-avatar-codec/tests/fixtures/looping-b.jxl").as_slice(),
        ),
    ] {
        let view = client
            .save_account_profile(
                AccountProfile {
                    display_name: name.into(),
                    avatar: Some(ProfileAvatar {
                        sha256: hash(bytes),
                        bytes: u32::try_from(bytes.len())?,
                        width: 64,
                        height: 64,
                        shape: shape.into(),
                    }),
                },
                Some(STANDARD.encode(bytes)),
                vec![],
            )
            .await?;
        anyhow::ensure!(
            view.pending_publication.is_none(),
            "Profile fixture publication pending: {:?}",
            view.pending_publication
        );
    }
    Ok(())
}

async fn verify_untouched_role_defaults(service: &Service) -> Result<()> {
    let mut tx = service.0.store.pool().begin().await?;
    let snapshot = crate::authorization::persistence::load(&mut tx).await?;
    anyhow::ensure!(
        snapshot.revision == 0 && !snapshot.active,
        "Opening Roles mutated policy"
    );
    let policy = snapshot.policy.context("Missing automatic defaults")?;
    anyhow::ensure!(policy.roles().len() == 3, "Expected three built-in roles");
    let changes: i64 = sqlx::query_scalar("SELECT count(*) FROM role_policy_changes")
        .fetch_one(&mut *tx)
        .await?;
    anyhow::ensure!(changes == 0, "Opening Roles submitted a setup operation");
    tx.rollback().await?;
    Ok(())
}

#[tokio::test]
#[ignore = "requires CORDS_TEST_DATABASE_URL; independent vaults over verified TLS"]
#[allow(
    clippy::too_many_lines,
    reason = "One uninterrupted two-vault and same-origin restart acceptance scenario"
)]
async fn role_catalog_owner_api_is_signed_persistent_and_does_not_activate_access() -> Result<()> {
    use cords_client_core::client::Client;
    use cords_protocol::roles::{Role, RoleChange, RoleChangeRequest};
    let (service, _directory, code) = fixture_with_state(false).await?;
    let certificate = rcgen::generate_simple_self_signed(vec!["localhost".into()])?;
    let mut tls = https_fixture(&service, &certificate).await?;
    let root = tempfile::tempdir()?;
    let ca = root.path().join("ca.pem");
    std::fs::write(&ca, certificate.cert.pem())?;
    let migrations =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../migrations/sqlite");
    let owner_path = root.path().join("owner");
    let mut owner = Client::open(
        &owner_path,
        &migrations,
        Some(b"role fixture owner"),
        Some(&ca),
    )
    .await?;
    let mut member = Client::open(
        &root.path().join("member"),
        &migrations,
        Some(b"role fixture member"),
        Some(&ca),
    )
    .await?;
    owner.trust(&tls.origin).await?;
    owner.claim_ownership(&code).await?;
    member.trust(&tls.origin).await?;
    member.authenticate().await?;
    assert!(member.role_catalog().await.is_err());
    let initial = owner.role_catalog().await?;
    assert_eq!(initial.revision, 0);
    assert!(!initial.active);
    assert_eq!(initial.roles.len(), 3);
    for role in crate::authorization::default_roles() {
        assert!(initial.roles.contains(&role));
    }
    let init = RoleChangeRequest {
        expected_revision: 0,
        request_id: id(),
        change: RoleChange::Initialize,
    };
    assert!(member.change_role_policy(&init).await.is_err());
    let receipt = owner.change_role_policy(&init).await?;
    assert_eq!(owner.change_role_policy(&init).await?, receipt);
    let role = Role {
        id: id(),
        name: "Developer team".into(),
        color: "#4488aa".into(),
        description: "Persisted through verified TLS".into(),
        priority: 10,
        permissions: std::collections::BTreeSet::new(),
        mentionable: false,
    };
    let create = RoleChangeRequest {
        expected_revision: 1,
        request_id: id(),
        change: RoleChange::PutRole { role: role.clone() },
    };
    assert_eq!(owner.change_role_policy(&create).await?.revision, 2);
    let stale = RoleChangeRequest {
        expected_revision: 1,
        request_id: id(),
        change: RoleChange::DeleteRole {
            role_id: role.id.clone(),
        },
    };
    assert!(owner.change_role_policy(&stale).await.is_err());
    let assign = RoleChangeRequest {
        expected_revision: 2,
        request_id: id(),
        change: RoleChange::AssignRole {
            account_id: member.status().account_id,
            role_id: cords_protocol::roles::ADMIN_ROLE.into(),
        },
    };
    let assignment_receipt = owner.change_role_policy(&assign).await?;
    let assigned = owner.account_roles(&member.status().account_id).await?;
    assert_eq!(assigned.catalog.revision, 3);
    assert_eq!(
        assigned.role_ids,
        [cords_protocol::roles::ADMIN_ROLE.to_owned()].into()
    );
    assert!(
        member
            .account_roles(&member.status().account_id)
            .await
            .is_err()
    );
    assert!(owner.account_roles("../role-policy").await.is_err());
    assert!(owner.account_roles("unknown-account").await.is_err());
    // An inactive catalog is not an authority grant, including after reauthentication.
    member.authenticate().await?;
    assert!(member.role_catalog().await.is_err());
    assert!(member.create_channel("must remain denied").await.is_err());
    // A lost acknowledgement is recoverable after the target loses membership.
    // Retrying is historical acknowledgement only, never a fresh grant.
    sqlx::query("UPDATE memberships SET active=false WHERE device_id IN (SELECT device_id FROM device_contacts WHERE account_id=$1)")
        .bind(member.status().account_id)
        .execute(service.0.store.pool()).await?;
    assert_eq!(owner.change_role_policy(&assign).await?, assignment_receipt);
    let mut substituted = assign.clone();
    substituted.change = RoleChange::AssignRole {
        account_id: member.status().account_id,
        role_id: role.id.clone(),
    };
    assert!(owner.change_role_policy(&substituted).await.is_err());
    substituted.request_id = id();
    substituted.expected_revision = 3;
    assert!(owner.change_role_policy(&substituted).await.is_err());
    let committed: i64 = sqlx::query_scalar("SELECT count(*) FROM role_policy_changes")
        .fetch_one(service.0.store.pool())
        .await?;
    assert_eq!(committed, 3);
    let audit = owner.role_audit(0).await?;
    assert_eq!(audit.entries.len(), 3);
    assert_eq!(audit.head_revision, 3);
    assert_eq!(audit.entries[2].operation, "assign_role");
    assert_eq!(audit.entries[2].actor_device_id, owner.status().device_id);
    let page = owner.role_audit(2).await?;
    assert_eq!(page.entries, audit.entries[2..]);
    assert!(owner.role_audit(3).await?.entries.is_empty());
    assert!(owner.role_audit(4).await.is_err());
    assert!(member.role_audit(0).await.is_err());
    owner.shutdown().await;
    let mut owner = Client::open(
        &owner_path,
        &migrations,
        Some(b"role fixture owner"),
        Some(&ca),
    )
    .await?;
    let catalog = owner.role_catalog().await?;
    assert_eq!(catalog.revision, 3);
    assert!(catalog.roles.contains(&role));
    assert!(!catalog.active);
    // Server service restart against the same database and identity retains exact policy.
    let restarted = Service::with_policy(
        service.0.store.clone(),
        service.0.identity.clone(),
        60,
        900,
        JoinPolicy::Public,
    )?;
    let address = format!(
        "127.0.0.1:{}",
        tls.origin.rsplit(':').next().context("TLS port missing")?
    );
    tls.task.abort();
    assert!(
        (&mut tls.task)
            .await
            .is_err_and(|error| error.is_cancelled())
    );
    let second_tls = https_fixture_at(&restarted, &certificate, &address).await?;
    assert_eq!(second_tls.origin, tls.origin);
    owner.authenticate().await?;
    assert_eq!(owner.role_catalog().await?, catalog);
    assert_eq!(owner.role_audit(0).await?, audit);
    assert_eq!(
        owner.account_roles(&member.status().account_id).await?,
        assigned
    );
    // Removal must remain available after departure; retries do not create new audit entries.
    let revoke = RoleChangeRequest {
        expected_revision: 3,
        request_id: id(),
        change: RoleChange::RevokeRole {
            account_id: member.status().account_id,
            role_id: cords_protocol::roles::ADMIN_ROLE.into(),
        },
    };
    let receipt = owner.change_role_policy(&revoke).await?;
    assert_eq!(receipt.revision, 4);
    assert_eq!(owner.change_role_policy(&revoke).await?, receipt);
    assert!(
        owner
            .account_roles(&member.status().account_id)
            .await?
            .role_ids
            .is_empty()
    );
    let removals = owner.role_audit(3).await?;
    assert_eq!(removals.entries.len(), 1);
    assert_eq!(removals.entries[0].operation, "revoke_role");
    let mut unknown = revoke;
    unknown.expected_revision = 4;
    unknown.request_id = id();
    unknown.change = RoleChange::RevokeRole {
        account_id: "unknown-account".into(),
        role_id: cords_protocol::roles::ADMIN_ROLE.into(),
    };
    assert!(owner.change_role_policy(&unknown).await.is_err());
    assert_eq!(owner.role_audit(3).await?, removals);
    Ok(())
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
            confidentiality_mode: ConfidentialityMode::Encrypted,
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
            confidentiality_mode: ConfidentialityMode::Encrypted,
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
            confidentiality_mode: ConfidentialityMode::Encrypted,
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
            confidentiality_mode: ConfidentialityMode::Encrypted,
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
            confidentiality_mode: ConfidentialityMode::Encrypted,
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
            confidentiality_mode: ConfidentialityMode::Encrypted,
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
            confidentiality_mode: ConfidentialityMode::Encrypted,
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
mod directory;
mod enforced_roles;
mod profiles;

#[tokio::test]
#[ignore = "requires CORDS_TEST_DATABASE_URL; real TLS and PostgreSQL"]
async fn admission_status_survives_restart_and_sync() -> Result<()> {
    use cords_client_core::client::Client;
    let (base, _server, code) = fixture_with_state(false).await?;
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
    let pass = b"independent admission test vault";
    let mut owner = Client::open(
        &root.path().join("owner"),
        &migrations,
        Some(pass),
        Some(&ca),
    )
    .await?;
    let member_path = root.path().join("member");
    let mut member = Client::open(&member_path, &migrations, Some(pass), Some(&ca)).await?;
    owner.trust(&tls.origin).await?;
    owner.claim_ownership(&code).await?;
    member.trust(&tls.origin).await?;
    assert_eq!(member.request_admission().await?.admission_state, "pending");
    let first = owner.membership_requests().await?;
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].account_id, member.status().account_id);
    assert!(member.membership_requests().await.is_err());
    assert!(member.account_directory("").await.is_err());
    assert_eq!(member.request_admission().await?.admission_state, "pending");
    let duplicate = owner.membership_requests().await?;
    assert_eq!(duplicate.len(), 1);
    assert_eq!(duplicate[0].requested_at, first[0].requested_at);
    drop(member);
    let mut member = Client::open(&member_path, &migrations, Some(pass), Some(&ca)).await?;
    assert_eq!(member.status().admission_state, "pending");
    owner.reject_membership(&member.status().device_id).await?;
    member.synchronize().await?;
    assert_eq!(member.status().admission_state, "rejected");
    drop(member);
    let mut member = Client::open(&member_path, &migrations, Some(pass), Some(&ca)).await?;
    assert_eq!(member.status().admission_state, "rejected");
    member.synchronize().await?;
    assert_eq!(member.status().admission_state, "rejected");
    assert_eq!(member.request_admission().await?.admission_state, "pending");
    owner.approve_membership(&member.status().device_id).await?;
    member.synchronize().await?;
    assert_eq!(member.status().admission_state, "active");
    assert_eq!(member.account_directory("").await?.accounts.len(), 2);
    assert!(owner.membership_requests().await?.is_empty());
    assert!(member.membership_requests().await.is_err());
    assert!(
        member
            .approve_membership(&owner.status().device_id)
            .await
            .is_err()
    );
    drop(member);
    let mut member = Client::open(&member_path, &migrations, Some(pass), Some(&ca)).await?;
    assert_eq!(member.status().admission_state, "active");
    member.synchronize().await?;
    drop(tls);
    assert!(
        member.request_admission().await.is_err(),
        "network failure must remain an error"
    );
    Ok(())
}

#[tokio::test]
#[ignore = "requires CORDS_TEST_DATABASE_URL; real desktop workers, TLS and PostgreSQL"]
async fn native_desktop_administrator_can_review_real_requests() -> Result<()> {
    let (base, _server, code) = fixture_with_state(false).await?;
    let service = Service::with_policy(
        base.0.store.clone(),
        base.0.identity.clone(),
        60,
        900,
        JoinPolicy::ModeratorApproval,
    )?;
    let certificate = rcgen::generate_simple_self_signed(vec!["localhost".into()])?;
    let tls = https_fixture(&service, &certificate).await?;
    let origin = tls.origin.clone();
    let root = tempfile::tempdir()?;
    let ca = root.path().join("test-ca.pem");
    std::fs::write(&ca, certificate.cert.pem())?;
    tokio::task::spawn_blocking(move || -> Result<()> {
        use cords_qt_bridge::{Desktop, ffi::Snapshot, start_desktop};
        fn wait(desktop: &Desktop, completed: u64) -> Result<Snapshot> {
            let deadline = std::time::Instant::now() + Duration::from_secs(40);
            while std::time::Instant::now() < deadline {
                for view in desktop.take_updates()? {
                    if view.completed >= completed {
                        anyhow::ensure!(
                            view.error.is_empty(),
                            "Desktop operation failed: {}",
                            view.error
                        );
                        return Ok(view);
                    }
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            anyhow::bail!("Desktop operation timed out")
        }
        let migrations =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../migrations/sqlite");
        let open = |name: &str| {
            start_desktop(
                root.path().join(name).to_string_lossy().into(),
                migrations.to_string_lossy().into(),
                ca.to_string_lossy().into(),
            )
        };
        let owner = open("owner")?;
        let member = open("member")?;
        wait(&owner, 0)?;
        wait(&member, 0)?;
        let password = "Native network acceptance passphrase 42";
        owner.create_account("Owner".into(), password.into(), false)?;
        wait(&owner, 1)?;
        member.create_account("Applicant".into(), password.into(), false)?;
        let identity = wait(&member, 1)?.account;
        owner.trust_server(origin.clone())?;
        wait(&owner, 2)?;
        owner.claim_ownership(code.to_string())?;
        let claimed = wait(&owner, 3)?;
        assert!(
            claimed.can_approve && claimed.can_roles && claimed.can_assign && claimed.can_audit
        );
        member.trust_server(origin)?;
        wait(&member, 2)?;
        member.authenticate()?;
        let pending = wait(&member, 3)?;
        assert_eq!(pending.admission, "pending");
        assert!(!pending.can_approve && !pending.connected);
        owner.load_requests()?;
        let requests = wait(&owner, 4)?;
        assert!(requests.requests_loaded);
        assert_eq!(requests.requests.len(), 1);
        assert_eq!(requests.requests[0].account, identity);
        assert_eq!(requests.requests[0].name, "Applicant");
        assert!(requests.requests[0].requested_at > 0);
        owner.approve_request(requests.requests[0].id.clone())?;
        assert!(wait(&owner, 5)?.requests.is_empty());
        member.synchronize()?;
        assert_eq!(wait(&member, 4)?.admission, "active");
        member.load_directory(String::new())?;
        assert_eq!(wait(&member, 5)?.members.len(), 2);
        owner.load_server_profile()?;
        let profile = wait(&owner, 6)?.server_profile;
        assert!(profile.supported && profile.loaded && profile.can_edit);
        owner.save_server_profile(
            "Native community".into(),
            "Edited through the typed desktop bridge".into(),
            "#521b60".into(),
            profile.revision,
            uuid::Uuid::now_v7().to_string(),
        )?;
        let saved = wait(&owner, 7)?;
        assert_eq!(saved.server_profile.name, "Native community");
        assert_eq!(saved.servers[0].name, "Native community");
        member.load_server_profile()?;
        let observed = wait(&member, 6)?;
        assert_eq!(observed.server_profile.name, "Native community");
        assert_eq!(observed.server_profile.background, "#521b60");
        assert!(!observed.server_profile.can_edit);
        drop(member);
        let member = open("member")?;
        wait(&member, 0)?;
        member.unlock_account(identity.clone(), password.into())?;
        let restored = wait(&member, 1)?;
        assert_eq!(restored.admission, "active");
        assert!(!restored.can_approve);
        assert_eq!(restored.server_profile.name, "Native community");
        assert_eq!(restored.server_profile.background, "#521b60");
        owner.prepare_server_icon(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../bins/cords-client/qt/assets/128x128.png")
                .to_string_lossy()
                .into(),
        )?;
        let draft = wait(&owner, 8)?.server_icon_draft;
        assert!(!draft.id.is_empty());
        owner.save_server_icon(draft.id, 0.5, 0.5, 1.0, "circle".into())?;
        let branded = wait(&owner, 9)?;
        assert!(branded.server_profile.has_icon);
        assert!(
            branded
                .server_profile
                .image
                .starts_with("data:image/jxl;base64,")
        );
        assert_eq!(branded.servers[0].image, branded.server_profile.image);
        member.load_server_profile()?;
        assert_eq!(
            wait(&member, 2)?.server_profile.image,
            branded.server_profile.image
        );
        drop(member);
        let member = open("member")?;
        wait(&member, 0)?;
        member.unlock_account(identity, password.into())?;
        assert_eq!(
            wait(&member, 1)?.server_profile.image,
            branded.server_profile.image
        );
        owner.remove_server_icon(
            branded.server_profile.revision,
            uuid::Uuid::now_v7().to_string(),
        )?;
        assert!(!wait(&owner, 10)?.server_profile.has_icon);
        member.load_server_profile()?;
        let removed = wait(&member, 2)?;
        assert!(
            !removed.server_profile.has_icon
                && removed.server_profile.image.is_empty()
                && removed.servers[0].image.is_empty()
        );
        Ok(())
    })
    .await??;
    Ok(())
}

#[tokio::test]
#[ignore = "requires CORDS_TEST_DATABASE_URL; real TLS and PostgreSQL"]
async fn server_profile_revisions_authorization_and_discovery() -> Result<()> {
    use cords_client_core::client::Client;
    use cords_protocol::server_profile::ServerProfileChange;
    let (base, _server, code) = fixture_with_state(false).await?;
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
    let password = b"Independent server presentation vault";
    let owner_path = root.path().join("owner");
    let mut owner = Client::open(&owner_path, &migrations, Some(password), Some(&ca)).await?;
    let mut member = Client::open(
        &root.path().join("member"),
        &migrations,
        Some(password),
        Some(&ca),
    )
    .await?;
    owner.trust(&tls.origin).await?;
    owner.claim_ownership(&code).await?;
    member.trust(&tls.origin).await?;
    member.request_admission().await?;
    owner.approve_membership(&member.status().device_id).await?;
    member.synchronize().await?;
    assert_eq!(owner.server_profile().await?.revision, 0);
    let change = ServerProfileChange {
        request_id: uuid::Uuid::now_v7().to_string(),
        expected_revision: 0,
        name: "A real community".into(),
        description: "A durable description.\nSecond line.".into(),
        background: "#20485a".into(),
    };
    let saved = owner.save_server_profile(change.clone()).await?;
    assert_eq!(saved.background, "#20485a");
    assert_eq!(saved.revision, 1);
    assert_eq!(owner.save_server_profile(change.clone()).await?, saved);
    let mut stale = change.clone();
    stale.request_id = uuid::Uuid::now_v7().to_string();
    assert!(owner.save_server_profile(stale).await.is_err());
    let mut collision = change.clone();
    collision.name = "Another name".into();
    assert!(owner.save_server_profile(collision).await.is_err());
    let mut unauthorized = change.clone();
    unauthorized.request_id = uuid::Uuid::now_v7().to_string();
    unauthorized.expected_revision = 1;
    assert!(member.save_server_profile(unauthorized).await.is_err());
    assert_eq!(member.server_profile().await?, saved);
    assert_eq!(member.server_name(&member.status().server_id), saved.name);
    let http = reqwest::Client::builder()
        .add_root_certificate(reqwest::Certificate::from_pem(
            certificate.cert.pem().as_bytes(),
        )?)
        .build()?;
    let metadata: cords_protocol::SignedServerMetadataV1 = http
        .get(format!("{}/.well-known/cords/server", tls.origin))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    metadata.metadata.verify(&metadata.signature)?;
    assert_eq!(metadata.metadata.server_name, saved.name);
    assert_eq!(metadata.metadata.join_policy, vec!["moderator_approval"]);
    let audits: i64 = sqlx::query_scalar("SELECT count(*) FROM server_profile_audit")
        .fetch_one(service.0.store.pool())
        .await?;
    assert_eq!(audits, 1);
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use cords_protocol::{profiles::ProfileAvatar, server_profile::ServerIconChange};
    let image = include_bytes!("../../../cords-avatar-codec/tests/fixtures/looping-a.jxl");
    let info = cords_avatar_codec::validate(image)?;
    assert_eq!(info.width, info.height);
    let change = ServerIconChange {
        request_id: uuid::Uuid::now_v7().to_string(),
        expected_revision: saved.revision,
        icon: Some(ProfileAvatar {
            sha256: hash(image),
            bytes: u32::try_from(image.len())?,
            width: u16::try_from(info.width)?,
            height: u16::try_from(info.height)?,
            shape: "circle".into(),
        }),
        image: Some(STANDARD.encode(image)),
    };
    assert!(member.save_server_icon(change.clone()).await.is_err());
    let mut corrupt = change.clone();
    corrupt.image = Some(STANDARD.encode(b"not an image"));
    if let Some(icon) = corrupt.icon.as_mut() {
        icon.sha256 = hash(b"not an image");
        icon.bytes = 12;
    }
    assert!(owner.save_server_icon(corrupt).await.is_err());
    let saved = owner.save_server_icon(change.clone()).await?;
    assert_eq!(owner.save_server_icon(change.clone()).await?, saved);
    let mut stale_icon = change;
    stale_icon.request_id = uuid::Uuid::now_v7().to_string();
    assert!(owner.save_server_icon(stale_icon).await.is_err());
    assert_eq!(member.server_profile().await?, saved);
    assert_eq!(
        member
            .cached_server_icon(&saved.server_id)
            .await?
            .context("Missing cached icon")?
            .0,
        STANDARD.encode(image)
    );
    drop(owner);
    let mut owner = Client::open(&owner_path, &migrations, Some(password), Some(&ca)).await?;
    assert_eq!(owner.cached_server_profile(), Some(&saved));
    let port = tls
        .origin
        .rsplit(':')
        .next()
        .context("missing port")?
        .to_owned();
    drop(tls);
    tokio::time::sleep(Duration::from_millis(50)).await;
    let restarted = Service::with_policy(
        base.0.store.clone(),
        base.0.identity.clone(),
        60,
        900,
        JoinPolicy::ModeratorApproval,
    )?;
    let _tls = https_fixture_at(&restarted, &certificate, &format!("127.0.0.1:{port}")).await?;
    assert_eq!(owner.server_profile().await?, saved);
    let removed = owner
        .save_server_icon(ServerIconChange {
            request_id: uuid::Uuid::now_v7().to_string(),
            expected_revision: saved.revision,
            icon: None,
            image: None,
        })
        .await?;
    assert!(removed.icon.is_none());
    member.server_profile().await?;
    assert!(member.cached_server_icon(&saved.server_id).await?.is_none());
    let bytes: Option<Vec<u8>> =
        sqlx::query_scalar("SELECT icon_image FROM server_profile WHERE singleton=TRUE")
            .fetch_one(base.0.store.pool())
            .await?;
    assert!(bytes.is_none());
    Ok(())
}

#[tokio::test]
#[ignore = "requires CORDS_TEST_DATABASE_URL; real concurrent PostgreSQL transactions"]
async fn approval_does_not_deadlock_with_pending_status_refresh() -> Result<()> {
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
    let (applicant, mut contact) = identity()?;
    assert!(join(&s, &applicant, contact.clone()).await.is_err());
    let device = applicant.authorization.value.device_id.clone();
    // A pending status refresh holds the account lock while persisting its contact,
    // then locks the admission row. Approval must take those locks in the same order.
    let mut refreshing = s.0.store.pool().begin().await?;
    let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *refreshing)
        .await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(&applicant.authorization.value.account_id)
        .execute(&mut *refreshing)
        .await?;
    let service = s.clone();
    let target = device.clone();
    let actor_headers = headers(&owner_session)?;
    let approving = tokio::spawn(async move {
        approve_membership(State(service), actor_headers, Path(target)).await
    });
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let blocked: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE $1=ANY(pg_blocking_pids(pid)))",
        )
        .bind(pid)
        .fetch_one(s.0.store.pool())
        .await?;
        if blocked {
            break;
        }
        anyhow::ensure!(
            tokio::time::Instant::now() < deadline,
            "Approval did not reach the held account lock"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    contact.user_card = Some(applicant.sign_device(cords_protocol::messaging::UserCard {
        version: 1,
        account_id: applicant.authorization.value.account_id.clone(),
        device_id: device.clone(),
        nickname: "Updated while waiting".into(),
        avatar: None,
        issued_at: now()?,
    })?);
    tokio::time::timeout(
        Duration::from_secs(5),
        sqlx::query("UPDATE membership_requests SET contact=$2 WHERE device_id=$1")
            .bind(&device)
            .bind(json(&contact)?)
            .execute(&mut *refreshing),
    )
    .await
    .context("Pending status refresh blocked behind approval")??;
    refreshing.commit().await?;
    let approved = tokio::time::timeout(Duration::from_secs(10), approving)
        .await???
        .0;
    assert_eq!(approved.value.device_id, device);
    let persisted: String =
        sqlx::query_scalar("SELECT contact FROM device_contacts WHERE device_id=$1")
            .bind(&device)
            .fetch_one(s.0.store.pool())
            .await?;
    assert_eq!(
        parse::<Contact>(&persisted)?
            .user_card
            .context("Missing refreshed card")?
            .value
            .nickname,
        "Updated while waiting"
    );
    let status: String =
        sqlx::query_scalar("SELECT status FROM membership_requests WHERE device_id=$1")
            .bind(device)
            .fetch_one(s.0.store.pool())
            .await?;
    assert_eq!(status, "approved");
    Ok(())
}
