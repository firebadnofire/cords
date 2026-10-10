//! Root-authorized account departure and owner succession. All state changes are transactional.
use super::{
    ApiError, Contact, Json, Membership, Result, Service, Signed, State, StatusCode, Tx, bad,
    canonical, conflict, denied, hash, id, int, json, now, parse, previous, remember,
};
use cords_protocol::messaging::{
    ChallengeRequest, DepartureReceipt, DepartureRequest, DeviceAuthorization, IdentityBurn,
    OwnershipRecoveryClaim, ServerDeparture, SuccessorAcceptance, SuccessorDesignation,
    SuccessorRequest,
};
use sqlx::Row as _;
use subtle::ConstantTimeEq as _;

const MATURATION_SECONDS: u64 = 30 * 24 * 60 * 60;

fn eligible(accepted_at: u64, now: u64) -> bool {
    now >= accepted_at.saturating_add(MATURATION_SECONDS)
}

async fn root_key(tx: &mut Tx<'_>, account: &str) -> Result<String> {
    let head: String = sqlx::query_scalar(
        "SELECT authorization_record FROM account_heads WHERE account_id=$1",
    )
    .bind(account)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(denied)?;
    let head: Signed<DeviceAuthorization> = parse(&head)?;
    if head.value.account_id != account {
        return Err(denied());
    }
    Ok(head.value.root_public_key)
}

async fn deactivate_account(tx: &mut Tx<'_>, account: &str) -> Result<()> {
    sqlx::query("UPDATE sessions SET active=FALSE WHERE device_id IN (SELECT device_id FROM device_contacts WHERE account_id=$1)")
        .bind(account).execute(&mut **tx).await?;
    sqlx::query("UPDATE memberships SET active=FALSE WHERE device_id IN (SELECT device_id FROM device_contacts WHERE account_id=$1)")
        .bind(account).execute(&mut **tx).await?;
    sqlx::query("INSERT INTO pending_policy_removals(channel_id,device_id) SELECT cm.channel_id,cm.device_id FROM channel_members cm JOIN device_contacts dc ON dc.device_id=cm.device_id WHERE dc.account_id=$1 AND cm.active ON CONFLICT DO NOTHING")
        .bind(account).execute(&mut **tx).await?;
    sqlx::query("UPDATE channel_members SET delivery_active=FALSE WHERE device_id IN (SELECT device_id FROM device_contacts WHERE account_id=$1)")
        .bind(account).execute(&mut **tx).await?;
    Ok(())
}

