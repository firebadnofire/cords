//! Current-policy enforcement, upgrade and native-client acceptance.
use super::*;
use cords_protocol::roles::{Permission, Role, RoleChange, RoleChangeRequest};

async fn remove_default_permission(
    service: &Service,
    owner: &Session,
    permission: Permission,
) -> Result<()> {
    let catalog = roles::catalog(State(service.clone()), headers(owner)?)
        .await?
        .0
        .value;
    let mut role = catalog
        .roles
        .into_iter()
        .find(|role| role.id == cords_protocol::roles::DEFAULT_ROLE)
        .context("default role")?;
    assert!(role.permissions.remove(&permission));
    change(
        service,
        owner,
        request(
            i64::try_from(catalog.revision)?,
            RoleChange::PutRole { role },
        ),
    )
    .await?;
    Ok(())
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn contact_and_key_package_operations_observe_current_policy() -> Result<()> {
    let (service, _directory) = fixture().await?;
    let (owner_id, owner_contact) = identity()?;
    let owner =
        grant_fixture_owner(&service, join(&service, &owner_id, owner_contact).await?).await?;
    let member_id = Identity::generate(now()?)?;
    let crypto = ConversationCrypto::new(member_id.authorization.value.device_id.clone())?;
    let member = join(
        &service,
        &member_id,
        member_id.contact(&crypto.public_key())?,
    )
    .await?;
    activate_candidate(&service).await?;
    let package = KeyPackageUpload {
        key_package: encode(crypto.key_package()?),
        idempotency_key: id(),
    };
    let published = upload_package(
        State(service.clone()),
        headers(&member)?,
        Json(package.clone()),
    )
    .await?
    .0;
    assert_eq!(published, hash(decode(&package.key_package)?));
    assert_eq!(
        packages(
            State(service.clone()),
            headers(&member)?,
            Path(member.membership.value.account_id.clone())
        )
        .await?
        .0
        .len(),
        1
    );
    assert_eq!(
        members(
            State(service.clone()),
            headers(&member)?,
            Query(MembershipPage::default())
        )
        .await?
        .0
        .len(),
        2
    );
    remove_default_permission(&service, &owner, Permission::PublishKeyPackage).await?;
    // An old credential and a known successful idempotency key cannot bypass a
    // permission revoked after publication. No additional package is persisted.
    assert_eq!(
        upload_package(State(service.clone()), headers(&member)?, Json(package))
            .await
            .err()
            .context("publication denial")?
            .0,
        StatusCode::FORBIDDEN
    );
    remove_default_permission(&service, &owner, Permission::ViewServer).await?;
    assert!(
        super::super::directory::accounts(
            State(service.clone()),
            headers(&member)?,
            Query(MembershipPage::default())
        )
        .await
        .is_err()
    );
    assert_eq!(
        members(
            State(service.clone()),
            headers(&member)?,
            Query(MembershipPage::default())
        )
        .await
        .err()
        .context("contact denial")?
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        packages(
            State(service.clone()),
            headers(&member)?,
            Path(owner.membership.value.account_id.clone())
        )
        .await
        .err()
        .context("package disclosure denial")?
        .0,
        StatusCode::FORBIDDEN
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM key_packages")
        .fetch_one(service.0.store.pool())
        .await?;
    assert_eq!(count, 1);
    Ok(())
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL and real TLS WebSocket"]
async fn live_tls_notifications_recheck_channel_policy() -> Result<()> {
    use futures_util::{SinkExt as _, StreamExt as _};
    use tokio_rustls::{TlsConnector, rustls};
    use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest as _};

    let (service, _directory) = fixture().await?;
    let (owner_id, owner_contact) = identity()?;
    let owner =
        grant_fixture_owner(&service, join(&service, &owner_id, owner_contact).await?).await?;
    let (member_id, member_contact) = identity()?;
    let member = join(&service, &member_id, member_contact).await?;
    let mut routes = Vec::new();
    for name in ["access removed", "access retained"] {
        routes.push(
            create_channel(
                State(service.clone()),
                headers(&owner)?,
                Json(ChannelCreate {
                    confidentiality_mode: ConfidentialityMode::Public,
                    name: name.into(),
                    idempotency_key: id(),
                }),
            )
            .await?
            .0,
        );
    }
    activate_candidate(&service).await?;
    let certificate = rcgen::generate_simple_self_signed(vec!["localhost".into()])?;
    let tls = https_fixture(&service, &certificate).await?;
    let mut roots = rustls::RootCertStore::empty();
    roots.add(certificate.cert.der().clone())?;
    let config = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_protocol_versions(&[&rustls::version::TLS13])?
    .with_root_certificates(roots)
    .with_no_client_auth();
    let stream =
        tokio::net::TcpStream::connect(tls.origin.strip_prefix("https://").context("TLS origin")?)
            .await?;
    let stream = TlsConnector::from(Arc::new(config))
        .connect(
            rustls::pki_types::ServerName::try_from("localhost")?,
            stream,
        )
        .await?;
    let mut request = format!(
        "{}/api/v1/events",
        tls.origin.replacen("https://", "wss://", 1)
    )
    .into_client_request()?;
    request
        .headers_mut()
        .insert("authorization", format!("Bearer {}", member.token).parse()?);
    request
        .headers_mut()
        .insert("sec-websocket-protocol", WS_PROTOCOL.parse()?);
    let (mut socket, _) = tokio_tungstenite::client_async(request, stream).await?;
    // The second allowed route is an ordering barrier: a denied route must not
    // emit a notice, while the same established connection remains useful.
    for (denied_route, expected_route, sequence) in [(false, &routes[0], 1), (true, &routes[1], 3)]
    {
        if denied_route {
            deny_channel(&service, &routes[0], Permission::ReadMessages).await?;
            service.0.notifications.send((routes[0].clone(), 2))?;
        }
        service
            .0
            .notifications
            .send((expected_route.clone(), sequence))?;
        let received = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                match socket.next().await {
                    Some(Ok(Message::Binary(bytes))) => return Ok::<_, anyhow::Error>(bytes),
                    Some(Ok(Message::Ping(_))) => socket.flush().await?,
                    other => anyhow::bail!("expected authenticated notice, got {other:?}"),
                }
            }
        })
        .await??;
        assert_eq!(
            received.as_ref(),
            notification(expected_route, sequence)?.as_slice()
        );
    }
    socket.close(None).await?;
    Ok(())
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn discovery_channel_snapshots_and_welcomes_observe_current_policy() -> Result<()> {
    let (service, _directory) = fixture().await?;
    let (owner_id, owner_contact) = identity()?;
    let owner =
        grant_fixture_owner(&service, join(&service, &owner_id, owner_contact).await?).await?;
    let (member_id, member_contact) = identity()?;
    let member = join(&service, &member_id, member_contact).await?;
    let route = create_channel(
        State(service.clone()),
        headers(&owner)?,
        Json(ChannelCreate {
            confidentiality_mode: ConfidentialityMode::Encrypted,
            name: "policy discovery".into(),
            idempotency_key: id(),
        }),
    )
    .await?
    .0;
    sqlx::query("INSERT INTO channel_members(channel_id,device_id) VALUES($1,$2)")
        .bind(&route)
        .bind(&member_id.authorization.value.device_id)
        .execute(service.0.store.pool())
        .await?;
    sqlx::query("INSERT INTO welcomes(channel_id,device_id,sequence,welcome) VALUES($1,$2,0,'opaque welcome')")
        .bind(&route).bind(&member_id.authorization.value.device_id)
        .execute(service.0.store.pool()).await?;
    let mut tx = service.0.store.pool().begin().await?;
    lifecycle::snapshot_epoch(&mut tx, &route).await?;
    tx.commit().await?;
    activate_candidate(&service).await?;

    assert_eq!(
        channels(State(service.clone()), headers(&member)?)
            .await?
            .0
            .len(),
        1
    );
    assert!(
        welcome(
            State(service.clone()),
            headers(&member)?,
            Path(route.clone())
        )
        .await
        .is_ok()
    );
    deny_channel(&service, &route, Permission::ReadMessages).await?;
    let discovered = channels(State(service.clone()), headers(&member)?).await?.0;
    assert!(
        discovered.is_empty(),
        "read-denied channels must be absent from navigation"
    );
    for epoch in [None, Some(0)] {
        let denied = channel(
            State(service.clone()),
            headers(&member)?,
            Path(route.clone()),
            Query(ChannelQuery { epoch }),
        )
        .await;
        assert_eq!(
            denied
                .err()
                .context("read-denied channel state disclosed")?
                .0,
            StatusCode::FORBIDDEN
        );
    }
    let denied = welcome(
        State(service.clone()),
        headers(&member)?,
        Path(route.clone()),
    )
    .await;
    assert_eq!(
        denied.err().context("read-denied Welcome disclosed")?.0,
        StatusCode::FORBIDDEN
    );
    deny_channel(&service, &route, Permission::ViewChannel).await?;
    assert!(
        channels(State(service.clone()), headers(&member)?)
            .await?
            .0
            .is_empty()
    );
    assert_eq!(
        channels(State(service.clone()), headers(&owner)?)
            .await?
            .0
            .len(),
        1,
        "independent owner management remains visible"
    );
    Ok(())
}

async fn activate_candidate(service: &Service) -> Result<()> {
    let mut tx = service.0.store.pool().begin().await?;
    sqlx::raw_sql(include_str!(
        "../../../../../migrations/postgres/202610210005_enforced_rbac.sql"
    ))
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    crate::authorization::migration::prepare(
        &service.0.store,
        &service.0.identity.server_id(),
        &encode(service.0.identity.signing_key.verifying_key().to_bytes()),
        now()?,
    )
    .await?;
    sqlx::query("UPDATE role_policy SET active=TRUE WHERE singleton=TRUE")
        .execute(service.0.store.pool())
        .await?;
    Ok(())
}

fn request(revision: i64, change: RoleChange) -> RoleChangeRequest {
    RoleChangeRequest {
        expected_revision: revision,
        request_id: id(),
        change,
    }
}

async fn change(service: &Service, session: &Session, request: RoleChangeRequest) -> Result<u64> {
    let receipt = roles::change(State(service.clone()), headers(session)?, Json(request))
        .await?
        .0;
    receipt.verify(&encode(
        service.0.identity.signing_key.verifying_key().to_bytes(),
    ))?;
    Ok(receipt.value.revision)
}

async fn establish_delegation(
    service: &Service,
    owner: &Session,
    delegate_id: &Identity,
) -> Result<(Role, Role)> {
    let delegator = Role {
        id: id(),
        name: "assigner".into(),
        color: "#123456".into(),
        description: String::new(),
        priority: 150,
        permissions: [Permission::AssignRoles, Permission::ViewAudit].into(),
        mentionable: false,
    };
    let member_role = Role {
        id: id(),
        name: "member marker".into(),
        color: "#123456".into(),
        description: String::new(),
        priority: 10,
        permissions: std::collections::BTreeSet::new(),
        mentionable: false,
    };
    change(
        service,
        owner,
        request(
            0,
            RoleChange::PutRole {
                role: delegator.clone(),
            },
        ),
    )
    .await?;
    change(
        service,
        owner,
        request(
            1,
            RoleChange::PutRole {
                role: member_role.clone(),
            },
        ),
    )
    .await?;
    change(
        service,
        owner,
        request(
            2,
            RoleChange::AssignRole {
                account_id: delegate_id.authorization.value.account_id.clone(),
                role_id: delegator.id.clone(),
            },
        ),
    )
    .await?;
    Ok((delegator, member_role))
}

