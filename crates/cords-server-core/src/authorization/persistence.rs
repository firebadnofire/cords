//! Transactional role repository. The caller must authenticate and lock identity,
//! membership and owner state in this SAME transaction before calling `apply`.
//! No HTTP route activates this repository until all delivery paths enforce it.
use super::{Policy, PolicyError, Subject, default_roles};
use cords_protocol::roles::{DeviceAdmission, Role};
pub use cords_protocol::roles::{RoleChange as Change, RoleChangeRequest as Request};
use sha2::{Digest as _, Sha256};
use sqlx::{Postgres, Transaction};

type Tx<'a> = Transaction<'a, Postgres>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("role policy changed; reload before retrying")]
    Conflict,
    #[error("role policy has not been initialized")]
    Uninitialized,
    #[error("idempotency key was reused for a different operation")]
    Replay,
    #[error(transparent)]
    Policy(#[from] PolicyError),
    #[error("role policy storage failed")]
    Database(#[from] sqlx::Error),
    #[error("stored role policy is invalid")]
    Encoding(#[from] serde_json::Error),
}

#[derive(Debug)]
pub struct Snapshot {
    pub revision: i64,
    pub active: bool,
    pub policy: Option<Policy>,
}

/// Takes the policy row lock for a coherent snapshot and serialized mutations.
/// # Errors
/// Fails closed on missing or malformed durable policy.
pub async fn load(tx: &mut Tx<'_>) -> Result<Snapshot, Error> {
    let (revision, active, admission): (i64, bool, String) = sqlx::query_as(
        "SELECT revision,active,device_admission FROM role_policy WHERE singleton=TRUE FOR UPDATE",
    )
    .fetch_one(&mut **tx)
    .await?;
    let admission = match admission.as_str() {
        "account_authorized" => DeviceAdmission::AccountAuthorized,
        "administrator_approval" => DeviceAdmission::AdministratorApproval,
        _ => return Err(PolicyError::Invalid.into()),
    };
    let rows: Vec<(String, String, String)> =
        sqlx::query_as("SELECT id,name,definition FROM server_roles ORDER BY id")
            .fetch_all(&mut **tx)
            .await?;
    let mut roles = Vec::with_capacity(rows.len());
    for (id, name, definition) in rows {
        let role: Role = serde_json::from_str(&definition)?;
        if role.id != id || role.name != name {
            return Err(PolicyError::Invalid.into());
        }
        roles.push(role);
    }
    let policy = if revision == 0 && !active && roles.is_empty() {
        None
    } else {
        Some(Policy::new(roles, admission)?)
    };
    Ok(Snapshot {
        revision,
        active,
        policy,
    })
}

async fn current_subject(tx: &mut Tx<'_>, subject: &Subject) -> Result<Subject, Error> {
    let mut current = subject.clone();
    current.roles = sqlx::query_scalar::<_, String>(
        "SELECT role_id FROM account_roles WHERE account_id=$1 ORDER BY role_id",
    )
    .bind(&subject.account_id)
    .fetch_all(&mut **tx)
    .await?
    .into_iter()
    .collect();
    Ok(current)
}

/// Stage an authorized change, audit and idempotent receipt together. Commit is
/// the caller's responsibility; it must never acknowledge before commit succeeds.
/// `actor` and optional assignment `target` must come from current verified state,
/// never from request JSON. Role assignments are reloaded under the policy lock.
/// # Errors
/// Rejects stale revision, idempotency substitution, unauthorized or invalid changes.
pub async fn apply(
    tx: &mut Tx<'_>,
    actor: &Subject,
    actor_device: &str,
    target: Option<&Subject>,
    request: &Request,
) -> Result<i64, Error> {
    validate_request(request)?;
    let snapshot = load(tx).await?;
    // The device/account binding is additionally checked here even though session
    // authentication is a mandatory caller responsibility.
    let matches: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM device_contacts WHERE device_id=$1 AND account_id=$2)",
    )
    .bind(actor_device)
    .bind(&actor.account_id)
    .fetch_one(&mut **tx)
    .await?;
    if !matches {
        return Err(PolicyError::Denied.into());
    }
    if let Some(revision) = committed_revision(tx, actor_device, request).await? {
        return Ok(revision);
    }
    if snapshot.revision != request.expected_revision {
        return Err(Error::Conflict);
    }
    let actor = current_subject(tx, actor).await?;
    let (operation, subject) = apply_change(tx, &snapshot, &actor, target, &request.change).await?;
    let revision = snapshot.revision.checked_add(1).ok_or(Error::Conflict)?;
    let digest = Sha256::digest(serde_json::to_vec(request)?).to_vec();
    sqlx::query("UPDATE role_policy SET revision=$1 WHERE singleton=TRUE")
        .bind(revision)
        .execute(&mut **tx)
        .await?;
    sqlx::query("INSERT INTO role_policy_changes(revision,actor_device_id,request_id,request_digest,operation,target) VALUES($1,$2,$3,$4,$5,$6)")
        .bind(revision).bind(actor_device).bind(&request.request_id).bind(digest)
        .bind(operation).bind(subject).execute(&mut **tx).await?;
    Ok(revision)
}

