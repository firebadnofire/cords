//! Current role presentation metadata; no snapshot is a mutation credential.
use super::*;
use crate::authorization::persistence;
use cords_protocol::roles::{RoleCatalog, RoleTargetActions, RoleWorkspace};

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct MemberPage {
    pub after: String,
    pub assigned: bool,
}

pub(super) async fn members(
    State(s): State<Service>,
    headers: HeaderMap,
    Path(role): Path<String>,
    Query(page): Query<MemberPage>,
) -> Result<Json<Signed<cords_protocol::roles::RoleMembersPage>>> {
    use cords_protocol::roles::{
        DEFAULT_ROLE, ROLE_MEMBERS_PAGE_SIZE, RoleMember, RoleMembersPage,
    };
    if page.after.len() > 128 || role.is_empty() || role.len() > 128 {
        return Err(bad());
    }
    let member = s.authenticate(&headers, "channel.read").await?;
    let mut tx = s.0.store.pool().begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock_shared(hashtextextended('server-ownership',0))")
        .execute(&mut *tx)
        .await?;
    // Identify bounded candidates first so identity locks precede the policy lock,
    // matching admission/revocation and assignment mutations. Recheck below.
    let ids: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT d.account_id COLLATE \"C\" AS account_id FROM device_contacts d JOIN memberships m ON m.device_id=d.device_id WHERE m.active AND d.account_id COLLATE \"C\">$1 COLLATE \"C\" AND NOT EXISTS(SELECT 1 FROM device_revocations r WHERE r.device_id=d.device_id) AND NOT EXISTS(SELECT 1 FROM identity_burns b WHERE b.account_id=d.account_id) AND (($2='builtin:default' OR EXISTS(SELECT 1 FROM account_roles a WHERE a.account_id=d.account_id AND a.role_id=$2))=$3) ORDER BY account_id LIMIT 17"
    ).bind(&page.after).bind(&role).bind(page.assigned).fetch_all(&mut *tx).await?;
    let mut targets = Vec::new();
    for account in ids.iter().take(ROLE_MEMBERS_PAGE_SIZE) {
        match roles::assignment_target(&mut tx, account).await {
            Ok(target) => targets.push(target),
            Err(error) if error.0 == StatusCode::FORBIDDEN && page.assigned => {
                // Expired device evidence cannot grant a role, but must not hide
                // an existing admitted account's stored role from removal UI.
                let active: bool =
                    sqlx::query_scalar("SELECT active FROM role_policy WHERE singleton=TRUE")
                        .fetch_one(&mut *tx)
                        .await?;
                let admitted = if active {
                    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM server_account_admission WHERE account_id=$1 AND accepted)")
                        .bind(account).fetch_one(&mut *tx).await?
                } else {
                    true
                };
                if admitted {
                    targets.push(roles::revocation_target(&mut tx, account).await?);
                }
            }
            Err(error) if error.0 == StatusCode::FORBIDDEN => (),
            Err(error) => return Err(error),
        }
    }
    s.authorize_mutation(&mut tx, &headers, &member, None)
        .await?;
    let actor = roles::administrative_subject(&s, &mut tx, &member.value, &[]).await?;
    let snapshot = persistence::load(&mut tx).await?;
    let policy = snapshot.policy.ok_or_else(denied)?;
    if !policy.roles().contains_key(&role) {
        return Err(bad());
    }
    let mut members = Vec::new();
    for mut target in targets {
        target.roles = sqlx::query_scalar(
            "SELECT role_id FROM account_roles WHERE account_id=$1 ORDER BY role_id",
        )
        .bind(&target.account_id)
        .fetch_all(&mut *tx)
        .await?
        .into_iter()
        .collect();
        let assigned = role == DEFAULT_ROLE || target.roles.contains(&role);
        if assigned != page.assigned {
            continue;
        }
        members.push(RoleMember {
            can_assign: snapshot.active
                && target.account_id != actor.account_id
                && !assigned
                && policy.authorize_assignment(&actor, &target, &role).is_ok(),
            can_remove: snapshot.active
                && target.account_id != actor.account_id
                && assigned
                && policy.authorize_revocation(&actor, &target, &role).is_ok(),
            account_id: target.account_id,
            assigned,
        });
    }
    let admission = if snapshot.active {
        "AND EXISTS(SELECT 1 FROM server_account_admission s WHERE s.account_id=d.account_id AND s.accepted)"
    } else {
        ""
    };
    let count: i64 = sqlx::query_scalar(&format!("SELECT count(DISTINCT d.account_id)::BIGINT FROM device_contacts d JOIN memberships m ON m.device_id=d.device_id WHERE m.active AND NOT EXISTS(SELECT 1 FROM device_revocations r WHERE r.device_id=d.device_id) AND NOT EXISTS(SELECT 1 FROM identity_burns b WHERE b.account_id=d.account_id) {admission} AND ($1='builtin:default' OR EXISTS(SELECT 1 FROM account_roles a WHERE a.account_id=d.account_id AND a.role_id=$1))"))
        .bind(&role).fetch_one(&mut *tx).await?;
    let response = s.sign(RoleMembersPage {
        version: 1,
        server_id: s.0.identity.server_id(),
        account_id: member.value.account_id,
        device_id: member.value.device_id,
        catalog: RoleCatalog {
            version: 1,
            server_id: s.0.identity.server_id(),
            revision: uint(snapshot.revision)?,
            active: snapshot.active,
            roles: policy.roles().values().cloned().collect(),
        },
        role_id: role,
        assigned: page.assigned,
        after: page.after,
        next_after: (ids.len() > ROLE_MEMBERS_PAGE_SIZE)
            .then(|| ids[ROLE_MEMBERS_PAGE_SIZE - 1].clone()),
        member_count: uint(count)?,
        members,
    })?;
    tx.commit().await?;
    Ok(Json(response))
}