async fn assert_assignment_does_not_authorize_definition(
    service: &Service,
    delegate: &Session,
    role: &Role,
) -> Result<()> {
    // The session predates delegation; no broad credential capability grants it.
    assert!(
        !delegate
            .membership
            .value
            .capabilities
            .contains(&"server.manage".into())
    );
    let result = change(
        service,
        delegate,
        request(4, RoleChange::PutRole { role: role.clone() }),
    )
    .await;
    assert!(
        result.is_err(),
        "assignment authority cannot edit definitions"
    );
    Ok(())
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn role_api_uses_current_delegation_and_rejects_stale_credentials_and_escalation()
-> Result<()> {
    let (service, _directory) = fixture().await?;
    let (owner_id, owner_contact) = identity()?;
    let owner_session = join(&service, &owner_id, owner_contact).await?;
    let owner = grant_fixture_owner(&service, owner_session).await?;
    let (delegate_id, delegate_contact) = identity()?;
    let delegate = join(&service, &delegate_id, delegate_contact).await?;
    let (target_id, target_contact) = identity()?;
    let _target = join(&service, &target_id, target_contact).await?;
    activate_candidate(&service).await?;
    let (delegator, member_role) = establish_delegation(&service, &owner, &delegate_id).await?;
    let assigned = request(
        3,
        RoleChange::AssignRole {
            account_id: target_id.authorization.value.account_id.clone(),
            role_id: member_role.id.clone(),
        },
    );
    assert_eq!(change(&service, &delegate, assigned.clone()).await?, 4);
    assert_eq!(change(&service, &delegate, assigned.clone()).await?, 4);
    assert_assignment_does_not_authorize_definition(&service, &delegate, &member_role).await?;
    for account in [
        &delegate_id.authorization.value.account_id,
        &owner_id.authorization.value.account_id,
    ] {
        assert!(
            change(
                &service,
                &delegate,
                request(
                    4,
                    RoleChange::AssignRole {
                        account_id: account.clone(),
                        role_id: member_role.id.clone(),
                    }
                )
            )
            .await
            .is_err()
        );
    }
    assert!(
        change(
            &service,
            &delegate,
            request(
                4,
                RoleChange::AssignRole {
                    account_id: target_id.authorization.value.account_id.clone(),
                    role_id: cords_protocol::roles::ADMIN_ROLE.into(),
                }
            )
        )
        .await
        .is_err()
    );
    let catalog = roles::catalog(State(service.clone()), headers(&delegate)?)
        .await?
        .0;
    assert!(catalog.value.active);
    change(
        &service,
        &owner,
        request(
            4,
            RoleChange::RevokeRole {
                account_id: delegate_id.authorization.value.account_id.clone(),
                role_id: delegator.id,
            },
        ),
    )
    .await?;
    // Revoking delegation removes management, while ordinary members retain
    // the read-only catalog needed to display profile role badges.
    assert!(
        roles::catalog(State(service.clone()), headers(&delegate)?)
            .await?
            .0
            .value
            .active
    );
    let hints = roles::actions(
        State(service.clone()),
        headers(&delegate)?,
        Query(roles::ActionScope::default()),
    )
    .await?
    .0
    .value;
    assert!(!hints.permissions.contains(&Permission::ManageRoles));
    assert!(!hints.permissions.contains(&Permission::AssignRoles));
    assert!(
        change(
            &service,
            &delegate,
            request(
                5,
                RoleChange::RevokeRole {
                    account_id: target_id.authorization.value.account_id.clone(),
                    role_id: member_role.id,
                }
            )
        )
        .await
        .is_err()
    );
    // A receipt remains available without reapplying its historical authority.
    assert_eq!(change(&service, &delegate, assigned).await?, 4);
    let audit: i64 = sqlx::query_scalar("SELECT count(*) FROM role_policy_changes")
        .fetch_one(service.0.store.pool())
        .await?;
    assert_eq!(audit, 5);
    Ok(())
}

async fn sibling_session(
    service: &Service,
    contact: &Contact,
    key: &ed25519_dalek::SigningKey,
) -> Result<Session> {
    let proof = join_challenge(
        State(service.clone()),
        Json(ChallengeRequest {
            contact: contact.clone(),
            purpose: "join".into(),
            idempotency_key: id(),
        }),
    )
    .await?
    .0;
    Ok(session(
        State(service.clone()),
        Json(SessionRequest {
            role_policy_version: Some(1),
            admission_profile: None,
            proof: cords_identity::sign(key, proof)?,
            idempotency_key: id(),
        }),
    )
    .await?
    .0)
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn enforced_fresh_owner_public_and_moderated_device_admission_are_independent() -> Result<()>
{
    let (service, _directory, code) = fixture_with_state(false).await?;
    activate_candidate(&service).await?;
    let (owner_id, owner_contact) = identity()?;
    let owner = claim_ownership(
        State(service.clone()),
        Json(ownership_request(&service, &owner_id, owner_contact, &code).await?),
    )
    .await?
    .0;
    service
        .authenticate(&headers(&owner)?, "server.manage")
        .await?;
    let (member_id, member_contact) = identity()?;
    let member = join(&service, &member_id, member_contact.clone()).await?;
    service
        .authenticate(&headers(&member)?, "channel.read")
        .await?;
    let approved: bool =
        sqlx::query_scalar("SELECT approved FROM server_device_admission WHERE device_id=$1")
            .bind(&member.membership.value.device_id)
            .fetch_one(service.0.store.pool())
            .await?;
    assert!(
        !approved,
        "public account admission must not imply administrator device approval"
    );
    let moderated = Service::with_policy(
        service.0.store.clone(),
        service.0.identity.clone(),
        60,
        900,
        JoinPolicy::ModeratorApproval,
    )?;
    let key = ed25519_dalek::SigningKey::generate(&mut rand_core::OsRng);
    let mut authorization = member_id.authorization.value.clone();
    authorization.device_id = id();
    authorization.device_public_key = encode(key.verifying_key().to_bytes());
    authorization.generation += 1;
    authorization.previous_record_hash = Some(hash(canonical(&member_id.authorization)?));
    let authorization = member_id.sign_root(authorization)?;
    let crypto = ConversationCrypto::new(authorization.value.device_id.clone())?;
    let sibling = Contact {
        mls_binding: cords_identity::sign(
            &key,
            cords_protocol::messaging::MlsBinding {
                version: 1,
                account_id: authorization.value.account_id.clone(),
                device_id: authorization.value.device_id.clone(),
                authorization_hash: hash(canonical(&authorization)?),
                mls_public_key: encode(crypto.public_key()),
            },
        )?,
        authorization,
        user_card: None,
    };
    // Root certification admits a sibling only in account-authorized mode;
    // it never admits an unknown account to a moderated server.
    let sibling_member = sibling_session(&moderated, &sibling, &key).await?;
    moderated
        .authenticate(&headers(&sibling_member)?, "channel.read")
        .await?;
    let (stranger_id, stranger_contact) = identity()?;
    assert!(
        join(&moderated, &stranger_id, stranger_contact)
            .await
            .is_err()
    );
    sqlx::query(
        "UPDATE role_policy SET device_admission='administrator_approval' WHERE singleton=TRUE",
    )
    .execute(service.0.store.pool())
    .await?;
    assert!(
        moderated
            .authenticate(&headers(&sibling_member)?, "channel.read")
            .await
            .is_err()
    );
    assert!(sibling_session(&moderated, &sibling, &key).await.is_err());
    let approved = approve_membership(
        State(moderated.clone()),
        headers(&owner)?,
        Path(sibling.authorization.value.device_id.clone()),
    )
    .await?
    .0;
    assert_eq!(
        approved.value.account_id,
        member.membership.value.account_id
    );
    let renewed = sibling_session(&moderated, &sibling, &key).await?;
    moderated
        .authenticate(&headers(&renewed)?, "channel.read")
        .await?;
    // Public admission cannot bypass administrator device approval either.
    let (public_id, public_contact) = identity()?;
    assert!(
        join(&service, &public_id, public_contact.clone())
            .await
            .is_err()
    );
    let _approved = approve_membership(
        State(service.clone()),
        headers(&owner)?,
        Path(public_id.authorization.value.device_id.clone()),
    )
    .await?
    .0;
    let public = join(&service, &public_id, public_contact).await?;
    service
        .authenticate(&headers(&public)?, "channel.read")
        .await?;
    let accepted: bool =
        sqlx::query_scalar("SELECT accepted FROM server_account_admission WHERE account_id=$1")
            .bind(&public.membership.value.account_id)
            .fetch_one(service.0.store.pool())
            .await?;
    assert!(accepted);
    Ok(())
}

fn recovery_request(
    identity: &Identity,
    session: &Session,
    candidate: &cords_protocol::roles::ChannelRecoveryCandidate,
) -> Result<ChannelReplace> {
    let crypto = ConversationCrypto::new(session.membership.value.device_id.clone())?;
    let record = identity.sign_device(cords_protocol::messaging::ChannelSuccession {
        version: 1,
        server_id: candidate.server_id.clone(),
        predecessor: candidate.predecessor.clone(),
        successor_channel_id: id(),
        name: "explicit fresh encrypted group".into(),
        confidentiality_mode: ConfidentialityMode::Encrypted,
        initiator: identity.contact(&crypto.public_key())?,
        membership: session.membership.clone(),
        issued_at: now()?,
        nonce: id(),
    })?;
    Ok(ChannelReplace {
        recovery: Some(
            identity.sign_device(cords_protocol::roles::ChannelRemovalRecovery {
                version: 1,
                server_id: candidate.server_id.clone(),
                account_id: session.membership.value.account_id.clone(),
                device_id: session.membership.value.device_id.clone(),
                channel_id: candidate.predecessor.value.channel_id.clone(),
                policy_revision: candidate.policy_revision,
                channel_epoch: candidate.channel_epoch,
                pending_digest: candidate.pending_digest.clone(),
                succession_digest: hash(canonical(&record)?),
            })?,
        ),
        record,
        idempotency_key: id(),
    })
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn removal_recovery_is_scoped_current_owner_only_and_keeps_ordinary_guard() -> Result<()> {
    let (service, _directory) = fixture().await?;
    let (owner_id, owner_contact) = identity()?;
    let owner =
        grant_fixture_owner(&service, join(&service, &owner_id, owner_contact).await?).await?;
    let (member_id, member_contact) = identity()?;
    let member = join(&service, &member_id, member_contact).await?;
    let route = create_channel(
        State(service.clone()),
        headers(&owner)?,
        Json(ChannelCreate {
            name: "fenced predecessor".into(),
            confidentiality_mode: ConfidentialityMode::Encrypted,
            idempotency_key: id(),
        }),
    )
    .await?
    .0;
    sqlx::query("INSERT INTO channel_members(channel_id,device_id) VALUES($1,$2)")
        .bind(&route)
        .bind(&member.membership.value.device_id)
        .execute(service.0.store.pool())
        .await?;
    activate_candidate(&service).await?;
    remove_default_permission(&service, &owner, Permission::ReadMessages).await?;
    let candidates = recovery::list(
        State(service.clone()),
        headers(&owner)?,
        Query(recovery::RecoveryQuery::default()),
    )
    .await?
    .0;
    assert_eq!(candidates.len(), 1);
    assert!(candidates[0].value.committer_available);
    let attempted = recovery_request(&owner_id, &owner, &candidates[0].value)?;
    assert!(
        replace_channel(
            State(service.clone()),
            headers(&owner)?,
            Path(route.clone()),
            Json(attempted)
        )
        .await
        .is_err()
    );
    assert!(
        recovery::list(
            State(service.clone()),
            headers(&member)?,
            Query(recovery::RecoveryQuery::default())
        )
        .await
        .is_err()
    );
    // The current owner remains a valid account/device but no longer has a leaf.
    let mut tx = service.0.store.pool().begin().await?;
    lifecycle::apply_remove(&mut tx, &route, &owner.membership.value.device_id).await?;
    let operation = lifecycle::reserve_removal(
        &mut tx,
        &route,
        &owner.membership.value.device_id,
        &member.membership.value.device_id,
        0,
    )
    .await?;
    tx.commit().await?;
    let candidates = recovery::list(
        State(service.clone()),
        headers(&owner)?,
        Query(recovery::RecoveryQuery::default()),
    )
    .await?
    .0;
    assert!(!candidates[0].value.committer_available);
    let request = recovery_request(&owner_id, &owner, &candidates[0].value)?;
    let mut ordinary = request.clone();
    ordinary.recovery = None;
    assert!(
        replace_channel(
            State(service.clone()),
            headers(&owner)?,
            Path(route.clone()),
            Json(ordinary)
        )
        .await
        .is_err()
    );
    let mut substituted = request.clone();
    substituted.record.value.successor_channel_id = id();
    substituted.record = owner_id.sign_device(substituted.record.value)?;
    assert!(
        replace_channel(
            State(service.clone()),
            headers(&owner)?,
            Path(route.clone()),
            Json(substituted)
        )
        .await
        .is_err()
    );
    let mut stale = request.clone();
    stale
        .recovery
        .as_mut()
        .context("proof")?
        .value
        .policy_revision += 1;
    stale.recovery = Some(owner_id.sign_device(stale.recovery.context("proof")?.value)?);
    assert!(
        replace_channel(
            State(service.clone()),
            headers(&owner)?,
            Path(route.clone()),
            Json(stale)
        )
        .await
        .is_err()
    );
    let successor = replace_channel(
        State(service.clone()),
        headers(&owner)?,
        Path(route.clone()),
        Json(request.clone()),
    )
    .await?
    .0;
    assert_ne!(successor, route);
    assert_eq!(
        replace_channel(
            State(service.clone()),
            headers(&owner)?,
            Path(route.clone()),
            Json(request)
        )
        .await?
        .0,
        successor
    );
    let cancelled: (bool, bool) =
        sqlx::query_as("SELECT complete,cancelled FROM roster_operations WHERE id=$1")
            .bind(&operation.operation_id)
            .fetch_one(service.0.store.pool())
            .await?;
    assert_eq!(cancelled, (false, true));
    let audits: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM channel_recovery_audit WHERE predecessor_id=$1 AND successor_id=$2",
    )
    .bind(&route)
    .bind(&successor)
    .fetch_one(service.0.store.pool())
    .await?;
    assert_eq!(audits, 1);
    assert!(
        recovery::list(
            State(service.clone()),
            headers(&owner)?,
            Query(recovery::RecoveryQuery::default())
        )
        .await?
        .0
        .is_empty()
    );
    let view = channel(
        State(service.clone()),
        headers(&owner)?,
        Path(successor),
        Query(ChannelQuery { epoch: None }),
    )
    .await?
    .0;
    assert_eq!(view.epoch, 0);
    assert_eq!(view.members.len(), 1);
    assert!(view.binding.is_none());
    assert!(
        channel(
            State(service.clone()),
            headers(&owner)?,
            Path(route),
            Query(ChannelQuery { epoch: None })
        )
        .await
        .is_err()
    );
    Ok(())
}

fn application(session: &Session, route: &str) -> Result<EventUpload> {
    // Relay authorization test only. The server neither decrypts this fixture nor
    // treats it as proof of cryptographic continuity (tested separately).
    let crypto = ConversationCrypto::new(session.membership.value.device_id.clone())?;
    crypto.create(route)?;
    Ok(EventUpload {
        protocol_version: 1,
        event_id: id(),
        route_kind: "channel".into(),
        route_id: route.into(),
        sender_member_id: session.membership.value.member_id.clone(),
        sender_device_id: session.membership.value.device_id.clone(),
        client_created_at: now()?,
        content_encoding: "mls.application".into(),
        ciphertext: encode(crypto.encrypt(route, b"authorization fixture")?),
        public_message: None,
        idempotency_key: id(),
    })
}

async fn deny_channel(service: &Service, route: &str, permission: Permission) -> Result<()> {
    use cords_protocol::roles::{DEFAULT_ROLE, Override, Overrides};
    let overrides: Overrides = [(
        DEFAULT_ROLE.into(),
        Override {
            allow: std::collections::BTreeSet::new(),
            deny: [permission].into(),
        },
    )]
    .into();
    sqlx::query("UPDATE channel_access_policy SET overrides=$1 WHERE channel_id=$2")
        .bind(json(&overrides)?)
        .bind(route)
        .execute(service.0.store.pool())
        .await?;
    Ok(())
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn application_and_history_reauthorize_current_channel_policy_and_exclusions() -> Result<()> {
    let (service, _directory) = fixture().await?;
    let (owner_id, owner_contact) = identity()?;
    let owner =
        grant_fixture_owner(&service, join(&service, &owner_id, owner_contact).await?).await?;
    let (member_id, member_contact) = identity()?;
    let member = join(&service, &member_id, member_contact).await?;
    let route = create_channel(
        State(service.clone()),
        headers(&owner)?,
        Json(ChannelCreate {
            confidentiality_mode: ConfidentialityMode::Encrypted,
            name: "restricted".into(),
            idempotency_key: id(),
        }),
    )
    .await?
    .0;
    sqlx::query("INSERT INTO channel_members(channel_id,device_id) VALUES($1,$2)")
        .bind(&route)
        .bind(&member_id.authorization.value.device_id)
        .execute(service.0.store.pool())
        .await?;
    activate_candidate(&service).await?;
    let accepted = upload_event(
        State(service.clone()),
        headers(&member)?,
        Path(route.clone()),
        Json(application(&member, &route)?),
    )
    .await?
    .0;
    assert_eq!(accepted.server_sequence, 1);
    deny_channel(&service, &route, Permission::SendMessages).await?;
    let denied = upload_event(
        State(service.clone()),
        headers(&member)?,
        Path(route.clone()),
        Json(application(&member, &route)?),
    )
    .await;
    let status = denied.err().context("current send denial bypassed")?.0;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let history = super::history(
        State(service.clone()),
        headers(&member)?,
        Path(route.clone()),
        Query(HistoryQuery { after: 0 }),
    )
    .await?
    .0;
    assert_eq!(history.len(), 1);
    deny_channel(&service, &route, Permission::ReadMessages).await?;
    let denied = super::history(
        State(service.clone()),
        headers(&member)?,
        Path(route.clone()),
        Query(HistoryQuery { after: 0 }),
    )
    .await;
    let status = denied.err().context("current read denial bypassed")?.0;
    assert_eq!(status, StatusCode::FORBIDDEN);
    sqlx::query("UPDATE channel_access_policy SET overrides='{}' WHERE channel_id=$1")
        .bind(&route)
        .execute(service.0.store.pool())
        .await?;
    sqlx::query("INSERT INTO channel_device_exclusions VALUES($1,$2)")
        .bind(&route)
        .bind(&member_id.authorization.value.device_id)
        .execute(service.0.store.pool())
        .await?;
    assert!(
        super::history(
            State(service.clone()),
            headers(&member)?,
            Path(route.clone()),
            Query(HistoryQuery { after: 0 })
        )
        .await
        .is_err()
    );
    assert!(
        upload_event(
            State(service.clone()),
            headers(&member)?,
            Path(route.clone()),
            Json(application(&member, &route)?)
        )
        .await
        .is_err()
    );
    let counts:(i64,i64)=sqlx::query_as("SELECT (SELECT count(*) FROM route_events WHERE route_id=$1),(SELECT count(*) FROM channel_account_access WHERE channel_id=$1 AND account_id=$2)")
        .bind(&route).bind(&member_id.authorization.value.account_id).fetch_one(service.0.store.pool()).await?;
    assert_eq!(
        counts,
        (1, 1),
        "denials append no event and account eligibility cannot override exclusion"
    );
    Ok(())
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn delegated_server_grants_replace_legacy_capabilities_without_granting_roster_management()
-> Result<()> {
    let (service, _directory) = fixture().await?;
    let (owner_id, owner_contact) = identity()?;
    let owner_session = join(&service, &owner_id, owner_contact).await?;
    let owner = grant_fixture_owner(&service, owner_session).await?;
    let (delegate_id, delegate_contact) = identity()?;
    let delegate = join(&service, &delegate_id, delegate_contact).await?;
    let route = create_channel(
        State(service.clone()),
        headers(&owner)?,
        Json(ChannelCreate {
            name: "guarded".into(),
            confidentiality_mode: ConfidentialityMode::Encrypted,
            idempotency_key: id(),
        }),
    )
    .await?
    .0;
    sqlx::query("INSERT INTO channel_members(channel_id,device_id) VALUES($1,$2)")
        .bind(&route)
        .bind(&delegate_id.authorization.value.device_id)
        .execute(service.0.store.pool())
        .await?;
    activate_candidate(&service).await?;
    assert!(
        !delegate
            .membership
            .value
            .capabilities
            .contains(&"server.manage".to_owned())
    );
    assert!(
        service
            .authenticate(&headers(&delegate)?, "server.manage")
            .await
            .is_err()
    );
    let role = Role {
        id: id(),
        name: "server staff".into(),
        color: "#83bdd9".into(),
        description: String::new(),
        priority: 150,
        permissions: [Permission::ManageServer, Permission::ApproveAccounts].into(),
        mentionable: false,
    };
    change(
        &service,
        &owner,
        request(0, RoleChange::PutRole { role: role.clone() }),
    )
    .await?;
    change(
        &service,
        &owner,
        request(
            1,
            RoleChange::AssignRole {
                account_id: delegate_id.authorization.value.account_id.clone(),
                role_id: role.id.clone(),
            },
        ),
    )
    .await?;
    service
        .authenticate(&headers(&delegate)?, "server.manage")
        .await?;
    assert!(
        membership_requests(
            State(service.clone()),
            headers(&delegate)?,
            Query(MembershipPage {
                after: String::new(),
                profiles: false
            })
        )
        .await?
        .0
        .is_empty()
    );
    // Default MLS participation is not authority to add arbitrary participants.
    let result = roster(
        State(service.clone()),
        headers(&delegate)?,
        Path(route.clone()),
        Json(RosterRequest {
            action: RosterAction::Add,
            target_device_id: owner_id.authorization.value.device_id.clone(),
            idempotency_key: id(),
        }),
    )
    .await;
    assert_eq!(
        result
            .err()
            .context("roster management unexpectedly allowed")?
            .0,
        StatusCode::FORBIDDEN
    );
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM roster_operations WHERE channel_id=$1")
            .bind(&route)
            .fetch_one(service.0.store.pool())
            .await?;
    assert_eq!(count, 0);
    change(
        &service,
        &owner,
        request(
            2,
            RoleChange::RevokeRole {
                account_id: delegate_id.authorization.value.account_id.clone(),
                role_id: role.id,
            },
        ),
    )
    .await?;
    assert!(
        service
            .authenticate(&headers(&delegate)?, "server.manage")
            .await
            .is_err()
    );
    Ok(())
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn role_changes_queue_signed_removal_and_fence_only_affected_channel() -> Result<()> {
    let (service, _directory) = fixture().await?;
    let (owner_id, owner_contact) = identity()?;
    let owner =
        grant_fixture_owner(&service, join(&service, &owner_id, owner_contact).await?).await?;
    let (member_id, member_contact) = identity()?;
    let member = join(&service, &member_id, member_contact).await?;
    let mut routes = Vec::new();
    for name in ["affected", "unaffected"] {
        let route = create_channel(
            State(service.clone()),
            headers(&owner)?,
            Json(ChannelCreate {
                name: name.into(),
                confidentiality_mode: ConfidentialityMode::Encrypted,
                idempotency_key: id(),
            }),
        )
        .await?
        .0;
        sqlx::query("INSERT INTO channel_members(channel_id,device_id) VALUES($1,$2)")
            .bind(&route)
            .bind(&member_id.authorization.value.device_id)
            .execute(service.0.store.pool())
            .await?;
        routes.push(route);
    }
    activate_candidate(&service).await?;
    let mut marker = Role {
        id: id(),
        name: "restricted marker".into(),
        color: "#83bdd9".into(),
        description: String::new(),
        priority: 10,
        permissions: [].into(),
        mentionable: false,
    };
    change(
        &service,
        &owner,
        request(
            0,
            RoleChange::PutRole {
                role: marker.clone(),
            },
        ),
    )
    .await?;
    change(
        &service,
        &owner,
        request(
            1,
            RoleChange::AssignRole {
                account_id: member_id.authorization.value.account_id.clone(),
                role_id: marker.id.clone(),
            },
        ),
    )
    .await?;
    let overrides: cords_protocol::roles::Overrides = [(
        marker.id.clone(),
        cords_protocol::roles::Override {
            allow: [].into(),
            deny: [Permission::ReadMessages].into(),
        },
    )]
    .into();
    sqlx::query("UPDATE channel_access_policy SET overrides=$1 WHERE channel_id=$2")
        .bind(json(&overrides)?)
        .bind(&routes[0])
        .execute(service.0.store.pool())
        .await?;
    marker.name = "renamed marker".into();
    let mutation = request(2, RoleChange::PutRole { role: marker });
    assert_eq!(change(&service, &owner, mutation.clone()).await?, 3);
    assert_eq!(change(&service, &owner, mutation).await?, 3);
    let pending = lifecycle::pending_removals(
        State(service.clone()),
        headers(&owner)?,
        Path(routes[0].clone()),
    )
    .await?
    .0;
    assert_eq!(pending.len(), 1);
    let cords_protocol::messaging::PolicyRemoval::ServerPolicy { device_id, policy } = &pending[0]
    else {
        anyhow::bail!("missing signed server policy evidence");
    };
    policy.verify(&encode(
        service.0.identity.signing_key.verifying_key().as_bytes(),
    ))?;
    assert_eq!(device_id, &member_id.authorization.value.device_id);
    assert_eq!(
        policy.value.account_id,
        member_id.authorization.value.account_id
    );
    assert_eq!(policy.value.channel_id, routes[0]);
    assert_eq!(policy.value.policy_revision, 3);
    assert_eq!(policy.value.channel_epoch, 0);
    assert!(
        history(
            State(service.clone()),
            headers(&member)?,
            Path(routes[0].clone()),
            Query(HistoryQuery { after: 0 })
        )
        .await
        .is_err()
    );
    let blocked = upload_event(
        State(service.clone()),
        headers(&owner)?,
        Path(routes[0].clone()),
        Json(application(&owner, &routes[0])?),
    )
    .await;
    let error = blocked
        .err()
        .context("protected channel send was not fenced")?;
    assert_eq!(error.0, StatusCode::CONFLICT);
    assert_eq!(error.1, "CORDS_CHANNEL_REMOVAL_PENDING");
    assert!(
        history(
            State(service.clone()),
            headers(&owner)?,
            Path(routes[0].clone()),
            Query(HistoryQuery { after: 0 })
        )
        .await?
        .0
        .is_empty()
    );
    assert_eq!(
        upload_event(
            State(service.clone()),
            headers(&owner)?,
            Path(routes[1].clone()),
            Json(application(&owner, &routes[1])?)
        )
        .await?
        .0
        .server_sequence,
        1
    );
    // Restoring eligibility never silently revives the excluded old leaf.
    sqlx::query("UPDATE channel_access_policy SET overrides='{}' WHERE channel_id=$1")
        .bind(&routes[0])
        .execute(service.0.store.pool())
        .await?;
    assert!(
        history(
            State(service.clone()),
            headers(&member)?,
            Path(routes[0].clone()),
            Query(HistoryQuery { after: 0 })
        )
        .await
        .is_err()
    );
    let operation = roster(
        State(service.clone()),
        headers(&owner)?,
        Path(routes[0].clone()),
        Json(RosterRequest {
            action: RosterAction::Remove,
            target_device_id: device_id.clone(),
            idempotency_key: id(),
        }),
    )
    .await?
    .0;
    assert_eq!(operation.action, RosterAction::Remove);
    Ok(())
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn discretionary_removal_evidence_survives_requester_and_allows_committer_takeover()
-> Result<()> {
    let (service, _directory) = fixture().await?;
    let (owner_id, contact) = identity()?;
    let owner = grant_fixture_owner(&service, join(&service, &owner_id, contact).await?).await?;
    let (target_id, contact) = identity()?;
    let _target = join(&service, &target_id, contact).await?;
    let (committer_id, contact) = identity()?;
    let committer = join(&service, &committer_id, contact).await?;
    let route = create_channel(
        State(service.clone()),
        headers(&owner)?,
        Json(ChannelCreate {
            name: "removal evidence".into(),
            confidentiality_mode: ConfidentialityMode::Encrypted,
            idempotency_key: id(),
        }),
    )
    .await?
    .0;
    for device in [
        &target_id.authorization.value.device_id,
        &committer_id.authorization.value.device_id,
    ] {
        sqlx::query("INSERT INTO channel_members(channel_id,device_id) VALUES($1,$2)")
            .bind(&route)
            .bind(device)
            .execute(service.0.store.pool())
            .await?;
    }
    activate_candidate(&service).await?;
    let request = RosterRequest {
        action: RosterAction::Remove,
        target_device_id: target_id.authorization.value.device_id.clone(),
        idempotency_key: id(),
    };
    let abandoned = roster(
        State(service.clone()),
        headers(&owner)?,
        Path(route.clone()),
        Json(request),
    )
    .await?
    .0;
    let removals = lifecycle::pending_removals(
        State(service.clone()),
        headers(&committer)?,
        Path(route.clone()),
    )
    .await?
    .0;
    let [cords_protocol::messaging::PolicyRemoval::ServerPolicy { device_id, policy }] =
        removals.as_slice()
    else {
        anyhow::bail!("missing durable discretionary removal evidence");
    };
    policy.verify(&encode(
        service.0.identity.signing_key.verifying_key().to_bytes(),
    ))?;
    assert_eq!(device_id, &target_id.authorization.value.device_id);
    assert_eq!(
        policy.value.account_id,
        target_id.authorization.value.account_id
    );
    // This participant has CommitMls through $default, without ManageChannelMembers.
    let replacement = roster(
        State(service.clone()),
        headers(&committer)?,
        Path(route.clone()),
        Json(RosterRequest {
            action: RosterAction::Remove,
            target_device_id: device_id.clone(),
            idempotency_key: id(),
        }),
    )
    .await?
    .0;
    assert_ne!(abandoned.operation_id, replacement.operation_id);
    let cancelled: bool = sqlx::query_scalar("SELECT cancelled FROM roster_operations WHERE id=$1")
        .bind(&abandoned.operation_id)
        .fetch_one(service.0.store.pool())
        .await?;
    assert!(cancelled);
    let pending: i64 = sqlx::query_scalar("SELECT count(*) FROM roster_operations WHERE channel_id=$1 AND NOT complete AND NOT cancelled")
        .bind(&route).fetch_one(service.0.store.pool()).await?;
    assert_eq!(pending, 1);
    let proofs: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM channel_policy_removal_proofs WHERE channel_id=$1",
    )
    .bind(&route)
    .fetch_one(service.0.store.pool())
    .await?;
    assert_eq!(proofs, 1);
    Ok(())
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn enforced_sessions_require_explicit_negotiation_and_preserve_unused_proofs() -> Result<()> {
    let (service, _directory) = fixture().await?;
    let (owner_id, owner_contact) = identity()?;
    let owner =
        grant_fixture_owner(&service, join(&service, &owner_id, owner_contact).await?).await?;
    let (member_id, member_contact) = identity()?;
    let member = join(&service, &member_id, member_contact.clone()).await?;
    sqlx::query("UPDATE sessions SET role_policy_version=NULL WHERE device_id=$1")
        .bind(&member_id.authorization.value.device_id)
        .execute(service.0.store.pool())
        .await?;
    activate_candidate(&service).await?;
    service
        .authenticate(&headers(&owner)?, "server.manage")
        .await?;
    let failure = service
        .authenticate(&headers(&member)?, "channel.read")
        .await
        .err()
        .context("legacy session was accepted")?;
    assert_eq!(failure.0, StatusCode::UPGRADE_REQUIRED);
    assert_eq!(failure.1, "CORDS_CLIENT_UPGRADE_REQUIRED");
    let challenge = auth_challenge(
        State(service.clone()),
        Json(ChallengeRequest {
            contact: member_contact,
            purpose: "authenticate".into(),
            idempotency_key: id(),
        }),
    )
    .await?
    .0;
    let challenge_id = challenge.challenge_id.clone();
    let mut request = SessionRequest {
        role_policy_version: None,
        proof: member_id.sign_device(challenge)?,
        idempotency_key: id(),
        admission_profile: None,
    };
    for version in [None, Some(99)] {
        request.role_policy_version = version;
        let failure = session(State(service.clone()), Json(request.clone()))
            .await
            .err()
            .context("unsupported negotiation was accepted")?;
        assert_eq!(failure.0, StatusCode::UPGRADE_REQUIRED);
        let consumed: bool = sqlx::query_scalar("SELECT consumed FROM auth_challenges WHERE id=$1")
            .bind(&challenge_id)
            .fetch_one(service.0.store.pool())
            .await?;
        assert!(!consumed);
    }
    request.role_policy_version = Some(1);
    let upgraded = session(State(service.clone()), Json(request)).await?.0;
    service
        .authenticate(&headers(&upgraded)?, "channel.read")
        .await?;
    Ok(())
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn enrollment_rechecks_target_policy_at_reservation_and_commit() -> Result<()> {
    let (service, _directory) = fixture().await?;
    let (owner_id, owner_contact) = identity()?;
    let owner =
        grant_fixture_owner(&service, join(&service, &owner_id, owner_contact).await?).await?;
    let member_id = Identity::generate(now()?)?;
    let crypto = ConversationCrypto::new(member_id.authorization.value.device_id.clone())?;
    let member = join(
        &service,
        &member_id,
        member_id.contact(&crypto.public_key())?,
    )
    .await?;
    let route = create_channel(
        State(service.clone()),
        headers(&owner)?,
        Json(ChannelCreate {
            name: "explicit enrollment".into(),
            confidentiality_mode: ConfidentialityMode::Encrypted,
            idempotency_key: id(),
        }),
    )
    .await?
    .0;
    activate_candidate(&service).await?;
    let target = &member.membership.value.device_id;
    sqlx::query(
        "UPDATE role_policy SET device_admission='administrator_approval' WHERE singleton=TRUE",
    )
    .execute(service.0.store.pool())
    .await?;
    let published = upload_package(
        State(service.clone()),
        headers(&member)?,
        Json(KeyPackageUpload {
            key_package: encode(crypto.key_package()?),
            idempotency_key: id(),
        }),
    )
    .await?
    .0;
    assert!(!published.is_empty());
    // An approved account need not already be included in the restricted channel.
    // The explicit authorized add establishes that boundary only at commit.
    sqlx::query("INSERT INTO channel_device_exclusions(channel_id,device_id) VALUES($1,$2)")
        .bind(&route)
        .bind(target)
        .execute(service.0.store.pool())
        .await?;
    let operation = roster(
        State(service.clone()),
        headers(&owner)?,
        Path(route.clone()),
        Json(RosterRequest {
            action: RosterAction::Add,
            target_device_id: target.clone(),
            idempotency_key: id(),
        }),
    )
    .await?
    .0;
    let upload = CommitUpload {
        operation_id: operation.operation_id,
        base_epoch: operation.base_epoch,
        commit: encode(b"opaque MLS relay test"),
        welcome: encode(b"opaque Welcome relay test"),
        binding: owner_id.sign_device(cords_protocol::messaging::GroupBinding {
            version: 1,
            server_id: service.0.identity.server_id(),
            channel_id: route.clone(),
            creator_device_id: owner.membership.value.device_id.clone(),
            group_id: route.clone(),
        })?,
        idempotency_key: id(),
    };
    sqlx::query("UPDATE server_device_admission SET approved=FALSE WHERE device_id=$1")
        .bind(target)
        .execute(service.0.store.pool())
        .await?;
    assert!(
        commit(
            State(service.clone()),
            headers(&owner)?,
            Path(route.clone()),
            Json(upload.clone())
        )
        .await
        .is_err()
    );
    let epoch: i64 = sqlx::query_scalar("SELECT epoch FROM channels WHERE id=$1")
        .bind(&route)
        .fetch_one(service.0.store.pool())
        .await?;
    assert_eq!(epoch, 0);
    sqlx::query("UPDATE server_device_admission SET approved=TRUE WHERE device_id=$1")
        .bind(target)
        .execute(service.0.store.pool())
        .await?;
    deny_channel(&service, &route, Permission::ReadMessages).await?;
    assert!(
        commit(
            State(service.clone()),
            headers(&owner)?,
            Path(route.clone()),
            Json(upload.clone())
        )
        .await
        .is_err()
    );
    sqlx::query("UPDATE channel_access_policy SET overrides='{}' WHERE channel_id=$1")
        .bind(&route)
        .execute(service.0.store.pool())
        .await?;
    let committed = commit(
        State(service.clone()),
        headers(&owner)?,
        Path(route.clone()),
        Json(upload),
    )
    .await?
    .0;
    assert_eq!(committed.server_sequence, 1);
    let enrolled: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM channel_account_access WHERE channel_id=$1 AND account_id=$2) AND NOT EXISTS(SELECT 1 FROM channel_device_exclusions WHERE channel_id=$1 AND device_id=$3)")
        .bind(&route).bind(&member.membership.value.account_id).bind(target).fetch_one(service.0.store.pool()).await?;
    assert!(enrolled);
    let mut tx = service.0.store.pool().begin().await?;
    lifecycle::apply_remove(&mut tx, &route, target).await?;
    tx.commit().await?;
    let excluded: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM channel_device_exclusions WHERE channel_id=$1 AND device_id=$2)")
        .bind(&route).bind(target).fetch_one(service.0.store.pool()).await?;
    assert!(excluded);
    sqlx::query("UPDATE server_device_admission SET approved=FALSE WHERE device_id=$1")
        .bind(target)
        .execute(service.0.store.pool())
        .await?;
    assert!(
        roster(
            State(service.clone()),
            headers(&owner)?,
            Path(route.clone()),
            Json(RosterRequest {
                action: RosterAction::Add,
                target_device_id: target.clone(),
                idempotency_key: id(),
            })
        )
        .await
        .is_err()
    );
    let reserved: i64 =
        sqlx::query_scalar("SELECT count(*) FROM roster_operations WHERE channel_id=$1")
            .bind(&route)
            .fetch_one(service.0.store.pool())
            .await?;
    assert_eq!(reserved, 1);
    Ok(())
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL and independent TLS/MLS clients"]
async fn policy_loss_completes_real_mls_removal_and_requires_fresh_enrollment() -> Result<()> {
    use cords_client_core::client::Client;
    let (service, _directory, code) = fixture_with_state(false).await?;
    let certificate = rcgen::generate_simple_self_signed(vec!["localhost".into()])?;
    let tls = https_fixture(&service, &certificate).await?;
    let root = tempfile::tempdir()?;
    let ca = root.path().join("test-ca.pem");
    std::fs::write(&ca, certificate.cert.pem())?;
    let migrations =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../migrations/sqlite");
    let pass = b"independent role-loss acceptance vault";
    let mut owner = Client::open(
        &root.path().join("owner"),
        &migrations,
        Some(pass),
        Some(&ca),
    )
    .await?;
    let mut member = Client::open(
        &root.path().join("member"),
        &migrations,
        Some(pass),
        Some(&ca),
    )
    .await?;
    owner.trust(&tls.origin).await?;
    owner.claim_ownership(&code).await?;
    member.trust(&tls.origin).await?;
    member.authenticate().await?;
    let device = member.status().device_id;
    let account = member.status().account_id;
    let affected = owner.create_channel("affected encrypted route").await?;
    let unaffected = owner.create_channel("unaffected encrypted route").await?;
    for route in [&affected, &unaffected] {
        member.publish_key_package().await?;
        owner.add_member(route, &device).await?;
        member.join_channel(route).await?;
        owner.send(route, "before policy loss").await?;
        member.sync_route(route).await?;
    }
    activate_candidate(&service).await?;
    let mut marker = Role {
        id: id(),
        name: "policy marker".into(),
        color: "#83bdd9".into(),
        description: String::new(),
        priority: 10,
        permissions: [].into(),
        mentionable: false,
    };
    owner
        .change_role_policy(&request(
            0,
            RoleChange::PutRole {
                role: marker.clone(),
            },
        ))
        .await?;
    owner
        .change_role_policy(&request(
            1,
            RoleChange::AssignRole {
                account_id: account.clone(),
                role_id: marker.id.clone(),
            },
        ))
        .await?;
    let overrides: cords_protocol::roles::Overrides = [(
        marker.id.clone(),
        cords_protocol::roles::Override {
            allow: [].into(),
            deny: [Permission::ReadMessages].into(),
        },
    )]
    .into();
    sqlx::query("UPDATE channel_access_policy SET overrides=$1 WHERE channel_id=$2")
        .bind(json(&overrides)?)
        .bind(&affected)
        .execute(service.0.store.pool())
        .await?;
    marker.name = "changed marker".into();
    owner
        .change_role_policy(&request(2, RoleChange::PutRole { role: marker }))
        .await?;
    assert_eq!(member.role_catalog().await?.revision, 3);
    assert!(
        owner
            .effective_actions(None)
            .await?
            .permissions
            .contains(&Permission::ManageRoles)
    );
    assert_eq!(member.account_roles(&account).await?.role_ids.len(), 1);
    let actions = member.effective_actions(None).await?;
    assert!(actions.active && actions.channel_id.is_none());
    assert!(!actions.permissions.contains(&Permission::ManageRoles));
    assert!(
        actions
            .permissions
            .iter()
            .all(|permission| !permission.channel_scoped())
    );
    let scoped = member.effective_actions(Some(&affected)).await?;
    assert!(!scoped.permissions.contains(&Permission::ReadMessages));
    assert!(!scoped.permissions.contains(&Permission::CommitMls));
    assert!(member.sync_route(&affected).await.is_err());
    owner
        .send(&unaffected, "unaffected route continues")
        .await?;
    member.sync_route(&unaffected).await?;
    let blocked = owner
        .send(&affected, "must not enter old epoch")
        .await
        .err()
        .context("missing send fence")?;
    assert!(
        format!("{blocked:#}").contains("CORDS_CHANNEL_REMOVAL_PENDING"),
        "unexpected send failure: {blocked:#}"
    );
    // A definite refusal must not leave an outbox item blocking control work.
    let removals = owner.pending_removals(&affected).await?;
    assert!(matches!(
        removals.as_slice(),
        [cords_protocol::messaging::PolicyRemoval::ServerPolicy { .. }]
    ));
    owner.remove_member(&affected, &device).await?;
    assert!(owner.pending_removals(&affected).await?.is_empty());
    let stale = owner
        .retry_last()
        .await
        .err()
        .context("stale MLS ciphertext was accepted")?;
    assert!(format!("{stale:#}").contains("CORDS_MLS_EPOCH_CONFLICT"));
    owner.send(&affected, "protected new epoch").await?;
    // Regrant restores account eligibility, never the removed leaf or old history.
    sqlx::query("UPDATE channel_access_policy SET overrides='{}' WHERE channel_id=$1")
        .bind(&affected)
        .execute(service.0.store.pool())
        .await?;
    assert!(member.sync_route(&affected).await.is_err());
    member.shutdown().await;
    owner.shutdown().await;
    let mut owner = Client::open(
        &root.path().join("owner"),
        &migrations,
        Some(pass),
        Some(&ca),
    )
    .await?;
    let mut member = Client::open(
        &root.path().join("member"),
        &migrations,
        Some(pass),
        Some(&ca),
    )
    .await?;
    sqlx::query("UPDATE role_policy SET revision=2 WHERE singleton=TRUE")
        .execute(service.0.store.pool())
        .await?;
    let rollback = member
        .role_catalog()
        .await
        .err()
        .context("known policy rollback was accepted after restart")?;
    assert!(format!("{rollback:#}").contains("CORDS_ROLE_POLICY_ROLLBACK"));
    sqlx::query("UPDATE role_policy SET revision=3,active=FALSE WHERE singleton=TRUE")
        .execute(service.0.store.pool())
        .await?;
    let downgrade = owner
        .role_catalog()
        .await
        .err()
        .context("enforcement downgrade was accepted")?;
    assert!(format!("{downgrade:#}").contains("CORDS_ROLE_POLICY_DOWNGRADE"));
    sqlx::query("UPDATE role_policy SET active=TRUE WHERE singleton=TRUE")
        .execute(service.0.store.pool())
        .await?;
    assert!(member.sync_route(&affected).await.is_err());
    owner.send(&affected, "restart preserves exclusion").await?;
    member.publish_key_package().await?;
    owner.add_member(&affected, &device).await?;
    member.join_channel(&affected).await?;
    let resumed = owner
        .send(&affected, "fresh enrollment resumes delivery")
        .await?;
    let received = member.sync_route(&affected).await?;
    assert!(
        received
            .messages
            .iter()
            .any(|message| message.message_id == resumed)
    );
    let excluded: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM channel_device_exclusions WHERE channel_id=$1 AND device_id=$2)")
        .bind(&affected).bind(&device).fetch_one(service.0.store.pool()).await?;
    assert!(!excluded);
    // Explicitly delegated group management can remove the owner's leaf without
    // changing the owner's independent policy/recovery authority.
    let manager = Role {
        id: id(),
        name: "group manager".into(),
        color: "#123456".into(),
        description: String::new(),
        priority: 20,
        permissions: [Permission::ManageChannel, Permission::ManageChannelMembers].into(),
        mentionable: false,
    };
    owner
        .change_role_policy(&request(
            3,
            RoleChange::PutRole {
                role: manager.clone(),
            },
        ))
        .await?;
    owner
        .change_role_policy(&request(
            4,
            RoleChange::AssignRole {
                account_id: account.clone(),
                role_id: manager.id,
            },
        ))
        .await?;
    // A requester can disappear after reserving a removal. Any other current
    // committer receives immutable, server-signed evidence and can finish it.
    let mut tx = service.0.store.pool().begin().await?;
    let abandoned = lifecycle::reserve_removal(
        &mut tx,
        &affected,
        &device,
        &owner.status().device_id,
        owner
            .cached_channels()
            .into_iter()
            .find(|channel| channel.channel_id == affected)
            .context("affected route")?
            .epoch as i64,
    )
    .await?;
    tx.commit().await?;
    member
        .remove_member(&affected, &owner.status().device_id)
        .await?;
    assert!(member.pending_removals(&affected).await?.is_empty());
    let cancelled: bool = sqlx::query_scalar("SELECT cancelled FROM roster_operations WHERE id=$1")
        .bind(&abandoned.operation_id)
        .fetch_one(service.0.store.pool())
        .await?;
    assert!(cancelled);
    assert!(owner.sync_route(&affected).await.is_err());
    let mut default = owner
        .role_catalog()
        .await?
        .roles
        .into_iter()
        .find(|role| role.id == cords_protocol::roles::DEFAULT_ROLE)
        .context("default role")?;
    assert!(default.permissions.remove(&Permission::ReadMessages));
    owner
        .change_role_policy(&request(5, RoleChange::PutRole { role: default }))
        .await?;
    let candidates = owner.recovery_candidates("").await?;
    let candidate = candidates
        .iter()
        .find(|candidate| candidate.predecessor.value.channel_id == affected)
        .context("unrecoverable original metadata")?;
    assert!(!candidate.committer_available);
    assert!(
        candidates.iter().any(
            |candidate| candidate.predecessor.value.channel_id == unaffected
                && candidate.committer_available
        )
    );
    let successor = owner
        .recover_channel(candidate, "fresh recovered encrypted group")
        .await?;
    assert_ne!(successor, affected);
    owner.send(&successor, "fresh recovery keys").await?;
    assert!(member.join_channel(&successor).await.is_err());
    let archived = owner
        .archived_channel_history(&owner.status().server_id, &affected)
        .await?;
    assert!(!archived.is_empty());
    let old_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM welcomes WHERE channel_id=$1 AND device_id=$2")
            .bind(&successor)
            .bind(&device)
            .fetch_one(service.0.store.pool())
            .await?;
    assert_eq!(
        old_count, 0,
        "recovery must not transfer old membership or Welcome material"
    );
    // Restart the real router in separate OS processes against the same durable
    // schema and server identity. This is candidate-service process acceptance;
    // final packaged-server startup remains an independent gate.
    let address = tls.origin.replace("https://localhost:", "127.0.0.1:");
    owner.shutdown().await;
    member.shutdown().await;
    drop(tls);
    tokio::task::yield_now().await;
    let schema: String = sqlx::query_scalar("SELECT current_schema()")
        .fetch_one(service.0.store.pool())
        .await?;
    let process_config = root.path().join("server-process.json");
    let config = RecoveryProcessConfig {
        schema,
        identity: _directory.path().join("server.key"),
        address,
        certificate: certificate.cert.der().to_vec(),
        key: certificate.signing_key.serialize_der(),
    };
    std::fs::write(&process_config, serde_json::to_vec(&config)?)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&process_config, std::fs::Permissions::from_mode(0o600))?;
    }
    let mut server = RecoveryProcess::start(&process_config).await?;
    let mut owner = Client::open(
        &root.path().join("owner"),
        &migrations,
        Some(pass),
        Some(&ca),
    )
    .await?;
    owner
        .send(&successor, "durable fresh group after restart")
        .await?;
    owner.shutdown().await;
    server.stop()?;
    let mut server = RecoveryProcess::start(&process_config).await?;
    let mut owner = Client::open(
        &root.path().join("owner"),
        &migrations,
        Some(pass),
        Some(&ca),
    )
    .await?;
    assert!(owner.effective_actions(None).await?.active);
    owner
        .send(&successor, "delivery after separate server-process restart")
        .await?;
    assert!(owner.pending_removals(&successor).await?.is_empty());
    assert!(
        !owner
            .archived_channel_history(&owner.status().server_id, &affected)
            .await?
            .is_empty()
    );
    let audit: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM channel_recovery_audit WHERE predecessor_id=$1 AND successor_id=$2",
    )
    .bind(&affected)
    .bind(&successor)
    .fetch_one(service.0.store.pool())
    .await?;
    assert_eq!(audit, 1);
    owner.shutdown().await;
    server.stop()?;
    Ok(())
}

#[derive(serde::Serialize, serde::Deserialize)]
struct RecoveryProcessConfig {
    schema: String,
    identity: std::path::PathBuf,
    address: String,
    certificate: Vec<u8>,
    key: Vec<u8>,
}

struct RecoveryProcess(Option<std::process::Child>);
impl Drop for RecoveryProcess {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            // Panic cleanup only. The successful path uses checked stop/reap.
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
impl RecoveryProcess {
    async fn start(config: &std::path::Path) -> Result<Self> {
        let ready = config.with_extension("ready");
        if ready.exists() {
            std::fs::remove_file(&ready)?;
        }
        let log_path = config.with_extension("log");
        let log = std::fs::File::create(&log_path)?;
        let child = std::process::Command::new(std::env::current_exe()?)
            .args([
                "--exact",
                "messaging::tests::enforced_roles::role_recovery_process_fixture",
                "--ignored",
                "--nocapture",
            ])
            .env("CORDS_ROLE_CHILD_CONFIG", config)
            .stdout(log.try_clone()?)
            .stderr(log)
            .spawn()?;
        let mut server = Self(Some(child));
        for _ in 0..200 {
            if ready.exists() {
                return Ok(server);
            }
            if let Some(status) = server
                .0
                .as_mut()
                .context("missing owned child")?
                .try_wait()?
            {
                anyhow::bail!(
                    "Candidate server child exited {status}; inspect {}",
                    log_path.display()
                );
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        anyhow::bail!(
            "Candidate server child did not become ready; inspect {}",
            log_path.display()
        );
    }
    fn stop(&mut self) -> Result<()> {
        let child = self.0.as_mut().context("owned server already stopped")?;
        child
            .kill()
            .context("stop owned candidate server process")?;
        child
            .wait()
            .context("reap owned candidate server process")?;
        self.0.take();
        Ok(())
    }
}

#[tokio::test]
#[ignore = "manual child-process harness; invoked only by the independent TLS/MLS test"]
async fn role_recovery_process_fixture() -> Result<()> {
    let path = std::path::PathBuf::from(
        std::env::var("CORDS_ROLE_CHILD_CONFIG")
            .context("manual child fixture requires its private configuration")?,
    );
    let config: RecoveryProcessConfig = serde_json::from_slice(&std::fs::read(&path)?)?;
    anyhow::ensure!(
        config.schema.starts_with("cords_test_")
            && config
                .schema
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_'),
        "invalid disposable schema"
    );
    let url = std::env::var("CORDS_TEST_DATABASE_URL")?;
    let options = sqlx::postgres::PgConnectOptions::from_str(&url)?
        .options([("search_path", config.schema.as_str())]);
    let store = PostgresStore::from_pool(
        sqlx::postgres::PgPoolOptions::new()
            .connect_with(options)
            .await?,
    );
    let (identity, created) = crate::ServerIdentity::load_or_create(&config.identity)?;
    anyhow::ensure!(
        !created,
        "candidate child must reuse the existing server identity"
    );
    store.bind_server_identity(&identity.server_id()).await?;
    let service = Service::with_policy(store, identity, 60, 900, JoinPolicy::Public)?;
    let mut tls =
        https_fixture_der(&service, config.certificate, config.key, &config.address).await?;
    std::fs::write(path.with_extension("ready"), std::process::id().to_string())?;
    (&mut tls.task).await??;
    anyhow::bail!("candidate child listener unexpectedly ended")
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn delegated_creation_uses_current_policy_and_initializes_restricted_boundary() -> Result<()>
{
    let (service, _directory) = fixture().await?;
    let (owner_id, owner_contact) = identity()?;
    let owner =
        grant_fixture_owner(&service, join(&service, &owner_id, owner_contact).await?).await?;
    let (delegate_id, delegate_contact) = identity()?;
    let delegate = join(&service, &delegate_id, delegate_contact).await?;
    activate_candidate(&service).await?;
    let role = Role {
        id: id(),
        name: "channel creator".into(),
        color: "#123456".into(),
        description: String::new(),
        priority: 10,
        permissions: [Permission::CreateChannels].into(),
        mentionable: false,
    };
    change(
        &service,
        &owner,
        request(0, RoleChange::PutRole { role: role.clone() }),
    )
    .await?;
    let creation = ChannelCreate {
        name: "delegated encrypted channel".into(),
        confidentiality_mode: ConfidentialityMode::Encrypted,
        idempotency_key: id(),
    };
    assert!(
        create_channel(
            State(service.clone()),
            headers(&delegate)?,
            Json(creation.clone())
        )
        .await
        .is_err()
    );
    change(
        &service,
        &owner,
        request(
            1,
            RoleChange::AssignRole {
                account_id: delegate.membership.value.account_id.clone(),
                role_id: role.id.clone(),
            },
        ),
    )
    .await?;
    let route = create_channel(
        State(service.clone()),
        headers(&delegate)?,
        Json(creation.clone()),
    )
    .await?
    .0;
    assert_eq!(
        create_channel(
            State(service.clone()),
            headers(&delegate)?,
            Json(creation.clone())
        )
        .await?
        .0,
        route
    );
    let restricted: bool = sqlx::query_scalar("SELECT account_restricted AND EXISTS(SELECT 1 FROM channel_account_access a WHERE a.channel_id=p.channel_id AND a.account_id=$2) FROM channel_access_policy p WHERE channel_id=$1")
        .bind(&route).bind(&delegate.membership.value.account_id).fetch_one(service.0.store.pool()).await?;
    assert!(restricted);
    assert!(
        channel(
            State(service.clone()),
            headers(&delegate)?,
            Path(route.clone()),
            Query(ChannelQuery { epoch: None })
        )
        .await
        .is_ok()
    );
    assert!(
        channel(
            State(service.clone()),
            headers(&owner)?,
            Path(route.clone()),
            Query(ChannelQuery { epoch: None })
        )
        .await
        .is_err()
    );
    change(
        &service,
        &owner,
        request(
            2,
            RoleChange::RevokeRole {
                account_id: delegate.membership.value.account_id.clone(),
                role_id: role.id,
            },
        ),
    )
    .await?;
    assert!(
        create_channel(State(service.clone()), headers(&delegate)?, Json(creation))
            .await
            .is_err()
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM channels")
        .fetch_one(service.0.store.pool())
        .await?;
    assert_eq!(count, 1);
    Ok(())
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn role_workspace_actions_track_current_actor_target_and_counts() -> Result<()> {
    use cords_protocol::roles::{ADMIN_ROLE, DEFAULT_ROLE};
    let (service, _directory) = fixture().await?;
    let (owner_id, contact) = identity()?;
    let session = join(&service, &owner_id, contact).await?;
    let owner = grant_fixture_owner(&service, session).await?;
    let (delegate_id, contact) = identity()?;
    let delegate = join(&service, &delegate_id, contact).await?;
    let (target_id, contact) = identity()?;
    let target = join(&service, &target_id, contact).await?;
    activate_candidate(&service).await?;
    let (delegator, marker) = establish_delegation(&service, &owner, &delegate_id).await?;
    let key = encode(service.0.identity.signing_key.verifying_key().to_bytes());
    let signed = role_workspace::workspace(State(service.clone()), headers(&delegate)?)
        .await?
        .0;
    signed.verify(&key)?;
    let metadata = signed.value;
    assert_eq!(metadata.account_id, delegate.membership.value.account_id);
    assert_eq!(metadata.device_id, delegate.membership.value.device_id);
    assert_eq!(metadata.catalog.revision, 3);
    assert_eq!(metadata.counts[DEFAULT_ROLE], 3);
    assert_eq!(metadata.counts[&delegator.id], 1);
    assert!(metadata.editable.is_empty() && metadata.deletable.is_empty() && !metadata.can_create);
    let account = target_id.authorization.value.account_id.clone();
    let before = role_workspace::target(
        State(service.clone()),
        headers(&delegate)?,
        Path(account.clone()),
    )
    .await?
    .0;
    before.verify(&key)?;
    assert!(before.value.eligible);
    assert!(before.value.assignable.contains(&marker.id));
    assert!(!before.value.assignable.contains(ADMIN_ROLE));
    assert!(before.value.removable.is_empty());
    let available = role_workspace::members(
        State(service.clone()),
        headers(&delegate)?,
        Path(marker.id.clone()),
        Query(role_workspace::MemberPage {
            after: String::new(),
            assigned: false,
        }),
    )
    .await?
    .0;
    available.verify(&key)?;
    assert_eq!(available.value.member_count, 0);
    assert_eq!(available.value.members.len(), 3);
    for row in &available.value.members {
        assert_eq!(row.can_assign, row.account_id == account);
        assert!(!row.can_remove);
    }
    for protected in [
        &owner_id.authorization.value.account_id,
        &delegate_id.authorization.value.account_id,
    ] {
        let hints = role_workspace::target(
            State(service.clone()),
            headers(&delegate)?,
            Path(protected.clone()),
        )
        .await?
        .0
        .value;
        assert!(hints.assignable.is_empty() && hints.removable.is_empty());
    }
    change(
        &service,
        &delegate,
        request(
            3,
            RoleChange::AssignRole {
                account_id: account.clone(),
                role_id: marker.id.clone(),
            },
        ),
    )
    .await?;
    let after = role_workspace::target(
        State(service.clone()),
        headers(&delegate)?,
        Path(account.clone()),
    )
    .await?
    .0
    .value;
    assert_eq!(after.catalog.revision, 4);
    assert!(after.removable.contains(&marker.id) && !after.assignable.contains(&marker.id));
    let assigned = role_workspace::members(
        State(service.clone()),
        headers(&delegate)?,
        Path(marker.id.clone()),
        Query(role_workspace::MemberPage {
            after: String::new(),
            assigned: true,
        }),
    )
    .await?
    .0
    .value;
    assert_eq!(assigned.member_count, 1);
    assert_eq!(assigned.members.len(), 1);
    assert_eq!(assigned.members[0].account_id, account);
    assert!(assigned.members[0].can_remove && !assigned.members[0].can_assign);
    let ordinary_page = role_workspace::members(
        State(service.clone()),
        headers(&target)?,
        Path(marker.id.clone()),
        Query(role_workspace::MemberPage {
            after: String::new(),
            assigned: true,
        }),
    )
    .await?
    .0
    .value;
    assert!(
        ordinary_page
            .members
            .iter()
            .all(|row| !row.can_assign && !row.can_remove)
    );
    let own = role_workspace::workspace(State(service.clone()), headers(&owner)?)
        .await?
        .0
        .value;
    assert_eq!(own.counts[&marker.id], 1);
    assert!(
        own.editable.contains(&marker.id) && own.deletable.contains(&marker.id) && own.can_create
    );
    // An ordinary member receives the same current labels/counts, no authority.
    let ordinary = role_workspace::workspace(State(service.clone()), headers(&target)?)
        .await?
        .0
        .value;
    assert!(ordinary.editable.is_empty() && ordinary.deletable.is_empty() && !ordinary.can_create);
    change(
        &service,
        &owner,
        request(
            4,
            RoleChange::RevokeRole {
                account_id: delegate_id.authorization.value.account_id.clone(),
                role_id: delegator.id,
            },
        ),
    )
    .await?;
    let revoked = role_workspace::target(
        State(service.clone()),
        headers(&delegate)?,
        Path(account.clone()),
    )
    .await?
    .0
    .value;
    assert_eq!(revoked.catalog.revision, 5);
    assert!(revoked.assignable.is_empty() && revoked.removable.is_empty());
    // Invalidated device evidence cannot grant new authority. Existing admitted
    // account assignments nevertheless remain visible and removable by staff.
    sqlx::query("UPDATE device_contacts SET contact=jsonb_set(contact::jsonb,'{authorization,value,expires_at}',to_jsonb(1::BIGINT))::TEXT WHERE device_id=$1")
        .bind(&target.membership.value.device_id).execute(service.0.store.pool()).await?;
    let expired = role_workspace::members(
        State(service.clone()),
        headers(&owner)?,
        Path(marker.id.clone()),
        Query(role_workspace::MemberPage {
            after: String::new(),
            assigned: true,
        }),
    )
    .await?
    .0
    .value;
    assert_eq!(expired.member_count, 1);
    assert_eq!(expired.members.len(), 1);
    assert!(expired.members[0].can_remove && !expired.members[0].can_assign);
    sqlx::query("UPDATE memberships SET active=FALSE WHERE device_id=$1")
        .bind(&target.membership.value.device_id)
        .execute(service.0.store.pool())
        .await?;
    let departed = role_workspace::target(State(service.clone()), headers(&owner)?, Path(account))
        .await?
        .0
        .value;
    assert!(!departed.eligible && departed.assignable.is_empty());
    assert!(departed.removable.contains(&marker.id));
    let counts = role_workspace::workspace(State(service.clone()), headers(&owner)?)
        .await?
        .0
        .value
        .counts;
    assert_eq!(counts[DEFAULT_ROLE], 2);
    assert_eq!(counts[&marker.id], 0);
    let audit: i64 = sqlx::query_scalar("SELECT count(*) FROM role_policy_changes")
        .fetch_one(service.0.store.pool())
        .await?;
    assert_eq!(
        audit, 5,
        "presentation reads never write policy audit entries"
    );
    Ok(())
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn role_member_pages_are_bounded_complete_and_default_is_implicit() -> Result<()> {
    use cords_protocol::roles::{DEFAULT_ROLE, ROLE_MEMBERS_PAGE_SIZE};
    let (service, _directory) = fixture().await?;
    let (owner_id, contact) = identity()?;
    let owner = grant_fixture_owner(&service, join(&service, &owner_id, contact).await?).await?;
    let mut expected = std::collections::BTreeSet::from([owner_id.authorization.value.account_id]);
    let mut last_device = String::new();
    for _ in 0..18 {
        let (identity, contact) = identity()?;
        expected.insert(identity.authorization.value.account_id.clone());
        last_device = identity.authorization.value.device_id.clone();
        join(&service, &identity, contact).await?;
    }
    activate_candidate(&service).await?;
    let key = encode(service.0.identity.signing_key.verifying_key().to_bytes());
    let mut after = String::new();
    let mut seen = std::collections::BTreeSet::new();
    let mut sizes = Vec::new();
    loop {
        let signed = role_workspace::members(
            State(service.clone()),
            headers(&owner)?,
            Path(DEFAULT_ROLE.into()),
            Query(role_workspace::MemberPage {
                after: after.clone(),
                assigned: true,
            }),
        )
        .await?
        .0;
        signed.verify(&key)?;
        let page = signed.value;
        assert_eq!(page.after, after);
        assert_eq!(page.member_count, 19);
        assert!(page.members.len() <= ROLE_MEMBERS_PAGE_SIZE);
        sizes.push(page.members.len());
        for row in page.members {
            assert!(row.account_id > after && row.assigned && !row.can_assign && !row.can_remove);
            assert!(seen.insert(row.account_id));
        }
        match page.next_after {
            Some(next) => {
                assert!(next > after);
                after = next;
            }
            None => break,
        }
    }
    assert_eq!(sizes, [16, 3]);
    assert_eq!(seen, expected);
    let unassigned = role_workspace::members(
        State(service.clone()),
        headers(&owner)?,
        Path(DEFAULT_ROLE.into()),
        Query(role_workspace::MemberPage {
            after: String::new(),
            assigned: false,
        }),
    )
    .await?
    .0
    .value;
    assert!(unassigned.members.is_empty() && unassigned.next_after.is_none());
    assert_eq!(unassigned.member_count, 19);
    // Oversized per-account contact sets must fail before deserializing them.
    sqlx::query("UPDATE device_contacts SET contact=repeat('x',4194305) WHERE device_id=$1")
        .bind(&last_device)
        .execute(service.0.store.pool())
        .await?;
    let account: String =
        sqlx::query_scalar("SELECT account_id FROM device_contacts WHERE device_id=$1")
            .bind(last_device)
            .fetch_one(service.0.store.pool())
            .await?;
    let error = role_workspace::target(State(service.clone()), headers(&owner)?, Path(account))
        .await
        .err()
        .context("bounded contact refusal")?;
    assert_eq!(error.0, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(error.1, "CORDS_ROLE_TARGET_DEVICE_LIMIT");
    Ok(())
}

#[tokio::test]
#[ignore = "manual real Qt role acceptance; requires explicit fixture directory and stop file"]
async fn native_role_management_fixture() -> Result<()> {
    use cords_client_core::accounts::AccountRegistry;
    let root = std::path::PathBuf::from(std::env::var("CORDS_NATIVE_ROLE_FIXTURE")?);
    anyhow::ensure!(
        root.is_absolute() && !root.exists(),
        "fixture must be new and absolute"
    );
    std::fs::create_dir(&root)?;
    let (service, _directory, code) = fixture_with_state(false).await?;
    activate_candidate(&service).await?;
    let certificate = rcgen::generate_simple_self_signed(vec!["localhost".into()])?;
    let tls = https_fixture(&service, &certificate).await?;
    let ca = root.join("ca.pem");
    std::fs::write(&ca, certificate.cert.pem())?;
    let migrations =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../migrations/sqlite");
    let owner_registry = AccountRegistry::open(&root.join("owner"), &migrations, Some(&ca)).await?;
    let member_registry =
        AccountRegistry::open(&root.join("member"), &migrations, Some(&ca)).await?;
    let other_registry = AccountRegistry::open(&root.join("other"), &migrations, Some(&ca)).await?;
    let password = "Native role fixture only 9841!";
    let mut owner = owner_registry.create("Role owner", password, false).await?;
    let mut member = member_registry
        .create("Role member", password, false)
        .await?;
    let mut other = other_registry
        .create("Other role member", password, false)
        .await?;
    owner.trust(&tls.origin).await?;
    owner.claim_ownership(&code).await?;
    member.trust(&tls.origin).await?;
    member.authenticate().await?;
    other.trust(&tls.origin).await?;
    other.authenticate().await?;
    let role = Role {
        id: id(),
        name: "Live native guides".into(),
        color: "#88bbcc".into(),
        description: String::new(),
        priority: 20,
        permissions: Default::default(),
        mentionable: false,
    };
    owner
        .change_role_policy(&request(0, RoleChange::PutRole { role: role.clone() }))
        .await?;
    let owner_account = owner.status().account_id;
    let member_account = member.status().account_id;
    let other_account = other.status().account_id;
    owner.shutdown().await;
    member.shutdown().await;
    other.shutdown().await;
    std::fs::write(
        root.join("ready.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "fixture":"native-roles-v1", "owner_state":root.join("owner"), "member_state":root.join("member"),
            "owner_account":owner_account,"member_account":member_account,"other_account":other_account,"ca":ca,"role":role.id
        }))?,
    )?;
    println!("Native role fixture ready at {}", root.display());
    tokio::time::timeout(Duration::from_mins(15), async {
        while !root.join("stop").exists() {
            anyhow::ensure!(!tls.task.is_finished(), "role fixture TLS listener stopped");
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
        Ok::<_, anyhow::Error>(())
    })
    .await
    .context("native role fixture timed out")??;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM account_roles WHERE role_id=$1")
        .bind(&role.id)
        .fetch_one(service.0.store.pool())
        .await?;
    anyhow::ensure!(count == 0, "native profile removal was not durable");
    let audits: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM role_policy_changes WHERE operation IN ('assign_role','revoke_role')",
    )
    .fetch_one(service.0.store.pool())
    .await?;
    anyhow::ensure!(
        audits == 4,
        "expected two native assignments and two removals, got {audits}"
    );
    Ok(())
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL and real native workers over TLS"]
async fn native_role_target_controls_use_fresh_enforced_admission_and_live_metadata() -> Result<()>
{
    let (service, _directory, code) = fixture_with_state(false).await?;
    activate_candidate(&service).await?;
    let certificate = rcgen::generate_simple_self_signed(vec!["localhost".into()])?;
    let tls = https_fixture(&service, &certificate).await?;
    let root = tempfile::tempdir()?;
    let ca = root.path().join("ca.pem");
    std::fs::write(&ca, certificate.cert.pem())?;
    let origin = tls.origin.clone();
    let role_id = id();
    let expected_role = role_id.clone();
    tokio::task::spawn_blocking(move || -> Result<()> {
        use cords_qt_bridge::{Desktop, ffi, start_desktop};
        fn wait(desktop: &Desktop, completed: u64, allow_error: bool) -> Result<ffi::Snapshot> {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(40);
            while std::time::Instant::now() < deadline {
                for snapshot in desktop.take_updates()? {
                    if snapshot.completed >= completed {
                        anyhow::ensure!(
                            allow_error || snapshot.error.is_empty(),
                            "native operation {completed}: {}",
                            snapshot.error
                        );
                        return Ok(snapshot);
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            anyhow::bail!("native role operation {completed} timed out")
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
        wait(&owner, 0, false)?;
        wait(&member, 0, false)?;
        let password = "Native role network acceptance 5932!";
        owner.create_account("Owner".into(), password.into(), false)?;
        wait(&owner, 1, false)?;
        member.create_account("Member".into(), password.into(), false)?;
        let account = wait(&member, 1, false)?.account;
        owner.trust_server(origin.clone())?;
        wait(&owner, 2, false)?;
        owner.claim_ownership(code.to_string())?;
        wait(&owner, 3, false)?;
        member.trust_server(origin)?;
        wait(&member, 2, false)?;
        member.authenticate()?;
        assert_eq!(wait(&member, 3, false)?.admission, "active");
        owner.load_roles()?;
        let catalog = wait(&owner, 4, false)?;
        assert!(catalog.roles_active && catalog.can_create_roles);
        let role = || ffi::RoleRow {
            editable: false,
            deletable: false,
            member_count: 0,
            id: role_id.clone(),
            name: "Native guides".into(),
            color: "#88bbcc".into(),
            description: String::new(),
            priority: 20,
            permissions: vec![],
            mentionable: false,
        };
        owner.put_role(role(), catalog.revision, id())?;
        let created = wait(&owner, 5, false)?;
        assert_eq!(created.revision, 1);
        owner.load_account_roles(account.clone())?;
        let target = wait(&owner, 6, false)?.role_target;
        assert!(target.loaded && target.assignable.contains(&role_id));
        owner.assign_role(
            account.clone(),
            role_id.clone(),
            false,
            target.revision,
            id(),
        )?;
        let assigned = wait(&owner, 7, false)?;
        assert!(assigned.role_target.assigned.contains(&role_id));
        assert!(assigned.role_target.removable.contains(&role_id));
        assert_eq!(
            assigned
                .roles
                .iter()
                .find(|row| row.id == role_id)
                .context("counted role")?
                .member_count,
            1
        );
        member.load_account_roles(account.clone())?;
        let read_only = wait(&member, 4, false)?.role_target;
        assert!(read_only.assigned.contains(&role_id));
        assert!(read_only.assignable.is_empty() && read_only.removable.is_empty());
        owner.load_profile(account.clone())?;
        let profile = wait(&owner, 8, false)?;
        assert_eq!(profile.profile.account, account);
        assert_eq!(profile.role_target.account, account);
        owner.assign_role(
            account.clone(),
            role_id.clone(),
            true,
            profile.revision,
            id(),
        )?;
        let removed = wait(&owner, 9, false)?;
        assert!(removed.role_target.assigned.is_empty());
        assert_eq!(
            removed
                .roles
                .iter()
                .find(|row| row.id == role_id)
                .context("removed role count")?
                .member_count,
            0
        );
        member.load_roles()?;
        wait(&member, 5, false)?;
        let mut renamed = role();
        renamed.name = "Live updated guides".into();
        owner.put_role(renamed, removed.revision, id())?;
        wait(&owner, 10, false)?;
        // No explicit member refresh: the existing unlocked worker must observe
        // external catalog changes and target actions within its refresh bound.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(45);
        let mut observed = false;
        while std::time::Instant::now() < deadline && !observed {
            for snapshot in member.take_updates()? {
                observed |= snapshot
                    .roles
                    .iter()
                    .any(|row| row.id == role_id && row.name == "Live updated guides")
                    && snapshot.role_target.loaded
                    && snapshot.role_target.assigned.is_empty();
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        anyhow::ensure!(
            observed,
            "member did not receive live role metadata without reopening profile"
        );
        owner.load_role_members(role_id.clone(), false, String::new())?;
        let available = wait(&owner, 11, false)?.role_members;
        assert!(available.loaded && !available.assigned && available.next_after.is_empty());
        assert_eq!(available.role, role_id);
        assert_eq!(available.member_count, 0);
        assert_eq!(available.rows.len(), 2);
        for row in &available.rows {
            assert_eq!(row.can_assign, row.account == account);
            assert!(!row.can_remove);
        }
        owner.load_role_members(role_id.clone(), true, String::new())?;
        let assigned = wait(&owner, 12, false)?.role_members;
        assert!(assigned.loaded && assigned.assigned && assigned.rows.is_empty());
        owner.load_role_members(
            cords_protocol::roles::DEFAULT_ROLE.into(),
            true,
            String::new(),
        )?;
        let default = wait(&owner, 13, false)?.role_members;
        assert_eq!(default.member_count, 2);
        assert!(
            default
                .rows
                .iter()
                .all(|row| !row.can_assign && !row.can_remove)
        );
        member.assign_role(account, role_id, false, 4, id())?;
        let denied = wait(&member, 6, true)?.error;
        assert!(
            !denied.is_empty(),
            "ordinary account must not grant itself a role"
        );
        owner.lock_account(false)?;
        let owner_locked = wait(&owner, 14, false)?;
        assert!(!owner_locked.role_members.loaded && owner_locked.role_members.rows.is_empty());
        member.lock_account(false)?;
        let locked = wait(&member, 7, true)?;
        assert!(!locked.unlocked && !locked.role_target.loaded);
        assert!(
            locked.error.is_empty() || locked.error == denied,
            "locking returned a new unexpected failure: {}",
            locked.error
        );
        Ok(())
    })
    .await??;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM account_roles WHERE role_id=$1")
        .bind(expected_role)
        .fetch_one(service.0.store.pool())
        .await?;
    assert_eq!(count, 0);
    let audits: i64 = sqlx::query_scalar("SELECT count(*) FROM role_policy_changes")
        .fetch_one(service.0.store.pool())
        .await?;
    assert_eq!(audits, 4);
    Ok(())
}
