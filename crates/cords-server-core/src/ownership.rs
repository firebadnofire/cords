//! Persistent, server-local ownership bootstrap lifecycle.

use cords_protocol::messaging::{encode, hash};
use cords_storage::PostgresStore;
use rand_core::{OsRng, RngCore as _};
use sqlx::Row as _;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;
use zeroize::Zeroizing;

const LOCK: &str = "server-ownership-bootstrap";
const HASH_DOMAIN: &str = "CORDS-OWNERSHIP-CLAIM-CODE-V1";

#[derive(Debug, Error)]
pub enum BootstrapError {
    #[error(
        "database contains memberships but no ownership bootstrap state; run `cords-server ownership-bootstrap initialize` locally"
    )]
    LegacyInitializationRequired,
    #[error("server ownership has already been claimed")]
    AlreadyClaimed,
    #[error("server ownership bootstrap has not been initialized")]
    NotInitialized,
    #[error("operator recovery codes can only be generated during OWNER_LOCKDOWN")]
    NotLockedDown,
    #[error("system clock is before the Unix epoch")]
    Clock,
    #[error("ownership bootstrap storage operation failed")]
    Storage(#[from] sqlx::Error),
}

#[derive(Debug)]
pub enum BootstrapOutcome {
    Existing,
    Generated(Zeroizing<String>),
}

fn now() -> Result<i64, BootstrapError> {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| BootstrapError::Clock)?
        .as_secs();
    i64::try_from(seconds).map_err(|_| BootstrapError::Clock)
}

fn code() -> Zeroizing<String> {
    let mut bytes = [0_u8; 32];
    OsRng.fill_bytes(&mut bytes);
    Zeroizing::new(encode(bytes))
}

#[must_use]
pub fn claim_code_hash(server_id: &str, code: &str) -> String {
    let material = Zeroizing::new(format!("{HASH_DOMAIN}\0{server_id}\0{code}"));
    hash(material.as_bytes())
}

#[must_use]
pub fn recovery_code_hash(server_id: &str, code: &str) -> String {
    let material = Zeroizing::new(format!("CORDS-OWNER-RECOVERY-CODE-V1\0{server_id}\0{code}"));
    hash(material.as_bytes())
}

/// Generate a fresh, one-use operator recovery code without reopening bootstrap.
/// The plaintext is returned only to the local administrative caller.
/// # Errors
/// Returns an error unless the server is locked down or storage is unavailable.
pub async fn rotate_recovery_code(
    store: &PostgresStore,
    server_id: &str,
) -> Result<Zeroizing<String>, BootstrapError> {
    let mut tx = store.pool().begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended('server-ownership',0))")
        .execute(&mut *tx)
        .await?;
    let locked: Option<bool> = sqlx::query_scalar(
        "SELECT locked_down FROM server_owner_control WHERE singleton=TRUE FOR UPDATE",
    )
    .fetch_optional(&mut *tx)
    .await?;
    if locked != Some(true) {
        return Err(BootstrapError::NotLockedDown);
    }
    let code = code();
    sqlx::query("UPDATE server_owner_control SET recovery_code_hash=$1,recovery_generation=recovery_generation+1,recovery_issued_at=$2 WHERE singleton=TRUE AND locked_down")
        .bind(recovery_code_hash(server_id, &code))
        .bind(now()?)
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO server_owner_audit(id,action,occurred_at,details_hash) VALUES($1,'operator_code_rotated',$2,$3)")
        .bind(uuid::Uuid::now_v7().to_string()).bind(now()?)
        .bind(hash(server_id)).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(code)
}

