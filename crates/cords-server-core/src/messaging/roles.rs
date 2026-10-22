//! Role administration. The normal preparatory schema remains owner-only;
//! the staged enforced schema derives actors from current durable policy.
use super::*;
use crate::authorization::{PolicyError, Subject, persistence, runtime};
use cords_protocol::roles::{
    AccountRoles, Permission, RoleCatalog, RoleChange, RoleChangeReceipt, RoleChangeRequest,
};

impl From<persistence::Error> for ApiError {
    fn from(error: persistence::Error) -> Self {
        match error {
            persistence::Error::Conflict => Self(StatusCode::CONFLICT, "CORDS_POLICY_CONFLICT"),
            persistence::Error::Replay => Self(StatusCode::CONFLICT, "CORDS_IDEMPOTENCY_CONFLICT"),
            persistence::Error::Uninitialized => {
                Self(StatusCode::CONFLICT, "CORDS_POLICY_UNINITIALIZED")
            }
            persistence::Error::Policy(PolicyError::Denied) => denied(),
            persistence::Error::Policy(_) => {
                Self(StatusCode::BAD_REQUEST, "CORDS_INVALID_ROLE_POLICY")
            }
            persistence::Error::Database(error) => error.into(),
            persistence::Error::Encoding(_) => {
                Self(StatusCode::SERVICE_UNAVAILABLE, "CORDS_POLICY_UNAVAILABLE")
            }
        }
    }
}

impl From<runtime::Error> for ApiError {
    fn from(error: runtime::Error) -> Self {
        match error {
            runtime::Error::Denied | runtime::Error::Policy(PolicyError::Denied) => denied(),
            runtime::Error::Repository(error) => error.into(),
            runtime::Error::Database(error) => error.into(),
            runtime::Error::NotReady | runtime::Error::Encoding(_) | runtime::Error::Policy(_) => {
                Self(StatusCode::SERVICE_UNAVAILABLE, "CORDS_POLICY_UNAVAILABLE")
            }
        }
    }
}

pub(super) async fn administrative_subject(
    service: &Service,
    tx: &mut Tx<'_>,
    member: &Membership,
    any_permission: &[Permission],
) -> Result<Subject> {
    let active: bool = sqlx::query_scalar("SELECT active FROM role_policy WHERE singleton=TRUE")
        .fetch_one(&mut **tx)
        .await?;
    if !active {
        // This branch is exclusively the pre-enforcement schema. Its assignments
        // remain preparatory and are archived before the staged schema activates.
        return owner_subject(tx, member).await;
    }
    let decision = runtime::load(
        tx,
        &member.device_id,
        &service.0.identity.server_id(),
        &encode(service.0.identity.signing_key.verifying_key().to_bytes()),
        now()?,
        None,
    )
    .await?;
    decision.require(Permission::ViewServer)?;
    if !any_permission.is_empty()
        && !any_permission
            .iter()
            .any(|permission| decision.permissions.contains(permission))
    {
        return Err(denied());
    }
    Ok(decision.subject)
}

async fn owner_subject(tx: &mut Tx<'_>, member: &Membership) -> Result<Subject> {
    // authorize_mutation holds the ownership/account locks before this read.
    let owner: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM server_ownership WHERE account_id=$1)")
            .bind(&member.account_id)
            .fetch_one(&mut **tx)
            .await?;
    let revoked: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM identity_burns WHERE account_id=$1) OR EXISTS(SELECT 1 FROM device_revocations WHERE device_id=$2)",
    ).bind(&member.account_id).bind(&member.device_id).fetch_one(&mut **tx).await?;
    if !owner || revoked {
        return Err(denied());
    }
    Ok(Subject {
        account_id: member.account_id.clone(),
        accepted: true,
        device_authorized: true,
        device_approved: false,
        revoked_or_burned: false,
        owner,
        roles: std::collections::BTreeSet::new(),
    })
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ActionScope {
    channel: Option<String>,
}

