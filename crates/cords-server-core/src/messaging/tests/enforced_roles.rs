//! The candidate remains isolated from automatic startup until all paths enforce it.
use super::*;
use cords_protocol::roles::{Permission, Role, RoleChange, RoleChangeRequest};

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
