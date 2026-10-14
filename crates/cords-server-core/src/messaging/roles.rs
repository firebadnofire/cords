//! Preparatory role administration. Schema 13 prevents activation; only the
//! current owner can edit the catalog until the full enforcement gate passes.
use super::*;
use crate::authorization::{PolicyError, Subject, persistence};
use cords_protocol::roles::{
    AccountRoles, RoleCatalog, RoleChange, RoleChangeReceipt, RoleChangeRequest,
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

pub(super) async fn catalog(
    State(s): State<Service>,
    headers: HeaderMap,
) -> Result<Json<Signed<RoleCatalog>>> {
    let member = s.authenticate(&headers, "server.manage").await?;
    let mut tx = s.0.store.pool().begin().await?;
    s.authorize_mutation(&mut tx, &headers, &member, None)
        .await?;
    owner_subject(&mut tx, &member.value).await?;
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
    let member = s.authenticate(&headers, "server.manage").await?;
    let mut tx = s.0.store.pool().begin().await?;
    s.authorize_mutation(&mut tx, &headers, &member, None)
        .await?;
    let actor = owner_subject(&mut tx, &member.value).await?;
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
        persistence::apply(
            &mut tx,
            &actor,
            &member.value.device_id,
            target.as_ref(),
            &request,
        )
        .await?
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
    Ok(Json(response))
}

pub(super) async fn account_roles(
    State(s): State<Service>,
    Path(account): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Signed<AccountRoles>>> {
    if account.is_empty() || account.len() > 128 {
        return Err(bad());
    }
    let member = s.authenticate(&headers, "server.manage").await?;
    let mut tx = s.0.store.pool().begin().await?;
    s.authorize_mutation(&mut tx, &headers, &member, None)
        .await?;
    owner_subject(&mut tx, &member.value).await?;
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
    let member = s.authenticate(&headers, "server.manage").await?;
    let mut tx = s.0.store.pool().begin().await?;
    s.authorize_mutation(&mut tx, &headers, &member, None)
        .await?;
    owner_subject(&mut tx, &member.value).await?;
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
async fn revocation_target(tx: &mut Tx<'_>, account: &str) -> Result<Subject> {
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

async fn assignment_target(tx: &mut Tx<'_>, account: &str) -> Result<Subject> {
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