fn validate_request(request: &Request) -> Result<(), Error> {
    if request.expected_revision < 0
        || request.request_id.is_empty()
        || request.request_id.len() > 128
        || request.request_id.chars().any(char::is_control)
    {
        return Err(PolicyError::Invalid.into());
    }
    Ok(())
}

/// Look up an immutable committed receipt after authenticating the current actor.
/// This grants no authority and must not reapply the historical operation. The
/// caller may return it even when the original target is no longer admitted.
/// `apply` repeats this lookup under the policy lock to serialize concurrent retries.
/// # Errors
/// Rejects malformed requests, substituted retry payloads and storage failures.
pub async fn committed_revision(
    tx: &mut Tx<'_>,
    actor_device: &str,
    request: &Request,
) -> Result<Option<i64>, Error> {
    validate_request(request)?;
    let previous: Option<(i64, Vec<u8>)> = sqlx::query_as(
        "SELECT revision,request_digest FROM role_policy_changes WHERE actor_device_id=$1 AND request_id=$2",
    ).bind(actor_device).bind(&request.request_id).fetch_optional(&mut **tx).await?;
    match previous {
        Some((revision, digest))
            if digest == Sha256::digest(serde_json::to_vec(request)?).to_vec() =>
        {
            Ok(Some(revision))
        }
        Some(_) => Err(Error::Replay),
        None => Ok(None),
    }
}

async fn apply_change(
    tx: &mut Tx<'_>,
    snapshot: &Snapshot,
    actor: &Subject,
    target: Option<&Subject>,
    change: &Change,
) -> Result<(&'static str, String), Error> {
    Ok(match change {
        Change::Initialize => {
            // Compatibility for an older preparatory UI. Defaults already exist;
            // acknowledge its explicit setup request only at the untouched baseline.
            // Never reset edited roles or insert a synthetic user audit on migration.
            if snapshot.revision != 0 {
                return Err(Error::Conflict);
            }
            let policy = Policy::new(default_roles(), DeviceAdmission::AccountAuthorized)?;
            if !actor.owner || !policy.admitted(actor) {
                return Err(PolicyError::Denied.into());
            }
            for role in policy.roles().values() {
                if snapshot
                    .policy
                    .as_ref()
                    .is_some_and(|current| current.roles().get(&role.id) != Some(role))
                {
                    return Err(Error::Conflict);
                }
                if snapshot.policy.is_none() {
                    write_role(tx, role).await?;
                }
            }
            ("initialize", "server".to_owned())
        }
        Change::PutRole { role } => {
            let policy = snapshot.policy.as_ref().ok_or(Error::Uninitialized)?;
            policy.authorize_edit(actor, &role.id, Some(role))?;
            write_role(tx, role).await?;
            ("put_role", role.id.clone())
        }
        Change::DeleteRole { role_id } => {
            let policy = snapshot.policy.as_ref().ok_or(Error::Uninitialized)?;
            policy.authorize_edit(actor, role_id, None)?;
            sqlx::query("DELETE FROM account_roles WHERE role_id=$1")
                .bind(role_id)
                .execute(&mut **tx)
                .await?;
            sqlx::query("DELETE FROM server_roles WHERE id=$1")
                .bind(role_id)
                .execute(&mut **tx)
                .await?;
            ("delete_role", role_id.clone())
        }
        Change::AssignRole {
            account_id,
            role_id,
        }
        | Change::RevokeRole {
            account_id,
            role_id,
        } => {
            let policy = snapshot.policy.as_ref().ok_or(Error::Uninitialized)?;
            let target = target
                .filter(|t| t.account_id == *account_id)
                .ok_or(PolicyError::Denied)?;
            let target = current_subject(tx, target).await?;
            let assign = matches!(change, Change::AssignRole { .. });
            if assign {
                policy.authorize_assignment(actor, &target, role_id)?;
            } else {
                policy.authorize_revocation(actor, &target, role_id)?;
            }
            let query = if assign {
                "INSERT INTO account_roles(account_id,role_id) VALUES($1,$2) ON CONFLICT DO NOTHING"
            } else {
                "DELETE FROM account_roles WHERE account_id=$1 AND role_id=$2"
            };
            sqlx::query(query)
                .bind(account_id)
                .bind(role_id)
                .execute(&mut **tx)
                .await?;
            (
                if assign { "assign_role" } else { "revoke_role" },
                format!("{account_id}/{role_id}"),
            )
        }
    })
}

async fn write_role(tx: &mut Tx<'_>, role: &Role) -> Result<(), Error> {
    sqlx::query("INSERT INTO server_roles(id,name,definition) VALUES($1,$2,$3) ON CONFLICT(id) DO UPDATE SET name=EXCLUDED.name,definition=EXCLUDED.definition")
        .bind(&role.id).bind(&role.name).bind(serde_json::to_string(role)?)
        .execute(&mut **tx).await?;
    Ok(())
}

#[cfg(test)]
mod tests;