pub(super) async fn workspace(
    State(s): State<Service>,
    headers: HeaderMap,
) -> Result<Json<Signed<RoleWorkspace>>> {
    let member = s.authenticate(&headers, "channel.read").await?;
    let mut tx = s.0.store.pool().begin().await?;
    s.authorize_mutation(&mut tx, &headers, &member, None)
        .await?;
    let actor = roles::administrative_subject(&s, &mut tx, &member.value, &[]).await?;
    let snapshot = persistence::load(&mut tx).await?;
    let policy = snapshot.policy.ok_or_else(denied)?;
    let mut editable = std::collections::BTreeSet::new();
    let mut deletable = std::collections::BTreeSet::new();
    for role in policy.roles().values() {
        if policy.authorize_edit(&actor, &role.id, Some(role)).is_ok() {
            editable.insert(role.id.clone());
        }
        if policy.authorize_edit(&actor, &role.id, None).is_ok() {
            deletable.insert(role.id.clone());
        }
    }
    let probe = cords_protocol::roles::Role {
        id: id(),
        name: id(),
        color: "#123456".into(),
        description: String::new(),
        priority: 0,
        permissions: std::collections::BTreeSet::default(),
        mentionable: false,
    };
    let can_create = policy
        .authorize_edit(&actor, &probe.id, Some(&probe))
        .is_ok();
    // Keep aggregation in PostgreSQL: the response is bounded by MAX_ROLES,
    // irrespective of the number of accounts or devices in this community.
    let admission = if snapshot.active {
        "AND EXISTS(SELECT 1 FROM server_account_admission s WHERE s.account_id=d.account_id AND s.accepted)"
    } else {
        ""
    };
    let query = format!(
        "WITH eligible AS (SELECT DISTINCT d.account_id FROM device_contacts d JOIN memberships m ON m.device_id=d.device_id WHERE m.active AND NOT EXISTS(SELECT 1 FROM device_revocations r WHERE r.device_id=d.device_id) AND NOT EXISTS(SELECT 1 FROM identity_burns b WHERE b.account_id=d.account_id) {admission}) SELECT a.role_id,count(DISTINCT a.account_id)::BIGINT FROM account_roles a JOIN eligible e ON e.account_id=a.account_id GROUP BY a.role_id UNION ALL SELECT $1,count(*)::BIGINT FROM eligible"
    );
    let rows: Vec<(String, i64)> = sqlx::query_as(&query)
        .bind(cords_protocol::roles::DEFAULT_ROLE)
        .fetch_all(&mut *tx)
        .await?;
    let mut counts: std::collections::BTreeMap<String, u64> =
        policy.roles().keys().map(|id| (id.clone(), 0)).collect();
    for (role, count) in rows {
        if let Some(value) = counts.get_mut(&role) {
            *value = uint(count)?;
        }
    }
    let result = s.sign(RoleWorkspace {
        version: 1,
        server_id: s.0.identity.server_id(),
        account_id: member.value.account_id,
        device_id: member.value.device_id,
        catalog: RoleCatalog {
            version: 1,
            server_id: s.0.identity.server_id(),
            revision: uint(snapshot.revision)?,
            active: snapshot.active,
            roles: policy.roles().values().cloned().collect(),
        },
        counts,
        editable,
        deletable,
        can_create,
    })?;
    tx.commit().await?;
    Ok(Json(result))
}