async fn owner_departure(s: &Service, tx: &mut Tx<'_>, account: &str, burn: bool, time: u64) -> Result<()> {
    let owner: Option<String> = sqlx::query_scalar(
        "SELECT account_id FROM server_ownership WHERE singleton=TRUE FOR UPDATE",
    )
    .fetch_optional(&mut **tx).await?;
    if owner.as_deref() != Some(account) {
        return Ok(());
    }
    if burn {
        let rows = sqlx::query("SELECT id,successor_account_id,accepted_at FROM server_successor_designations WHERE acceptance IS NOT NULL ORDER BY accepted_at,id FOR UPDATE")
            .fetch_all(&mut **tx).await?;
        for row in rows {
            let accepted_at = u64::try_from(row.try_get::<i64, _>("accepted_at")?).map_err(|_| bad())?;
            if !eligible(accepted_at, time) {
                continue;
            }
            let successor: String = row.try_get("successor_account_id")?;
            let device: Option<String> = sqlx::query_scalar("SELECT m.device_id FROM memberships m JOIN device_contacts dc ON dc.device_id=m.device_id WHERE dc.account_id=$1 AND m.active ORDER BY m.device_id LIMIT 1 FOR UPDATE OF m")
                .bind(&successor).fetch_optional(&mut **tx).await?;
            let Some(device) = device else { continue };
            sqlx::query("UPDATE server_ownership SET account_id=$1,claimed_by_device_id=$2,claimed_at=$3 WHERE singleton=TRUE")
                .bind(&successor).bind(&device).bind(int(time)?).execute(&mut **tx).await?;
            let members = sqlx::query("SELECT m.device_id,m.credential FROM memberships m JOIN device_contacts dc ON dc.device_id=m.device_id WHERE dc.account_id=$1 AND m.active FOR UPDATE OF m")
                .bind(&successor).fetch_all(&mut **tx).await?;
            for member in members {
                let device_id: String = member.try_get("device_id")?;
                let old: Signed<Membership> = parse(member.try_get("credential")?)?;
                let updated = s.sign(Membership {
                    version: 1,
                    server_id: s.0.identity.server_id(),
                    member_id: old.value.member_id,
                    account_id: successor.clone(),
                    device_id: device_id.clone(),
                    generation: old.value.generation.checked_add(1).ok_or_else(bad)?,
                    issued_at: time,
                    capabilities: super::member_capabilities(true),
                    status: "active".into(),
                })?;
                sqlx::query("UPDATE memberships SET credential=$1 WHERE device_id=$2")
                    .bind(json(&updated)?).bind(device_id).execute(&mut **tx).await?;
            }
            sqlx::query("UPDATE sessions SET active=FALSE WHERE device_id IN (SELECT device_id FROM device_contacts WHERE account_id=$1)")
                .bind(&successor).execute(&mut **tx).await?;
            sqlx::query("INSERT INTO server_owner_audit(id,action,actor_account_id,subject_account_id,occurred_at,details_hash) VALUES($1,'burn_successor_transfer',$2,$3,$4,$5)")
                .bind(id()).bind(account).bind(successor).bind(int(time)?).bind(row.try_get::<String, _>("id")?).execute(&mut **tx).await?;
            sqlx::query("DELETE FROM server_successor_designations").execute(&mut **tx).await?;
            sqlx::query("UPDATE server_owner_control SET locked_down=FALSE,recovery_code_hash=NULL,recovery_issued_at=NULL WHERE singleton=TRUE")
                .execute(&mut **tx).await?;
            sqlx::query("UPDATE server_ownership_bootstrap SET generation=generation+1 WHERE singleton=TRUE")
                .execute(&mut **tx).await?;
            return Ok(());
        }
    }
    sqlx::query("UPDATE server_owner_control SET locked_down=TRUE,recovery_code_hash=NULL,recovery_issued_at=NULL WHERE singleton=TRUE")
        .execute(&mut **tx).await?;
    sqlx::query("UPDATE server_ownership_bootstrap SET generation=generation+1 WHERE singleton=TRUE")
        .execute(&mut **tx).await?;
    sqlx::query("INSERT INTO server_owner_audit(id,action,actor_account_id,occurred_at,details_hash) VALUES($1,$2,$3,$4,$5)")
        .bind(id()).bind(if burn { "owner_burn_lockdown" } else { "owner_drop_lockdown" })
        .bind(account).bind(int(time)?).bind(hash(account)).execute(&mut **tx).await?;
    Ok(())
}

