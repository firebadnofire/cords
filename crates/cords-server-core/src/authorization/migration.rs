//! Native verification of the frozen pre-RBAC channel boundary.
//! The candidate SQL is staged separately until endpoint enforcement is complete.
use cords_identity::{validate_authorization, validate_contact};
use cords_protocol::messaging::{Contact, DeviceAuthorization, Membership, Signed};
use cords_storage::PostgresStore;
use sqlx::Row as _;

fn verified_binding(
    row: &sqlx::postgres::PgRow,
    server_id: &str,
    server_key: &str,
    now: u64,
) -> Result<bool, Error> {
    let contact: Contact = serde_json::from_str(row.try_get("contact")?)?;
    let head: Signed<DeviceAuthorization> =
        serde_json::from_str(row.try_get("authorization_record")?)?;
    let membership: Signed<Membership> = serde_json::from_str(row.try_get("credential")?)?;
    let account: String = row.try_get("account_id")?;
    let device: String = row.try_get("device_id")?;
    let authorization = &contact.authorization.value;
    Ok(validate_contact(&contact, now).is_ok()
        && validate_authorization(&head, now).is_ok()
        && head.value.account_id == account
        && head.value.root_public_key == authorization.root_public_key
        && authorization.generation <= head.value.generation
        && authorization.account_id == account
        && authorization.device_id == device
        && membership.verify(server_key).is_ok()
        && membership.value.version == 1
        && membership.value.server_id == server_id
        && membership.value.account_id == account
        && membership.value.device_id == device
        && membership.value.status == "active")
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("RBAC migration database operation failed")]
    Database(#[from] sqlx::Error),
    #[error(
        "RBAC migration contains malformed identity data; repair verified records before retrying"
    )]
    Encoding(#[from] serde_json::Error),
    #[error("RBAC migration server identity does not match the bound database")]
    Identity,
    #[error("RBAC migration time is outside the supported range")]
    Time,
    #[error("RBAC startup policy validation failed")]
    Policy(#[from] super::persistence::Error),
    #[error("RBAC startup requires a complete native boundary and valid role catalog")]
    Incomplete,
}

/// Prepare the durable boundary and enable the validated current-policy engine
/// before opening any listener. Repeated startup preserves definitions, explicit
/// assignments, history bounds and audit state. Failure leaves traffic closed.
/// # Errors
/// Rejects mismatched identity, invalid policy or incomplete native preparation.
pub async fn activate(
    store: &PostgresStore,
    server_id: &str,
    server_key: &str,
    now: u64,
) -> Result<(), Error> {
    prepare(store, server_id, server_key, now).await?;
    let mut tx = store.pool().begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended('server-ownership',0))")
        .execute(&mut *tx)
        .await?;
    let snapshot = super::persistence::load(&mut tx).await?;
    let completed: bool =
        sqlx::query_scalar("SELECT completed FROM rbac_migration WHERE singleton=TRUE FOR UPDATE")
            .fetch_one(&mut *tx)
            .await?;
    if !completed || snapshot.policy.is_none() {
        return Err(Error::Incomplete);
    }
    sqlx::query("UPDATE role_policy SET active=TRUE WHERE singleton=TRUE AND NOT active")
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

/// Complete the frozen migration boundary exactly once, without changing any
/// device, MLS, ownership, history, or pending-removal state. The caller must
/// keep the server closed to traffic until enforcement is ready.
/// # Errors
/// Storage/encoding failures roll back all grants; a different server is rejected.
pub async fn prepare(
    store: &PostgresStore,
    server_id: &str,
    server_key: &str,
    now: u64,
) -> Result<(), Error> {
    let public: [u8; 32] = cords_protocol::messaging::decode(server_key)
        .map_err(|_| Error::Identity)?
        .try_into()
        .map_err(|_| Error::Identity)?;
    if cords_protocol::server_id_from_public_key(&public) != server_id {
        return Err(Error::Identity);
    }
    let mut tx = store.pool().begin().await?;
    let bound: Option<String> = sqlx::query_scalar(
        "SELECT server_id FROM cords_server_identity WHERE singleton=TRUE FOR UPDATE",
    )
    .fetch_optional(&mut *tx)
    .await?;
    if bound.as_deref() != Some(server_id) {
        return Err(Error::Identity);
    }
    let completed: bool =
        sqlx::query_scalar("SELECT completed FROM rbac_migration WHERE singleton=TRUE FOR UPDATE")
            .fetch_one(&mut *tx)
            .await?;
    if completed {
        tx.commit().await?;
        return Ok(());
    }
    // Prevent admission, revocation or roster writes between validation and
    // commit, including when called accidentally alongside an old test service.
    sqlx::query("LOCK TABLE account_heads,device_contacts,memberships,channel_members,device_revocations,identity_burns,pending_policy_removals IN SHARE MODE")
        .execute(&mut *tx).await?;
    let admitted = sqlx::query(
        "SELECT d.device_id,d.account_id,d.contact,h.authorization_record,s.credential \
         FROM device_contacts d JOIN account_heads h ON h.account_id=d.account_id \
         JOIN memberships s ON s.device_id=d.device_id WHERE s.active \
         AND NOT EXISTS(SELECT 1 FROM device_revocations r WHERE r.device_id=d.device_id) \
         AND NOT EXISTS(SELECT 1 FROM identity_burns b WHERE b.account_id=d.account_id)",
    )
    .fetch_all(&mut *tx)
    .await?;
    for row in admitted {
        if verified_binding(&row, server_id, server_key, now)? {
            sqlx::query(
                "INSERT INTO server_account_admission VALUES($1,TRUE) ON CONFLICT DO NOTHING",
            )
            .bind(row.try_get::<String, _>("account_id")?)
            .execute(&mut *tx)
            .await?;
            sqlx::query(
                "INSERT INTO server_device_admission VALUES($1,TRUE) ON CONFLICT DO NOTHING",
            )
            .bind(row.try_get::<String, _>("device_id")?)
            .execute(&mut *tx)
            .await?;
        }
    }
    let rows = sqlx::query(
        "SELECT f.channel_id,f.device_id,f.account_id,d.contact,h.authorization_record,s.credential \
         FROM rbac_migration_candidates f \
         JOIN device_contacts d ON d.device_id=f.device_id AND d.account_id=f.account_id \
         JOIN account_heads h ON h.account_id=f.account_id \
         JOIN memberships s ON s.device_id=f.device_id \
         JOIN channel_members m ON m.channel_id=f.channel_id AND m.device_id=f.device_id \
         WHERE s.active AND m.active AND m.delivery_active \
         AND NOT EXISTS(SELECT 1 FROM device_revocations r WHERE r.device_id=f.device_id) \
         AND NOT EXISTS(SELECT 1 FROM identity_burns b WHERE b.account_id=f.account_id) \
         AND NOT EXISTS(SELECT 1 FROM pending_policy_removals p WHERE p.channel_id=f.channel_id AND p.device_id=f.device_id) \
         AND NOT EXISTS(SELECT 1 FROM channel_device_exclusions x WHERE x.channel_id=f.channel_id AND x.device_id=f.device_id)",
    ).fetch_all(&mut *tx).await?;
    let mut rejected = 0_i64;
    for row in rows {
        let membership: Signed<Membership> = serde_json::from_str(row.try_get("credential")?)?;
        let account: String = row.try_get("account_id")?;
        // A head may certify another device. Require its valid root binding,
        // without mistaking account generation for permission to use that leaf.
        let valid = verified_binding(&row, server_id, server_key, now)?
            && membership
                .value
                .capabilities
                .iter()
                .any(|capability| capability == "channel.read");
        if valid {
            sqlx::query("INSERT INTO channel_account_access(channel_id,account_id) VALUES($1,$2) ON CONFLICT DO NOTHING")
                .bind(row.try_get::<String, _>("channel_id")?).bind(account).execute(&mut *tx).await?;
        } else {
            rejected += 1;
        }
    }
    sqlx::query("UPDATE rbac_migration SET completed=TRUE,checked_at=$1,rejected_devices=$2 WHERE singleton=TRUE")
        .bind(i64::try_from(now).map_err(|_| Error::Time)?).bind(rejected).execute(&mut *tx).await?;
    // Native verification is independent of activation. Only the integrated
    // server checkpoint may remove the runtime fence and enable enforcement.
    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests;
