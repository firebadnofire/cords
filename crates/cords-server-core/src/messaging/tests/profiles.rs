use super::*;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use cords_protocol::profiles::{
    AccountProfile, ProfileAvatar, ProfileField, ProfileGraph, ProfilePublish, ProfileRecord,
    ProfileRevision,
};

#[tokio::test]
#[ignore = "requires isolated PostgreSQL and TLS"]
#[allow(clippy::too_many_lines)] // One two-server, offline/restart and revocation scenario.
async fn profiles_two_native_accounts_two_servers_and_sealed_restart() -> Result<()> {
    use cords_client_core::client::Client;
    let (first, _first_dir, first_code) = fixture_with_state(false).await?;
    let (second, _second_dir, second_code) = fixture_with_state(false).await?;
    let certificate = rcgen::generate_simple_self_signed(vec!["localhost".into()])?;
    let tls1 = https_fixture(&first, &certificate).await?;
    let tls2 = https_fixture(&second, &certificate).await?;
    let root = tempfile::tempdir()?;
    let ca = root.path().join("ca.pem");
    std::fs::write(&ca, certificate.cert.pem())?;
    let migrations =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../migrations/sqlite");
    let pass = b"disposable profile acceptance vault";
    let alice_dir = root.path().join("alice");
    let mut alice = Client::open(&alice_dir, &migrations, Some(pass), Some(&ca)).await?;
    let mut bob =
        Client::open(&root.path().join("bob"), &migrations, Some(pass), Some(&ca)).await?;
    alice.trust(&tls1.origin).await?;
    alice.claim_ownership(&first_code).await?;
    bob.trust(&tls1.origin).await?;
    bob.authenticate().await?;
    let bytes = include_bytes!("../../../../cords-avatar-codec/tests/fixtures/animated16.png.jxl");
    let avatar = ProfileAvatar {
        sha256: hash(bytes),
        bytes: u32::try_from(bytes.len())?,
        width: 1,
        height: 1,
        shape: "square".into(),
    };
    let account = alice.status().account_id;
    let view = alice
        .save_account_profile(
            AccountProfile {
                display_name: "Animated Alice".into(),
                avatar: Some(avatar.clone()),
            },
            Some(STANDARD.encode(bytes)),
            vec![],
        )
        .await?;
    assert!(
        view.pending_publication.is_none(),
        "{:?}",
        view.pending_publication
    );
    let remote = bob.account_profile(&account).await?;
    assert_eq!(
        remote
            .state
            .agreed
            .context("missing remote profile")?
            .avatar,
        Some(avatar.clone())
    );
    assert_eq!(
        remote.image.context("missing remote image")?,
        STANDARD.encode(bytes)
    );
    alice.shutdown().await;
    let mut alice = Client::open(&alice_dir, &migrations, Some(pass), Some(&ca)).await?;
    alice.trust(&tls2.origin).await?;
    alice.claim_ownership(&second_code).await?;
    let replicated = alice.account_profile(&account).await?;
    assert!(
        replicated.pending_publication.is_none(),
        "{:?}",
        replicated.pending_publication
    );
    bob.trust(&tls2.origin).await?;
    bob.authenticate().await?;
    let remote = bob.account_profile(&account).await?;
    assert_eq!(
        remote
            .state
            .agreed
            .context("missing second-server profile")?
            .display_name,
        "Animated Alice"
    );
    assert_eq!(
        remote.image.context("missing replicated image")?,
        STANDARD.encode(bytes)
    );
    let address = format!(
        "127.0.0.1:{}",
        tls2.origin
            .strip_prefix("https://localhost:")
            .context("unexpected fixture origin")?
            .parse::<u16>()?
    );
    drop(tls2);
    tokio::task::yield_now().await;
    let cached = alice.account_profile(&account).await?;
    assert!(cached.pending_publication.is_some());
    assert_eq!(
        alice
            .account_profile_image(&account, &avatar.sha256)
            .await?,
        STANDARD.encode(bytes)
    );
    let offline = alice
        .save_account_profile(
            AccountProfile {
                display_name: "Alice offline edit".into(),
                avatar: None,
            },
            None,
            cached.state.heads,
        )
        .await?;
    assert!(offline.pending_publication.is_some());
    alice.shutdown().await;
    let mut alice = Client::open(&alice_dir, &migrations, Some(pass), Some(&ca)).await?;
    let restarted = Service::with_policy(
        second.0.store.clone(),
        second.0.identity.clone(),
        60,
        900,
        JoinPolicy::Public,
    )?;
    let _tls2 = https_fixture_at(&restarted, &certificate, &address).await?;
    let retried = alice.account_profile(&account).await?;
    assert!(
        retried.pending_publication.is_none(),
        "{:?}",
        retried.pending_publication
    );
    let received = bob.account_profile(&account).await?;
    let profile = received.state.agreed.context("missing offline update")?;
    assert_eq!(profile.display_name, "Alice offline edit");
    assert!(profile.avatar.is_none());
    assert!(received.image.is_none());
    alice.revoke_device().await?;
    assert!(
        alice
            .save_account_profile(
                AccountProfile {
                    display_name: "revoked change".into(),
                    avatar: None
                },
                None,
                retried.state.heads
            )
            .await
            .is_err()
    );
    Ok(())
}

