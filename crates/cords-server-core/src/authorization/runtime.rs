//! Durable authorization snapshots for the enforced-policy schema.
//!
//! Callers authenticate the session inside this transaction, then retain it through the mutation
//! or delivery decision. Policy writers take the same policy lock. A snapshot is
//! never a credential and must not be cached across requests or socket deliveries.
use super::{PolicyError, Subject, persistence};
use cords_identity::{validate_authorization, validate_contact};
use cords_protocol::{
    messaging::{Contact, DeviceAuthorization, Membership, Signed},
    roles::{Overrides, Permission},
};
use sqlx::{Postgres, Row as _, Transaction};
use std::collections::BTreeSet;

type Tx<'a> = Transaction<'a, Postgres>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("enforced authorization is not ready; keep the server closed")]
    NotReady,
    #[error("current authorization denies this action")]
    Denied,
    #[error(transparent)]
    Policy(#[from] PolicyError),
    #[error(transparent)]
    Repository(#[from] persistence::Error),
    #[error("authorization storage failed")]
    Database(#[from] sqlx::Error),
    #[error("stored authorization evidence is malformed")]
    Encoding(#[from] serde_json::Error),
}

#[derive(Debug)]
pub struct Decision {
    pub revision: i64,
    pub subject: Subject,
    pub permissions: BTreeSet<Permission>,
    /// Permission to enroll, independent of possession of current MLS state.
    pub account_eligible: bool,
    /// Durable delivery authorization only, not proof of client-held secrets.
    pub device_participating: bool,
}

impl Decision {
    /// A presentation hint is never sufficient authorization for a later mutation.
    /// # Errors
    /// Fails closed when current policy or device participation denies the action.
    pub fn require(&self, permission: Permission) -> Result<(), Error> {
        if self.permissions.contains(&permission) {
            Ok(())
        } else {
            Err(Error::Denied)
        }
    }
}

/// Derive an actor from durable evidence, never credential capabilities or request roles.
/// Session authentication is a separate mandatory gate held by the caller.
async fn subject(
    tx: &mut Tx<'_>,
    account: &str,
    device: &str,
    server: &str,
    server_key: &str,
    now: u64,
) -> Result<Subject, Error> {
    let row = sqlx::query(
        "SELECT d.account_id,d.contact,h.authorization_record,m.credential,m.active, \
         COALESCE(a.accepted,FALSE) accepted,COALESCE(v.approved,FALSE) approved, \
         EXISTS(SELECT 1 FROM device_revocations r WHERE r.device_id=d.device_id) revoked, \
         EXISTS(SELECT 1 FROM identity_burns b WHERE b.account_id=d.account_id) burned, \
         EXISTS(SELECT 1 FROM server_ownership o WHERE o.account_id=d.account_id) owner \
         FROM device_contacts d JOIN account_heads h ON h.account_id=d.account_id \
         JOIN memberships m ON m.device_id=d.device_id \
         LEFT JOIN server_account_admission a ON a.account_id=d.account_id \
         LEFT JOIN server_device_admission v ON v.device_id=d.device_id WHERE d.device_id=$1 AND d.account_id=$2",
    )
    .bind(device)
    .bind(account)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(Error::Denied)?;
    let account: String = row.try_get("account_id")?;
    let contact: Contact = serde_json::from_str(row.try_get("contact")?)?;
    let head: Signed<DeviceAuthorization> =
        serde_json::from_str(row.try_get("authorization_record")?)?;
    let membership: Signed<Membership> = serde_json::from_str(row.try_get("credential")?)?;
    let authorization = &contact.authorization.value;
    let authorized = validate_contact(&contact, now).is_ok()
        && validate_authorization(&head, now).is_ok()
        && head.value.account_id == account
        && head.value.root_public_key == authorization.root_public_key
        && authorization.generation <= head.value.generation
        && authorization.account_id == account
        && authorization.device_id == device
        && membership.verify(server_key).is_ok()
        && membership.value.version == 1
        && membership.value.server_id == server
        && membership.value.account_id == account
        && membership.value.device_id == device
        && membership.value.status == "active";
    let roles = sqlx::query_scalar("SELECT role_id FROM account_roles WHERE account_id=$1")
        .bind(&account)
        .fetch_all(&mut **tx)
        .await?
        .into_iter()
        .collect();
    Ok(Subject {
        account_id: account,
        accepted: row.try_get::<bool, _>("accepted")? && row.try_get::<bool, _>("active")?,
        device_authorized: authorized,
        device_approved: row.try_get("approved")?,
        revoked_or_burned: row.try_get::<bool, _>("revoked")?
            || row.try_get::<bool, _>("burned")?,
        owner: row.try_get("owner")?,
        roles,
    })
}

/// Load current policy under its serialization lock. An unavailable or partially
/// migrated policy is an error, never a fallback to credential capabilities.
/// # Errors
/// Rejects unready policy, malformed durable evidence and unknown devices/routes.
pub async fn load(
    tx: &mut Tx<'_>,
    device: &str,
    server: &str,
    server_key: &str,
    now: u64,
    channel: Option<&str>,
) -> Result<Decision, Error> {
    // Match existing ownership and identity transition lock order. Resolve the
    // device's account first, then verify that binding again under these locks.
    sqlx::query("SELECT pg_advisory_xact_lock_shared(hashtextextended('server-ownership',0))")
        .execute(&mut **tx)
        .await?;
    let account: String =
        sqlx::query_scalar("SELECT account_id FROM device_contacts WHERE device_id=$1")
            .bind(device)
            .fetch_optional(&mut **tx)
            .await?
            .ok_or(Error::Denied)?;
    sqlx::query("SELECT pg_advisory_xact_lock_shared(hashtextextended($1,0))")
        .bind(&account)
        .execute(&mut **tx)
        .await?;
    let snapshot = persistence::load(tx).await?;
    if !snapshot.active {
        return Err(Error::NotReady);
    }
    let ready: bool =
        sqlx::query_scalar("SELECT completed FROM rbac_migration WHERE singleton=TRUE")
            .fetch_one(&mut **tx)
            .await?;
    if !ready {
        return Err(Error::NotReady);
    }
    let policy = snapshot.policy.ok_or(Error::NotReady)?;
    let subject = subject(tx, &account, device, server, server_key, now).await?;
    let mut decision = Decision {
        revision: snapshot.revision,
        permissions: policy.server_permissions(&subject)?,
        subject,
        account_eligible: false,
        device_participating: false,
    };
    if let Some(channel) = channel {
        channel_access(tx, &policy, &mut decision, device, channel).await?;
    } else {
        // A server-wide response cannot promise authority over an unchecked route.
        decision
            .permissions
            .retain(|permission| !permission.channel_scoped());
    }
    Ok(decision)
}

async fn channel_access(
    tx: &mut Tx<'_>,
    policy: &super::Policy,
    decision: &mut Decision,
    device: &str,
    channel: &str,
) -> Result<(), Error> {
    let row = sqlx::query(
        "SELECT c.confidentiality_mode,c.retired,p.account_restricted,p.overrides, \
         COALESCE(g.overrides,'{}') category_overrides, \
         EXISTS(SELECT 1 FROM channel_account_access a WHERE a.channel_id=c.id AND a.account_id=$2) included, \
         EXISTS(SELECT 1 FROM channel_device_exclusions x WHERE x.channel_id=c.id AND x.device_id=$3) excluded, \
         EXISTS(SELECT 1 FROM pending_policy_removals r WHERE r.channel_id=c.id AND r.device_id=$3) removing, \
         EXISTS(SELECT 1 FROM channel_members m WHERE m.channel_id=c.id AND m.device_id=$3 AND m.active AND m.delivery_active) participating \
         FROM channels c JOIN channel_access_policy p ON p.channel_id=c.id \
         LEFT JOIN channel_categories g ON g.id=p.category_id WHERE c.id=$1 FOR SHARE OF c,p",
    ).bind(channel).bind(&decision.subject.account_id).bind(device)
        .fetch_optional(&mut **tx).await?.ok_or(Error::Denied)?;
    let category: Overrides = serde_json::from_str(row.try_get("category_overrides")?)?;
    let channel: Overrides = serde_json::from_str(row.try_get("overrides")?)?;
    let access = policy.access(&decision.subject, &category, &channel)?;
    decision.permissions = access.permissions;
    decision.account_eligible = access.eligible
        && (!row.try_get::<bool, _>("account_restricted")?
            || row.try_get::<bool, _>("included")?);
    let public = row.try_get::<String, _>("confidentiality_mode")? == "public";
    decision.device_participating = decision.account_eligible
        && !row.try_get::<bool, _>("excluded")?
        && !row.try_get::<bool, _>("removing")?
        && (public || row.try_get::<bool, _>("participating")?);
    if !decision.device_participating {
        // Independent ownership can manage policy without manufacturing delivery
        // permission or claiming possession of MLS group state.
        for permission in [
            Permission::ReadMessages,
            Permission::SendMessages,
            Permission::CommitMls,
        ] {
            decision.permissions.remove(&permission);
        }
    }
    if row.try_get::<bool, _>("retired")? {
        decision.permissions.remove(&Permission::SendMessages);
        decision.permissions.remove(&Permission::CommitMls);
    }
    Ok(())
}