/// Explicit local-operator intervention; never callable using an owner account key.
/// Idempotent lockdown preserves data and does not reopen bootstrap.
/// # Errors
/// Refuses an unclaimed server or unavailable storage.
pub async fn begin_recovery(store: &PostgresStore, server_id: &str) -> Result<(), BootstrapError> {
    let mut tx = store.pool().begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended('server-ownership',0))")
        .execute(&mut *tx)
        .await?;
    let claimed: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM server_ownership WHERE singleton=TRUE)")
            .fetch_one(&mut *tx)
            .await?;
    if !claimed {
        return Err(BootstrapError::NotInitialized);
    }
    let changed = sqlx::query("UPDATE server_owner_control SET locked_down=TRUE,recovery_code_hash=NULL,recovery_issued_at=NULL WHERE singleton=TRUE AND NOT locked_down")
        .execute(&mut *tx).await?.rows_affected();
    if changed != 0 {
        sqlx::query(
            "UPDATE server_ownership_bootstrap SET generation=generation+1 WHERE singleton=TRUE",
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query("INSERT INTO server_owner_audit(id,action,occurred_at,details_hash) VALUES($1,'operator_lockdown',$2,$3)")
            .bind(uuid::Uuid::now_v7().to_string()).bind(now()?).bind(hash(server_id))
            .execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(())
}

/// Ensure a fresh database has durable unclaimed state. Existing populated databases fail closed.
///
/// # Errors
/// Returns an error for inconsistent, legacy-populated, or unavailable persistent state.
pub async fn ensure(
    store: &PostgresStore,
    server_id: &str,
) -> Result<BootstrapOutcome, BootstrapError> {
    initialize(store, server_id, false).await
}

/// Explicitly initialize an unowned legacy database without selecting an existing account.
///
/// # Errors
/// Returns an error if ownership is already claimed or persistent state is unavailable.
pub async fn initialize_legacy(
    store: &PostgresStore,
    server_id: &str,
) -> Result<BootstrapOutcome, BootstrapError> {
    initialize(store, server_id, true).await
}

async fn initialize(
    store: &PostgresStore,
    server_id: &str,
    allow_populated: bool,
) -> Result<BootstrapOutcome, BootstrapError> {
    let mut tx = store.pool().begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(LOCK)
        .execute(&mut *tx)
        .await?;
    let bootstrap =
        sqlx::query("SELECT state FROM server_ownership_bootstrap WHERE singleton=TRUE FOR UPDATE")
            .fetch_optional(&mut *tx)
            .await?;
    if bootstrap.is_some() {
        tx.commit().await?;
        return Ok(BootstrapOutcome::Existing);
    }
    let owner =
        sqlx::query("SELECT claimed_at FROM server_ownership WHERE singleton=TRUE FOR UPDATE")
            .fetch_optional(&mut *tx)
            .await?;
    if let Some(owner) = owner {
        let claimed_at: i64 = owner.try_get("claimed_at")?;
        sqlx::query("INSERT INTO server_ownership_bootstrap(singleton,state,claim_code_hash,generation,initialized_at,claimed_at) VALUES(TRUE,'CLAIMED',NULL,1,$1,$1)")
            .bind(claimed_at)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        return Ok(BootstrapOutcome::Existing);
    }
    let memberships: i64 = sqlx::query_scalar("SELECT count(*) FROM memberships")
        .fetch_one(&mut *tx)
        .await?;
    if memberships != 0 && !allow_populated {
        return Err(BootstrapError::LegacyInitializationRequired);
    }
    let code = code();
    sqlx::query("INSERT INTO server_ownership_bootstrap(singleton,state,claim_code_hash,generation,initialized_at,claimed_at) VALUES(TRUE,'UNCLAIMED',$1,1,$2,NULL)")
        .bind(claim_code_hash(server_id, &code))
        .bind(now()?)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(BootstrapOutcome::Generated(code))
}

/// Replace the verifier for an initialized, still-unclaimed server.
///
/// # Errors
/// Returns an error unless the server is persistently unclaimed.
pub async fn rotate(
    store: &PostgresStore,
    server_id: &str,
) -> Result<Zeroizing<String>, BootstrapError> {
    let mut tx = store.pool().begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(LOCK)
        .execute(&mut *tx)
        .await?;
    let state: Option<String> = sqlx::query_scalar(
        "SELECT state FROM server_ownership_bootstrap WHERE singleton=TRUE FOR UPDATE",
    )
    .fetch_optional(&mut *tx)
    .await?;
    match state.as_deref() {
        Some("UNCLAIMED") => {}
        Some("CLAIMED") => return Err(BootstrapError::AlreadyClaimed),
        _ => return Err(BootstrapError::NotInitialized),
    }
    let code = code();
    sqlx::query("UPDATE server_ownership_bootstrap SET claim_code_hash=$1,generation=generation+1,initialized_at=$2 WHERE singleton=TRUE AND state='UNCLAIMED'")
        .bind(claim_code_hash(server_id, &code))
        .bind(now()?)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_codes_are_256_bit_url_safe_and_unique() {
        let first = code();
        let second = code();
        assert_eq!(first.len(), 43);
        assert!(
            first
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
        );
        assert_ne!(first.as_str(), second.as_str());
    }
}
