//! The candidate remains isolated from automatic startup until all paths enforce it.
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
    assert_eq!(
        discovered.len(),
        1,
        "view permission retains channel metadata"
    );
    assert!(
        discovered[0].members.is_empty(),
        "read-denied discovery must conceal the MLS roster"
    );
    assert!(discovered[0].binding.is_none());
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
        "../../../../../migrations/rbac/202610150001_enforced_rbac.sql"
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
    assert!(
        roles::catalog(State(service.clone()), headers(&delegate)?)
            .await
            .is_err()
    );
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

fn application(session: &Session, route: &str) -> Result<EventUpload> {
    // Relay authorization test only. The server neither decrypts this fixture nor
    // treats it as proof of cryptographic continuity (tested separately).
    Ok(EventUpload {
        protocol_version: 1,
        event_id: id(),
        route_kind: "channel".into(),
        route_id: route.into(),
        sender_member_id: session.membership.value.member_id.clone(),
        sender_device_id: session.membership.value.device_id.clone(),
        client_created_at: now()?,
        content_encoding: "mls.application".into(),
        ciphertext: encode(b"opaque authorization fixture"),
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