/// Signed presentation hints from the same evaluator used by mutations.
/// Neither this snapshot nor its revision is a reusable authorization credential.
pub(super) async fn actions(
    State(s): State<Service>,
    headers: HeaderMap,
    Query(scope): Query<ActionScope>,
) -> Result<Json<Signed<cords_protocol::roles::EffectiveActions>>> {
    if scope
        .channel
        .as_ref()
        .is_some_and(|id| uuid::Uuid::parse_str(id).is_err())
    {
        return Err(bad());
    }
    let member = s.authenticate(&headers, "channel.read").await?;
    let mut tx = s.0.store.pool().begin().await?;
    s.authorize_session(&mut tx, &headers, &member).await?;
    let active: bool = sqlx::query_scalar("SELECT active FROM role_policy WHERE singleton=TRUE")
        .fetch_one(&mut *tx)
        .await?;
    let (revision, permissions, owner) = if active {
        let decision = runtime::load(
            &mut tx,
            &member.value.device_id,
            &s.0.identity.server_id(),
            &encode(s.0.identity.signing_key.verifying_key().to_bytes()),
            now()?,
            scope.channel.as_deref(),
        )
        .await?;
        decision.require(Permission::ViewServer)?;
        (
            uint(decision.revision)?,
            decision.permissions,
            decision.subject.owner,
        )
    } else {
        // Legacy hints mirror the legacy capability boundary. Preparatory role
        // assignments cannot grant any permission and never enter this set.
        let owner: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM server_ownership WHERE account_id=$1)")
                .bind(&member.value.account_id)
                .fetch_one(&mut *tx)
                .await?;
        let revision = uint(persistence::load(&mut tx).await?.revision)?;
        let mut permissions: std::collections::BTreeSet<Permission> = member
            .value
            .capabilities
            .iter()
            .filter_map(|cap| serde_json::from_value(serde_json::Value::String(cap.clone())).ok())
            .collect();
        if owner {
            permissions.extend([
                Permission::ManageRoles,
                Permission::AssignRoles,
                Permission::ViewAudit,
            ]);
        }
        if let Some(route) = scope.channel.as_deref() {
            let row = sqlx::query("SELECT c.creator,c.confidentiality_mode,c.retired,EXISTS(SELECT 1 FROM channel_members m WHERE m.channel_id=c.id AND m.device_id=$2 AND m.active AND m.delivery_active) participating FROM channels c WHERE c.id=$1 FOR SHARE")
                .bind(route).bind(&member.value.device_id).fetch_optional(&mut *tx).await?.ok_or_else(denied)?;
            if row.try_get::<bool, _>("participating")?
                || row.try_get::<String, _>("confidentiality_mode")? == "public"
            {
                permissions.extend([Permission::ViewChannel, Permission::ReadMessages]);
                if row.try_get::<String, _>("creator")? == member.value.device_id
                    || permissions.contains(&Permission::ManageChannel)
                {
                    permissions.extend([
                        Permission::ManageChannel,
                        Permission::ManageChannelMembers,
                        Permission::CommitMls,
                    ]);
                }
            } else {
                permissions.retain(|permission| !permission.channel_scoped());
            }
            if row.try_get::<bool, _>("retired")? {
                permissions.remove(&Permission::SendMessages);
                permissions.remove(&Permission::CommitMls);
            }
        } else {
            permissions.retain(|permission| !permission.channel_scoped());
        }
        (revision, permissions, owner)
    };
    let response = s.sign(cords_protocol::roles::EffectiveActions {
        version: 1,
        active,
        channel_id: scope.channel,
        server_id: s.0.identity.server_id(),
        account_id: member.value.account_id,
        device_id: member.value.device_id,
        policy_revision: revision,
        permissions,
        owner,
    })?;
    tx.commit().await?;
    Ok(Json(response))
}

pub(super) async fn catalog(
    State(s): State<Service>,
    headers: HeaderMap,
) -> Result<Json<Signed<RoleCatalog>>> {
    let member = s.authenticate(&headers, "channel.read").await?;
    let mut tx = s.0.store.pool().begin().await?;
    s.authorize_mutation(&mut tx, &headers, &member, None)
        .await?;
    administrative_subject(&s, &mut tx, &member.value, &[]).await?;
    let snapshot = persistence::load(&mut tx).await?;
    let roles = snapshot.policy.map_or_else(Vec::new, |policy| {
        policy.roles().values().cloned().collect()
    });
    let response = s.sign(RoleCatalog {
        version: 1,
        server_id: s.0.identity.server_id(),
        revision: uint(snapshot.revision)?,
        active: snapshot.active,
        roles,
    })?;
    tx.commit().await?;
    Ok(Json(response))
}

