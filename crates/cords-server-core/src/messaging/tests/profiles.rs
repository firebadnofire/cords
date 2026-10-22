use super::*;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use cords_protocol::profiles::{
    AccountProfile, ProfileAvatar, ProfileField, ProfileGraph, ProfilePublish, ProfileRecord,
    ProfileRevision,
};

#[tokio::test]
#[ignore = "requires isolated PostgreSQL and TLS on the E2E host"]
#[allow(clippy::too_many_lines)]
async fn admission_picture_two_clients_before_approval_and_restart() -> Result<()> {
    use cords_client_core::client::Client;
    // Debug Wasmtime compilation is substantially slower than the production
    // build; initialize the sandbox before issuing time-limited device proofs.
    let bytes = include_bytes!("../../../../cords-avatar-codec/tests/fixtures/animated16.png.jxl");
    tokio::task::spawn_blocking(|| cords_avatar_codec::validate(bytes)).await??;
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
    let ca = root.path().join("ca.pem");
    std::fs::write(&ca, certificate.cert.pem())?;
    let migrations =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../migrations/sqlite");
    let pass = b"disposable admission profile test vault";
    let owner_path = root.path().join("owner");
    let applicant_path = root.path().join("applicant");
    let mut owner = Client::open(&owner_path, &migrations, Some(pass), Some(&ca)).await?;
    let mut applicant = Client::open(&applicant_path, &migrations, Some(pass), Some(&ca)).await?;
    owner.trust(&tls.origin).await?;
    owner.claim_ownership(&code).await?;
    // Save the account picture locally before requesting any membership.
    let avatar = ProfileAvatar {
        sha256: hash(bytes),
        bytes: u32::try_from(bytes.len())?,
        width: 1,
        height: 1,
        shape: "square".into(),
    };
    applicant
        .save_account_profile(
            AccountProfile {
                display_name: "Pending picture".into(),
                avatar: Some(avatar.clone()),
            },
            Some(STANDARD.encode(bytes)),
            vec![],
        )
        .await?;
    applicant.trust(&tls.origin).await?;
    assert_eq!(
        applicant.request_admission().await?.admission_state,
        "pending"
    );
    let requests = owner.membership_requests().await?;
    assert_eq!(requests.len(), 1);
    assert!(requests[0].has_profile);
    assert!(
        applicant
            .membership_request_profile(&requests[0].device_id, &requests[0].account_id)
            .await
            .is_err()
    );
    assert!(applicant.account_directory("").await.is_err());
    let profile = owner
        .membership_request_profile(&requests[0].device_id, &requests[0].account_id)
        .await?;
    assert_eq!(
        profile.image.as_deref(),
        Some(STANDARD.encode(bytes).as_str())
    );
    assert_eq!(
        profile
            .state
            .agreed
            .context("missing agreed profile")?
            .avatar,
        Some(avatar.clone())
    );
    let old_payload: String =
        sqlx::query_scalar("SELECT admission_profile FROM membership_requests WHERE device_id=$1")
            .bind(&requests[0].device_id)
            .fetch_one(service.0.store.pool())
            .await?;
    let mut updated_avatar = avatar.clone();
    updated_avatar.shape = "circle".into();
    let own = applicant.account_profile(&requests[0].account_id).await?;
    applicant
        .save_account_profile(
            AccountProfile {
                display_name: "Updated pending picture".into(),
                avatar: Some(updated_avatar.clone()),
            },
            None,
            own.state.heads,
        )
        .await?;
    applicant.request_admission().await?;
    assert_eq!(
        owner
            .membership_request_profile(&requests[0].device_id, &requests[0].account_id)
            .await?
            .state
            .agreed
            .context("missing updated profile")?
            .avatar,
        Some(updated_avatar.clone())
    );
    let current_payload: String =
        sqlx::query_scalar("SELECT admission_profile FROM membership_requests WHERE device_id=$1")
            .bind(&requests[0].device_id)
            .fetch_one(service.0.store.pool())
            .await?;
    sqlx::query("UPDATE membership_requests SET admission_profile=$2 WHERE device_id=$1")
        .bind(&requests[0].device_id)
        .bind(old_payload)
        .execute(service.0.store.pool())
        .await?;
    assert_eq!(
        owner
            .membership_request_profile(&requests[0].device_id, &requests[0].account_id)
            .await?
            .state
            .agreed
            .context("known profile was rolled back")?
            .avatar,
        Some(updated_avatar)
    );
    sqlx::query("UPDATE membership_requests SET admission_profile=$2 WHERE device_id=$1")
        .bind(&requests[0].device_id)
        .bind(current_payload)
        .execute(service.0.store.pool())
        .await?;
    let http = reqwest::Client::builder()
        .add_root_certificate(reqwest::Certificate::from_pem(
            certificate.cert.pem().as_bytes(),
        )?)
        .build()?;
    let response = http
        .get(format!(
            "{}/api/v1/membership-requests/{}/profile",
            tls.origin, requests[0].device_id
        ))
        .send()
        .await?;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    // A repeat is idempotent, and both request data and verified media survive reopening.
    applicant.request_admission().await?;
    assert_eq!(
        owner.membership_requests().await?[0].requested_at,
        requests[0].requested_at
    );
    owner.shutdown().await;
    applicant.shutdown().await;
    let mut owner = Client::open(&owner_path, &migrations, Some(pass), Some(&ca)).await?;
    let mut applicant = Client::open(&applicant_path, &migrations, Some(pass), Some(&ca)).await?;
    let request = owner.membership_requests().await?.remove(0);
    assert_eq!(
        owner
            .membership_request_profile(&request.device_id, &request.account_id)
            .await?
            .image,
        Some(STANDARD.encode(bytes))
    );
    assert_eq!(applicant.status().admission_state, "pending");
    // Server-side tampering is rejected independently by the receiving client.
    let stored: String =
        sqlx::query_scalar("SELECT admission_profile FROM membership_requests WHERE device_id=$1")
            .bind(&request.device_id)
            .fetch_one(service.0.store.pool())
            .await?;
    let mut forged: cords_protocol::profiles::AdmissionProfile = serde_json::from_str(&stored)?;
    forged.records[0].revision.value.profile.display_name = "Forged".into();
    sqlx::query("UPDATE membership_requests SET admission_profile=$2 WHERE device_id=$1")
        .bind(&request.device_id)
        .bind(json(&forged)?)
        .execute(service.0.store.pool())
        .await?;
    assert!(
        owner
            .membership_request_profile(&request.device_id, &request.account_id)
            .await
            .is_err()
    );
    sqlx::query("UPDATE membership_requests SET admission_profile=$2 WHERE device_id=$1")
        .bind(&request.device_id)
        .bind(stored)
        .execute(service.0.store.pool())
        .await?;
    owner.approve_membership(&request.device_id).await?;
    assert!(
        owner
            .membership_request_profile(&request.device_id, &request.account_id)
            .await
            .is_err()
    );
    applicant.synchronize().await?;
    assert_eq!(applicant.status().admission_state, "active");
    assert_eq!(
        owner.account_profile(&request.account_id).await?.image,
        Some(STANDARD.encode(bytes))
    );
    owner.shutdown().await;
    applicant.shutdown().await;
    Ok(())
}

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