fn profile_record(
    identity: &Identity,
    parents: Vec<String>,
    generation: u64,
    name: &str,
    avatar: Option<ProfileAvatar>,
    changed: Vec<ProfileField>,
) -> Result<ProfileRecord> {
    Ok(ProfileRecord {
        authorization: identity.authorization.clone(),
        revision: identity.sign_device(ProfileRevision {
            version: 1,
            account_id: identity.authorization.value.account_id.clone(),
            device_id: identity.authorization.value.device_id.clone(),
            authorization_hash: hash(canonical(&identity.authorization)?),
            parents,
            generation,
            changed,
            profile: AccountProfile {
                display_name: name.into(),
                avatar,
            },
        })?,
    })
}

async fn publish_profile(
    s: &Service,
    session: &Session,
    record: ProfileRecord,
    image: Option<String>,
) -> Result<cords_protocol::profiles::ProfileReceipt> {
    let mut request = Request::builder()
        .method("POST")
        .uri("/api/v1/account-profiles")
        .body(Body::from(serde_json::to_vec(&ProfilePublish {
            record,
            image,
        })?))?;
    *request.headers_mut() = headers(session)?;
    Ok(super::super::profiles::publish(State(s.clone()), request)
        .await?
        .0)
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
#[allow(clippy::too_many_lines)] // Cross-account publication, restart and conflict scenario.
async fn profiles_publish_cross_account_animated_retrieval_restart_and_conflict() -> Result<()> {
    let (s, _directory) = fixture().await?;
    let (alice, contact) = identity()?;
    let alice_session = join(&s, &alice, contact).await?;
    let (bob, contact) = identity()?;
    let bob_session = join(&s, &bob, contact).await?;
    let bytes = include_bytes!("../../../../cords-avatar-codec/tests/fixtures/animated.gif.jxl");
    let avatar = ProfileAvatar {
        sha256: hash(bytes),
        bytes: u32::try_from(bytes.len())?,
        width: 1,
        height: 1,
        shape: "circle".into(),
    };
    let genesis = profile_record(
        &alice,
        vec![],
        0,
        "Alice",
        Some(avatar.clone()),
        vec![ProfileField::DisplayName, ProfileField::Avatar],
    )?;
    let first = publish_profile(
        &s,
        &alice_session,
        genesis.clone(),
        Some(STANDARD.encode(bytes)),
    )
    .await?;
    let retry = publish_profile(&s, &alice_session, genesis.clone(), None).await?;
    assert_eq!(first.id, retry.id);
    assert!(
        publish_profile(&s, &bob_session, genesis.clone(), None)
            .await
            .is_err()
    );
    let path = (
        alice.authorization.value.account_id.clone(),
        avatar.sha256.clone(),
    );
    let response =
        super::super::profiles::image(State(s.clone()), headers(&bob_session)?, Path(path.clone()))
            .await?;
    assert_eq!(&to_bytes(response.into_body(), 5_000_000).await?[..], bytes);
    assert!(
        super::super::profiles::image(State(s.clone()), HeaderMap::new(), Path(path.clone()))
            .await
            .is_err()
    );
    let left = profile_record(
        &alice,
        vec![first.id.clone()],
        1,
        "Alice left",
        Some(avatar.clone()),
        vec![ProfileField::DisplayName],
    )?;
    let right = profile_record(
        &alice,
        vec![first.id],
        1,
        "Alice right",
        Some(avatar.clone()),
        vec![ProfileField::DisplayName],
    )?;
    publish_profile(&s, &alice_session, left, None).await?;
    publish_profile(&s, &alice_session, right, None).await?;
    // Reconstruct the service over the same PostgreSQL and server identity.
    let s = Service::with_policy(
        s.0.store.clone(),
        s.0.identity.clone(),
        60,
        900,
        JoinPolicy::Public,
    )?;
    let page = super::super::profiles::history(
        State(s.clone()),
        headers(&bob_session)?,
        Path(path.0),
        Query(MembershipPage::default()),
    )
    .await?
    .0;
    assert_eq!(page.records.len(), 3);
    assert_eq!(page.heads.len(), 2);
    let mut graph = ProfileGraph::default();
    for record in page.records {
        graph.insert(record)?;
    }
    let state = graph.state()?;
    assert_eq!(
        state.agreed.context("missing agreed profile")?.display_name,
        "Alice"
    );
    assert_eq!(state.conflicts, vec![ProfileField::DisplayName]);
    let resolved = profile_record(
        &alice,
        page.heads,
        2,
        "Alice resolved",
        Some(avatar),
        vec![ProfileField::DisplayName],
    )?;
    publish_profile(&s, &alice_session, resolved, None).await?;
    sqlx::query("UPDATE sessions SET active=false WHERE device_id=$1")
        .bind(&alice.authorization.value.device_id)
        .execute(s.0.store.pool())
        .await?;
    assert!(
        publish_profile(&s, &alice_session, genesis, None)
            .await
            .is_err()
    );
    Ok(())
}