pub(super) async fn change(
    State(s): State<Service>,
    headers: HeaderMap,
    Json(request): Json<RoleChangeRequest>,
) -> Result<Json<Signed<RoleChangeReceipt>>> {
    let member = s.authenticate(&headers, "channel.read").await?;
    let mut tx = s.0.store.pool().begin().await?;
    // Admission/revocation holds the target account before taking the policy
    // lock. Match that order so assignment cannot deadlock those transitions.
    if let RoleChange::AssignRole { account_id, .. } | RoleChange::RevokeRole { account_id, .. } =
        &request.change
    {
        if account_id.is_empty() || account_id.len() > 128 {
            return Err(bad());
        }
        sqlx::query("SELECT pg_advisory_xact_lock_shared(hashtextextended('server-ownership',0))")
            .execute(&mut *tx)
            .await?;
        sqlx::query("SELECT pg_advisory_xact_lock_shared(hashtextextended($1,0))")
            .bind(account_id)
            .execute(&mut *tx)
            .await?;
    }
    s.authorize_mutation(&mut tx, &headers, &member, None)
        .await?;
    let actor = administrative_subject(&s, &mut tx, &member.value, &[]).await?;
    let mut affected_routes = Vec::new();
    let revision = if let Some(revision) =
        persistence::committed_revision(&mut tx, &member.value.device_id, &request).await?
    {
        revision
    } else {
        let target = match &request.change {
            RoleChange::AssignRole { account_id, .. } => {
                Some(assignment_target(&mut tx, account_id).await?)
            }
            RoleChange::RevokeRole { account_id, .. } => {
                Some(revocation_target(&mut tx, account_id).await?)
            }
            _ => None,
        };
        let revision = persistence::apply(
            &mut tx,
            &actor,
            &member.value.device_id,
            target.as_ref(),
            &request,
        )
        .await?;
        affected_routes = reconcile_channel_access(&s, &mut tx, revision).await?;
        revision
    };
    let response = s.sign(RoleChangeReceipt {
        version: 1,
        server_id: s.0.identity.server_id(),
        revision: uint(revision)?,
        request_id: request.request_id.clone(),
        request_digest: hash(canonical(&request)?),
        actor_device_id: member.value.device_id,
    })?;
    // A prepared transaction is never acknowledged as committed.
    tx.commit().await?;
    for route in affected_routes {
        let _ = s.0.notifications.send((route, 0));
    }
    Ok(Json(response))
}

/// Role mutations can only subtract existing delivery here. A role grant never
/// creates an MLS leaf, clears an exclusion, or restores historical delivery.
async fn reconcile_channel_access(
    s: &Service,
    tx: &mut Tx<'_>,
    revision: i64,
) -> Result<Vec<String>> {
    let snapshot = persistence::load(tx).await?;
    if !snapshot.active {
        return Ok(Vec::new());
    }
    let policy = snapshot.policy.ok_or_else(denied)?;
    let mut after = (String::new(), String::new());
    let mut routes = std::collections::BTreeSet::new();
    loop {
        let rows = sqlx::query("SELECT c.id,c.epoch,p.account_restricted,p.overrides,COALESCE(g.overrides,'{}') category_overrides,m.device_id,d.account_id,EXISTS(SELECT 1 FROM server_ownership o WHERE o.account_id=d.account_id) owner,EXISTS(SELECT 1 FROM channel_account_access a WHERE a.channel_id=c.id AND a.account_id=d.account_id) included,ARRAY(SELECT role_id FROM account_roles a WHERE a.account_id=d.account_id ORDER BY role_id) roles FROM channels c JOIN channel_access_policy p ON p.channel_id=c.id LEFT JOIN channel_categories g ON g.id=p.category_id JOIN channel_members m ON m.channel_id=c.id JOIN device_contacts d ON d.device_id=m.device_id WHERE c.confidentiality_mode='encrypted' AND NOT c.retired AND m.active AND m.delivery_active AND (c.id,m.device_id)>($1,$2) ORDER BY c.id,m.device_id LIMIT 256")
            .bind(&after.0).bind(&after.1).fetch_all(&mut **tx).await?;
        if rows.is_empty() {
            break;
        }
        for row in rows {
            let route: String = row.try_get("id")?;
            let device: String = row.try_get("device_id")?;
            after = (route.clone(), device.clone());
            // This conservative subject tests role access only. Identity/admission
            // revocation is enforced separately; no positive grant is issued here.
            let target = Subject {
                account_id: row.try_get("account_id")?,
                accepted: true,
                device_authorized: true,
                device_approved: true,
                revoked_or_burned: false,
                owner: row.try_get("owner")?,
                roles: row
                    .try_get::<Vec<String>, _>("roles")?
                    .into_iter()
                    .collect(),
            };
            let category = parse(row.try_get("category_overrides")?)?;
            let channel = parse(row.try_get("overrides")?)?;
            let access = policy
                .access(&target, &category, &channel)
                .map_err(persistence::Error::from)?;
            if access.eligible
                && (!row.try_get::<bool, _>("account_restricted")?
                    || row.try_get::<bool, _>("included")?)
            {
                continue;
            }
            let proof = s.sign(cords_protocol::roles::ChannelAccessRemoval {
                version: 1,
                server_id: s.0.identity.server_id(),
                channel_id: route.clone(),
                account_id: target.account_id,
                device_id: device.clone(),
                policy_revision: uint(revision)?,
                channel_epoch: uint(row.try_get("epoch")?)?,
                issued_at: now()?,
            })?;
            sqlx::query("INSERT INTO pending_policy_removals(channel_id,device_id) VALUES($1,$2) ON CONFLICT DO NOTHING")
                .bind(&route).bind(&device).execute(&mut **tx).await?;
            sqlx::query("INSERT INTO channel_policy_removal_proofs(channel_id,device_id,proof) VALUES($1,$2,$3) ON CONFLICT DO NOTHING")
                .bind(&route).bind(&device).bind(json(&proof)?).execute(&mut **tx).await?;
            sqlx::query("UPDATE channel_members SET delivery_active=FALSE WHERE channel_id=$1 AND device_id=$2")
                .bind(&route).bind(&device).execute(&mut **tx).await?;
            routes.insert(route);
        }
    }
    Ok(routes.into_iter().collect())
}