pub(super) async fn target(
    State(s): State<Service>,
    headers: HeaderMap,
    Path(account): Path<String>,
) -> Result<Json<Signed<RoleTargetActions>>> {
    if account.is_empty() || account.len() > 128 {
        return Err(bad());
    }
    let member = s.authenticate(&headers, "channel.read").await?;
    let mut tx = s.0.store.pool().begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock_shared(hashtextextended('server-ownership',0))")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SELECT pg_advisory_xact_lock_shared(hashtextextended($1,0))")
        .bind(&account)
        .execute(&mut *tx)
        .await?;
    s.authorize_mutation(&mut tx, &headers, &member, None)
        .await?;
    let actor = roles::administrative_subject(&s, &mut tx, &member.value, &[]).await?;
    let snapshot = persistence::load(&mut tx).await?;
    let policy = snapshot.policy.ok_or_else(denied)?;
    let mut target = roles::revocation_target(&mut tx, &account).await?;
    let eligible = match roles::assignment_target(&mut tx, &account).await {
        Ok(eligible) => {
            target.accepted = eligible.accepted;
            true
        }
        Err(error) if error.0 == StatusCode::FORBIDDEN => false,
        Err(error) => return Err(error),
    };
    target.roles = sqlx::query_scalar(
        "SELECT role_id FROM account_roles WHERE account_id=$1 ORDER BY role_id",
    )
    .bind(&account)
    .fetch_all(&mut *tx)
    .await?
    .into_iter()
    .collect();
    let mut assignable = std::collections::BTreeSet::new();
    let mut removable = std::collections::BTreeSet::new();
    // Personal role controls remain read-only, including for the owner whose
    // authority is independent of role assignment. Mutations still reauthorize.
    if snapshot.active && target.account_id != actor.account_id {
        for role in policy.roles().keys() {
            if target.roles.contains(role) {
                if policy.authorize_revocation(&actor, &target, role).is_ok() {
                    removable.insert(role.clone());
                }
            } else if eligible && policy.authorize_assignment(&actor, &target, role).is_ok() {
                assignable.insert(role.clone());
            }
        }
    }
    let result = s.sign(RoleTargetActions {
        version: 1,
        server_id: s.0.identity.server_id(),
        account_id: member.value.account_id,
        device_id: member.value.device_id,
        target_account_id: account,
        catalog: RoleCatalog {
            version: 1,
            server_id: s.0.identity.server_id(),
            revision: uint(snapshot.revision)?,
            active: snapshot.active,
            roles: policy.roles().values().cloned().collect(),
        },
        role_ids: target.roles,
        assignable,
        removable,
        eligible,
    })?;
    tx.commit().await?;
    Ok(Json(result))
}