async fn depart<T: serde::Serialize + cords_protocol::messaging::Statement>(
    s: &Service,
    record: &Signed<T>,
    account: &str,
    server: &str,
    version: u16,
    issued_at: u64,
    nonce: &str,
    idempotency_key: &str,
    kind: &str,
) -> Result<Signed<DepartureReceipt>> {
    s.limit_authentication()?;
    let time = now()?;
    if version != 1 || server != s.0.identity.server_id() || nonce.len() != 43
        || issued_at > time.saturating_add(cords_identity::STATEMENT_CLOCK_SKEW_SECONDS)
        || idempotency_key.is_empty() || idempotency_key.len() > 128 {
        return Err(bad());
    }
    let request_hash = hash(canonical(record)?);
    let mut tx = s.0.store.pool().begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended('server-ownership',0))")
        .execute(&mut *tx).await?;
    let locked: bool = sqlx::query_scalar("SELECT locked_down FROM server_owner_control WHERE singleton=TRUE FOR SHARE")
        .fetch_one(&mut *tx).await?;
    if locked && kind == "drop" {
        return Err(ApiError(StatusCode::CONFLICT, "OWNER_RECOVERY_REQUIRED"));
    }
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(account).execute(&mut *tx).await?;
    let scope = format!("{kind}:{account}");
    if let Some(result) = previous(&mut tx, &scope, idempotency_key, record).await? {
        return Ok(result);
    }
    let prior: Option<String> = sqlx::query_scalar("SELECT receipt FROM server_departure_receipts WHERE account_id=$1 AND kind=$2 AND request_hash=$3")
        .bind(account).bind(kind).bind(&request_hash).fetch_optional(&mut *tx).await?;
    if let Some(prior) = prior {
        return parse(&prior);
    }
    let root = root_key(&mut tx, account).await?;
    record.verify(&root)?;
    if kind == "burn" {
        let already: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM identity_burns WHERE account_id=$1)")
            .bind(account).fetch_one(&mut *tx).await?;
        if already { return Err(conflict()); }
    }
    owner_departure(s, &mut tx, account, kind == "burn", time).await?;
    deactivate_account(&mut tx, account).await?;
    if kind == "burn" {
        sqlx::query("INSERT INTO identity_burns(account_id,record,accepted_at) VALUES($1,$2,$3)")
            .bind(account).bind(json(record)?).bind(int(time)?).execute(&mut *tx).await?;
    }
    let receipt = s.sign(DepartureReceipt {
        version: 1,
        server_id: s.0.identity.server_id(),
        account_id: account.into(),
        kind: kind.into(),
        request_hash: request_hash.clone(),
        accepted_at: time,
    })?;
    sqlx::query("INSERT INTO server_departure_receipts(account_id,kind,request_hash,receipt) VALUES($1,$2,$3,$4)")
        .bind(account).bind(kind).bind(request_hash).bind(json(&receipt)?).execute(&mut *tx).await?;
    remember(&mut tx, &scope, idempotency_key, record, &receipt).await?;
    tx.commit().await?;
    Ok(receipt)
}

pub(super) async fn drop_membership(State(s): State<Service>, Json(request): Json<DepartureRequest<ServerDeparture>>) -> Result<Json<Signed<DepartureReceipt>>> {
    let value = &request.record.value;
    Ok(Json(depart(&s, &request.record, &value.account_id, &value.server_id, value.version,
        value.issued_at, &value.nonce, &request.idempotency_key, "drop").await?))
}

pub(super) async fn burn_identity(State(s): State<Service>, Json(request): Json<DepartureRequest<IdentityBurn>>) -> Result<Json<Signed<DepartureReceipt>>> {
    let value = &request.record.value;
    Ok(Json(depart(&s, &request.record, &value.account_id, &value.server_id, value.version,
        value.issued_at, &value.nonce, &request.idempotency_key, "burn").await?))
}

pub(super) async fn designate_successor(State(s): State<Service>, Json(request): Json<SuccessorRequest<SuccessorDesignation>>) -> Result<Json<String>> {
    let designation = &request.record.value;
    if designation.version != 1 || designation.server_id != s.0.identity.server_id()
        || designation.owner_account_id == designation.successor_account_id
        || designation.nonce.len() != 43 { return Err(bad()); }
    let mut tx = s.0.store.pool().begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended('server-ownership',0))")
        .execute(&mut *tx).await?;
    let locked: bool = sqlx::query_scalar("SELECT locked_down FROM server_owner_control WHERE singleton=TRUE FOR SHARE")
        .fetch_one(&mut *tx).await?;
    if locked { return Err(ApiError(StatusCode::CONFLICT, "OWNER_RECOVERY_REQUIRED")); }
    let owner: String = sqlx::query_scalar("SELECT account_id FROM server_ownership WHERE singleton=TRUE FOR UPDATE")
        .fetch_one(&mut *tx).await?;
    if owner != designation.owner_account_id { return Err(denied()); }
    request.record.verify(&root_key(&mut tx, &owner).await?)?;
    let active: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM memberships m JOIN device_contacts d ON d.device_id=m.device_id WHERE d.account_id=$1 AND m.active)")
        .bind(&designation.successor_account_id).fetch_one(&mut *tx).await?;
    if !active { return Err(denied()); }
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM server_successor_designations)")
        .fetch_one(&mut *tx).await?;
    if exists { return Err(conflict()); }
    let designation_hash = hash(canonical(&request.record)?);
    sqlx::query("INSERT INTO server_successor_designations(id,owner_account_id,successor_account_id,designation,accepted_at) VALUES($1,$2,$3,$4,$5)")
        .bind(&designation_hash).bind(&owner).bind(&designation.successor_account_id)
        .bind(json(&request.record)?).bind(int(now()?)?).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO server_owner_audit(id,action,actor_account_id,subject_account_id,occurred_at,details_hash) VALUES($1,'successor_designated',$2,$3,$4,$5)")
        .bind(id()).bind(owner).bind(&designation.successor_account_id).bind(int(now()?)?)
        .bind(&designation_hash).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(Json(designation_hash))
}