pub(super) async fn account_roles(
    State(s): State<Service>,
    Path(account): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Signed<AccountRoles>>> {
    if account.is_empty() || account.len() > 128 {
        return Err(bad());
    }
    let member = s.authenticate(&headers, "channel.read").await?;
    let mut tx = s.0.store.pool().begin().await?;
    s.authorize_mutation(&mut tx, &headers, &member, None)
        .await?;
    administrative_subject(&s, &mut tx, &member.value, &[]).await?;
    let snapshot = persistence::load(&mut tx).await?;
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM account_heads WHERE account_id=$1)")
            .bind(&account)
            .fetch_one(&mut *tx)
            .await?;
    if !exists {
        return Err(denied());
    }
    let role_ids: Vec<String> = sqlx::query_scalar(
        "SELECT role_id FROM account_roles WHERE account_id=$1 ORDER BY role_id",
    )
    .bind(&account)
    .fetch_all(&mut *tx)
    .await?;
    let response = s.sign(AccountRoles {
        account_id: account,
        catalog: RoleCatalog {
            version: 1,
            server_id: s.0.identity.server_id(),
            revision: uint(snapshot.revision)?,
            active: snapshot.active,
            roles: snapshot.policy.map_or_else(Vec::new, |policy| {
                policy.roles().values().cloned().collect()
            }),
        },
        role_ids: role_ids.into_iter().collect(),
    })?;
    tx.commit().await?;
    Ok(Json(response))
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AuditQuery {
    #[serde(default)]
    after: u64,
}

pub(super) async fn audit(
    State(s): State<Service>,
    Query(query): Query<AuditQuery>,
    headers: HeaderMap,
) -> Result<Json<Signed<cords_protocol::roles::RoleAuditPage>>> {
    use cords_protocol::roles::{ROLE_AUDIT_PAGE_SIZE, RoleAuditEntry, RoleAuditPage};
    let member = s.authenticate(&headers, "channel.read").await?;
    let mut tx = s.0.store.pool().begin().await?;
    s.authorize_mutation(&mut tx, &headers, &member, None)
        .await?;
    administrative_subject(&s, &mut tx, &member.value, &[Permission::ViewAudit]).await?;
    let snapshot = persistence::load(&mut tx).await?;
    if query.after > uint(snapshot.revision)? {
        return Err(bad());
    }
    let rows = sqlx::query("SELECT revision,actor_device_id,operation,target,FLOOR(EXTRACT(EPOCH FROM committed_at))::BIGINT AS committed_at FROM role_policy_changes WHERE revision>$1 ORDER BY revision LIMIT $2")
        .bind(int(query.after)?).bind(i64::try_from(ROLE_AUDIT_PAGE_SIZE).map_err(|_| bad())?)
        .fetch_all(&mut *tx).await?;
    let entries = rows
        .iter()
        .map(|row| {
            Ok(RoleAuditEntry {
                revision: uint(row.try_get("revision")?)?,
                actor_device_id: row.try_get("actor_device_id")?,
                operation: row.try_get("operation")?,
                target: row.try_get("target")?,
                committed_at: uint(row.try_get("committed_at")?)?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let response = s.sign(RoleAuditPage {
        version: 1,
        server_id: s.0.identity.server_id(),
        after_revision: query.after,
        head_revision: uint(snapshot.revision)?,
        entries,
    })?;
    tx.commit().await?;
    Ok(Json(response))
}

// Only identifies a durable account for removal. These flags deliberately confer
// no admission; persistence reloads its roles before checking rank protections.
pub(super) async fn revocation_target(tx: &mut Tx<'_>, account: &str) -> Result<Subject> {
    if account.is_empty() || account.len() > 128 {
        return Err(bad());
    }
    sqlx::query("SELECT pg_advisory_xact_lock_shared(hashtextextended($1,0))")
        .bind(account)
        .execute(&mut **tx)
        .await?;
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM account_heads WHERE account_id=$1)")
            .bind(account)
            .fetch_one(&mut **tx)
            .await?;
    if !exists {
        return Err(denied());
    }
    let owner: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM server_ownership WHERE account_id=$1)")
            .bind(account)
            .fetch_one(&mut **tx)
            .await?;
    let burned: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM identity_burns WHERE account_id=$1)")
            .bind(account)
            .fetch_one(&mut **tx)
            .await?;
    Ok(Subject {
        account_id: account.into(),
        accepted: false,
        device_authorized: false,
        device_approved: false,
        revoked_or_burned: burned,
        owner,
        roles: std::collections::BTreeSet::new(),
    })
}

pub(super) async fn assignment_target(tx: &mut Tx<'_>, account: &str) -> Result<Subject> {
    if account.len() > 128 {
        return Err(bad());
    }
    sqlx::query("SELECT pg_advisory_xact_lock_shared(hashtextextended($1,0))")
        .bind(account)
        .execute(&mut **tx)
        .await?;
    let burned: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM identity_burns WHERE account_id=$1)")
            .bind(account)
            .fetch_one(&mut **tx)
            .await?;
    if burned {
        return Err(denied());
    }
    let active: bool = sqlx::query_scalar("SELECT active FROM role_policy WHERE singleton=TRUE")
        .fetch_one(&mut **tx)
        .await?;
    if active {
        let admitted: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM server_account_admission WHERE account_id=$1 AND accepted)")
            .bind(account).fetch_one(&mut **tx).await?;
        if !admitted {
            return Err(denied());
        }
    }
    let (devices, bytes): (i64, i64) = sqlx::query_as(
        "SELECT count(*)::BIGINT,COALESCE(sum(octet_length(d.contact)),0)::BIGINT FROM device_contacts d JOIN memberships m ON m.device_id=d.device_id WHERE d.account_id=$1 AND m.active AND NOT EXISTS(SELECT 1 FROM device_revocations r WHERE r.device_id=d.device_id)",
    ).bind(account).fetch_one(&mut **tx).await?;
    if devices > 64 || bytes > 4 * 1024 * 1024 {
        return Err(ApiError(
            StatusCode::PAYLOAD_TOO_LARGE,
            "CORDS_ROLE_TARGET_DEVICE_LIMIT",
        ));
    }
    let contacts: Vec<String> = sqlx::query_scalar(
        "SELECT d.contact FROM device_contacts d JOIN memberships m ON m.device_id=d.device_id WHERE d.account_id=$1 AND m.active AND NOT EXISTS(SELECT 1 FROM device_revocations r WHERE r.device_id=d.device_id) FOR SHARE OF d,m",
    ).bind(account).fetch_all(&mut **tx).await?;
    let mut valid = false;
    for contact in contacts {
        let contact: Contact = parse(&contact)?;
        if contact.authorization.value.account_id != account {
            return Err(denied());
        }
        if validate_contact(&contact, now()?).is_ok() {
            valid = true;
        }
    }
    if !valid {
        return Err(denied());
    }
    let owner: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM server_ownership WHERE account_id=$1)")
            .bind(account)
            .fetch_one(&mut **tx)
            .await?;
    Ok(Subject {
        account_id: account.into(),
        accepted: true,
        device_authorized: true,
        device_approved: false,
        revoked_or_burned: false,
        owner,
        roles: std::collections::BTreeSet::new(),
    })
}