pub(super) async fn accept_successor(State(s): State<Service>, Json(request): Json<SuccessorRequest<SuccessorAcceptance>>) -> Result<Json<bool>> {
    let value = &request.record.value;
    if value.version != 1 || value.server_id != s.0.identity.server_id() || value.nonce.len() != 43 { return Err(bad()); }
    let mut tx = s.0.store.pool().begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended('server-ownership',0))")
        .execute(&mut *tx).await?;
    let row = sqlx::query("SELECT successor_account_id,acceptance FROM server_successor_designations WHERE id=$1 FOR UPDATE")
        .bind(&value.designation_hash).fetch_optional(&mut *tx).await?.ok_or_else(denied)?;
    if row.try_get::<String, _>("successor_account_id")? != value.successor_account_id ||
        row.try_get::<Option<String>, _>("acceptance")?.is_some() { return Err(conflict()); }
    request.record.verify(&root_key(&mut tx, &value.successor_account_id).await?)?;
    sqlx::query("UPDATE server_successor_designations SET acceptance=$1,successor_accepted_at=$2 WHERE id=$3")
        .bind(json(&request.record)?).bind(int(now()?)?).bind(&value.designation_hash)
        .execute(&mut *tx).await?;
    sqlx::query("INSERT INTO server_owner_audit(id,action,actor_account_id,occurred_at,details_hash) VALUES($1,'successor_accepted',$2,$3,$4)")
        .bind(id()).bind(&value.successor_account_id).bind(int(now()?)?).bind(&value.designation_hash)
        .execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(Json(true))
}

pub(super) async fn recovery_challenge(State(s): State<Service>, Json(request): Json<ChallengeRequest>) -> Result<Json<super::Challenge>> {
    if request.purpose != "recover_ownership" { return Err(bad()); }
    s.limit_ownership()?;
    let locked: bool = sqlx::query_scalar("SELECT locked_down FROM server_owner_control WHERE singleton=TRUE")
        .fetch_one(s.0.store.pool()).await?;
    if !locked { return Err(conflict()); }
    super::challenge(&s, request).await.map(Json)
}

pub(super) async fn recover_owner(State(s): State<Service>, Json(request): Json<OwnershipRecoveryClaim>) -> Result<Json<super::Session>> {
    s.limit_ownership()?;
    if request.recovery_code.len() != 43 || request.idempotency_key.is_empty()
        || request.idempotency_key.len() > 128 { return Err(denied()); }
    let time = now()?;
    cords_identity::validate_contact(&request.contact, time)?;
    let authorization = &request.contact.authorization.value;
    request.device_proof.verify(&authorization.device_public_key)?;
    request.root_proof.verify(&authorization.root_public_key)?;
    let proof = &request.root_proof.value;
    let challenge = &request.device_proof.value;
    if proof.version != 1 || proof.server_id != s.0.identity.server_id()
        || proof.account_id != authorization.account_id || proof.device_id != authorization.device_id
        || proof.challenge_hash != hash(canonical(challenge)?) { return Err(denied()); }
    let mut tx = s.0.store.pool().begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended('server-ownership',0))")
        .execute(&mut *tx).await?;
    let control = sqlx::query("SELECT locked_down,recovery_code_hash FROM server_owner_control WHERE singleton=TRUE FOR UPDATE")
        .fetch_one(&mut *tx).await?;
    if !control.try_get::<bool, _>("locked_down")? { return Err(conflict()); }
    let expected_hash: Option<String> = control.try_get("recovery_code_hash")?;
    let expected_hash = expected_hash.ok_or_else(denied)?;
    let presented = crate::ownership::recovery_code_hash(&s.0.identity.server_id(), &request.recovery_code);
    if !bool::from(expected_hash.as_bytes().ct_eq(presented.as_bytes())) { return Err(denied()); }
    let row = sqlx::query("SELECT challenge,contact,consumed FROM auth_challenges WHERE id=$1 FOR UPDATE")
        .bind(&challenge.challenge_id).fetch_optional(&mut *tx).await?.ok_or_else(denied)?;
    let expected: super::Challenge = parse(row.try_get("challenge")?)?;
    let contact: Contact = parse(row.try_get("contact")?)?;
    if row.try_get::<bool, _>("consumed")? || expected.purpose != "recover_ownership"
        || expected.expires_at <= time || expected.server_id != s.0.identity.server_id()
        || canonical(&expected)? != canonical(challenge)?
        || canonical(&contact)? != canonical(&request.contact)? { return Err(denied()); }
    let burned: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM identity_burns WHERE account_id=$1)")
        .bind(&authorization.account_id).fetch_one(&mut *tx).await?;
    if burned { return Err(denied()); }
    super::persist_contact(&mut tx, &request.contact).await?;
    let old: Option<String> = sqlx::query_scalar("SELECT credential FROM memberships WHERE device_id=$1 FOR UPDATE")
        .bind(&authorization.device_id).fetch_optional(&mut *tx).await?;
    let (member_id,generation) = if let Some(old) = old {
        let old: Signed<Membership> = parse(&old)?;
        (old.value.member_id,old.value.generation.checked_add(1).ok_or_else(bad)?)
    } else { (id(),1) };
    let credential = s.sign(Membership { version: 1,server_id: s.0.identity.server_id(),
        member_id,account_id: authorization.account_id.clone(),device_id: authorization.device_id.clone(),
        generation,issued_at: time,capabilities: super::member_capabilities(true),status: "active".into() })?;
    sqlx::query("INSERT INTO memberships(device_id,credential,active) VALUES($1,$2,TRUE) ON CONFLICT(device_id) DO UPDATE SET credential=EXCLUDED.credential,active=TRUE")
        .bind(&authorization.device_id).bind(json(&credential)?).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO server_ownership(singleton,account_id,claimed_by_device_id,claimed_at) VALUES(TRUE,$1,$2,$3) ON CONFLICT(singleton) DO UPDATE SET account_id=EXCLUDED.account_id,claimed_by_device_id=EXCLUDED.claimed_by_device_id,claimed_at=EXCLUDED.claimed_at")
        .bind(&authorization.account_id).bind(&authorization.device_id).bind(int(time)?).execute(&mut *tx).await?;
    sqlx::query("UPDATE server_owner_control SET locked_down=FALSE,recovery_code_hash=NULL,recovery_issued_at=NULL,recovery_generation=recovery_generation+1 WHERE singleton=TRUE")
        .execute(&mut *tx).await?;
    sqlx::query("UPDATE server_ownership_bootstrap SET generation=generation+1 WHERE singleton=TRUE")
        .execute(&mut *tx).await?;
    sqlx::query("DELETE FROM server_successor_designations").execute(&mut *tx).await?;
    let expires_at = time + s.0.session_seconds;
    let token = super::session_token(&s, &expected.challenge_id, expires_at)?;
    sqlx::query("INSERT INTO sessions(token_hash,device_id,expires_at,challenge_id,request_hash,idempotency_key) VALUES($1,$2,$3,$4,$5,$6)")
        .bind(hash(&token)).bind(&authorization.device_id).bind(int(expires_at)?)
        .bind(&expected.challenge_id).bind(hash(canonical(&request.root_proof)?))
        .bind(&request.idempotency_key).execute(&mut *tx).await?;
    sqlx::query("UPDATE auth_challenges SET consumed=TRUE WHERE id=$1")
        .bind(&expected.challenge_id).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO server_owner_audit(id,action,actor_account_id,occurred_at,details_hash) VALUES($1,'operator_recovery',$2,$3,$4)")
        .bind(id()).bind(&authorization.account_id).bind(int(time)?).bind(hash(canonical(&request.root_proof)?))
        .execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(Json(super::Session { token,expires_at,membership: credential }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn successor_matures_only_at_full_thirty_day_boundary() {
        assert!(!eligible(100, 100 + MATURATION_SECONDS - 1));
        assert!(eligible(100, 100 + MATURATION_SECONDS));
    }
}
