//! Account-authorized revocation remains independent of server membership policy.
use super::{
    Json, Result, RosterAction, RosterOperation, Service, Signed, State, Tx, bad, canonical,
    channel_view_in, conflict, denied, hash, id, int, json, now, parse, previous, remember, uint,
    validate_transition,
};
use axum::{extract::Path, http::HeaderMap};
use cords_protocol::messaging::{
    ChannelBind, DepartureProof, DeviceAuthorization, DeviceRevocation, PolicyRemoval,
    RevocationRequest,
};

pub(super) async fn bind(
    State(s): State<Service>,
    headers: HeaderMap,
    Path(route): Path<String>,
    Json(request): Json<ChannelBind>,
) -> Result<Json<bool>> {
    let member = s.authenticate(&headers, "channel.mls.commit").await?;
    let mut tx = s.0.store.pool().begin().await?;
    s.authorize_mutation(&mut tx, &headers, &member, Some((&route, true)))
        .await?;
    let scope = format!("binding:{route}:{}", member.value.device_id);
    if let Some(result) = previous(&mut tx, &scope, &request.idempotency_key, &request).await? {
        return Ok(Json(result));
    }
    let channel = channel_view_in(&mut tx, &route).await?;
    let creator = channel
        .members
        .iter()
        .find(|c| c.authorization.value.device_id == member.value.device_id)
        .ok_or_else(denied)?;
    request
        .binding
        .verify(&creator.authorization.value.device_public_key)?;
    let binding = &request.binding.value;
    if binding.version != 1
        || binding.server_id != s.0.identity.server_id()
        || binding.channel_id != route
        || binding.group_id != route
        || binding.creator_device_id != member.value.device_id
    {
        return Err(bad());
    }
    if channel.epoch != 0 || channel.binding.is_some() {
        return Err(conflict());
    }
    sqlx::query("UPDATE channels SET binding=$1 WHERE id=$2")
        .bind(json(&request.binding)?)
        .bind(&route)
        .execute(&mut *tx)
        .await?;
    snapshot_epoch(&mut tx, &route).await?;
    remember(&mut tx, &scope, &request.idempotency_key, &request, &true).await?;
    tx.commit().await?;
    Ok(Json(true))
}

pub(super) async fn apply_remove(tx: &mut Tx<'_>, route: &str, target: &str) -> Result<()> {
    sqlx::query("UPDATE channel_members SET active=FALSE,delivery_active=FALSE WHERE channel_id=$1 AND device_id=$2")
        .bind(route).bind(target).execute(&mut **tx).await?;
    sqlx::query("DELETE FROM pending_policy_removals WHERE channel_id=$1 AND device_id=$2")
        .bind(route)
        .bind(target)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

pub(super) async fn apply_add(
    tx: &mut Tx<'_>,
    route: &str,
    target: &str,
    sequence: u64,
    welcome: &str,
    package: &str,
) -> Result<()> {
    sqlx::query("INSERT INTO channel_members(channel_id,device_id,joined_sequence) VALUES($1,$2,$3) ON CONFLICT(channel_id,device_id) DO UPDATE SET active=TRUE,delivery_active=TRUE,joined_sequence=EXCLUDED.joined_sequence")
        .bind(route).bind(target).bind(int(sequence)?).execute(&mut **tx).await?;
    sqlx::query("INSERT INTO welcomes(channel_id,device_id,sequence,welcome) VALUES($1,$2,$3,$4) ON CONFLICT(channel_id,device_id) DO UPDATE SET sequence=EXCLUDED.sequence,welcome=EXCLUDED.welcome")
        .bind(route).bind(target).bind(int(sequence)?).bind(welcome).execute(&mut **tx).await?;
    sqlx::query("UPDATE key_packages SET consumed=TRUE WHERE id=$1")
        .bind(package)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

pub(super) async fn pending_removals(
    State(s): State<Service>,
    headers: HeaderMap,
    Path(route): Path<String>,
) -> Result<Json<Vec<PolicyRemoval>>> {
    let member = s.authenticate(&headers, "channel.mls.commit").await?;
    s.route_access(&route, &member.value, true).await?;
    let records: Vec<String> = sqlx::query_scalar("SELECT r.record FROM pending_policy_removals p JOIN device_revocations r ON r.device_id=p.device_id WHERE p.channel_id=$1 ORDER BY p.device_id LIMIT 1000")
        .bind(&route).fetch_all(s.0.store.pool()).await?;
    let mut result: Vec<PolicyRemoval> = records
        .iter()
        .map(|record| parse(record).map(PolicyRemoval::Revocation))
        .collect::<Result<_>>()?;
    let departures = sqlx::query("SELECT p.device_id,n.kind,n.record FROM pending_policy_removals p JOIN device_contacts d ON d.device_id=p.device_id JOIN account_departure_notices n ON n.account_id=d.account_id WHERE p.channel_id=$1 AND NOT EXISTS(SELECT 1 FROM device_revocations r WHERE r.device_id=p.device_id) ORDER BY p.device_id,n.kind LIMIT 1000")
        .bind(route).fetch_all(s.0.store.pool()).await?;
    for row in departures {
        use sqlx::Row as _;
        let proof = match row.try_get::<String, _>("kind")?.as_str() {
            "drop" => DepartureProof::Drop(parse(row.try_get("record")?)?),
            "burn" => DepartureProof::Burn(parse(row.try_get("record")?)?),
            _ => return Err(bad()),
        };
        result.push(PolicyRemoval::Account {
            device_id: row.try_get("device_id")?,
            proof,
        });
    }
    Ok(Json(result))
}

pub(super) async fn snapshot_epoch(tx: &mut Tx<'_>, route: &str) -> Result<()> {
    let view = channel_view_in(tx, route).await?;
    sqlx::query("INSERT INTO channel_epochs(channel_id,epoch,view) VALUES($1,$2,$3) ON CONFLICT(channel_id,epoch) DO NOTHING")
        .bind(route).bind(int(view.epoch)?).bind(json(&view)?).execute(&mut **tx).await?;
    Ok(())
}

pub(super) async fn reserve_removal(
    tx: &mut Tx<'_>,
    route: &str,
    requester: &str,
    target: &str,
    epoch: i64,
) -> Result<RosterOperation> {
    let contact: String =
        sqlx::query_scalar("SELECT contact FROM device_contacts WHERE device_id=$1")
            .bind(target)
            .fetch_one(&mut **tx)
            .await?;
    let operation_id = id();
    sqlx::query("INSERT INTO roster_operations(id,channel_id,requester,target,base_epoch,action) VALUES($1,$2,$3,$4,$5,'remove')")
        .bind(&operation_id).bind(route).bind(requester).bind(target).bind(epoch).execute(&mut **tx).await?;
    sqlx::query(
        "UPDATE channel_members SET delivery_active=FALSE WHERE channel_id=$1 AND device_id=$2",
    )
    .bind(route)
    .bind(target)
    .execute(&mut **tx)
    .await?;
    Ok(RosterOperation {
        action: RosterAction::Remove,
        operation_id,
        base_epoch: uint(epoch)?,
        target: super::roster_contact(&contact)?,
        key_package: String::new(),
    })
}

pub(super) async fn validate_current_head(
    tx: &mut Tx<'_>,
    head: &Signed<DeviceAuthorization>,
    next: &Signed<DeviceAuthorization>,
) -> Result<()> {
    let latest: Option<String> = sqlx::query_scalar("SELECT record FROM device_revocations WHERE account_id=$1 ORDER BY generation DESC LIMIT 1")
        .bind(&head.value.account_id).fetch_optional(&mut **tx).await?;
    if let Some(latest) = latest {
        let latest: Signed<DeviceRevocation> = parse(&latest)?;
        if latest.value.generation > head.value.generation {
            next.verify(&head.value.root_public_key)?;
            if next.value.account_id != head.value.account_id
                || next.value.root_public_key != head.value.root_public_key
                || next.value.generation
                    != latest.value.generation.checked_add(1).ok_or_else(bad)?
                || next.value.previous_record_hash != Some(hash(canonical(&latest)?))
            {
                return Err(denied());
            }
            return Ok(());
        }
    }
    validate_transition(head, next)?;
    Ok(())
}

pub(super) async fn revoke(
    State(s): State<Service>,
    Json(request): Json<RevocationRequest>,
) -> Result<Json<bool>> {
    s.limit_authentication()?;
    let record = &request.record.value;
    if record.version != 1
        || record.issued_at > now()?.saturating_add(cords_identity::STATEMENT_CLOCK_SKEW_SECONDS)
    {
        return Err(bad());
    }
    let mut tx = s.0.store.pool().begin().await?;
    let scope = format!("revoke:{}", record.account_id);
    sqlx::query("SELECT pg_advisory_xact_lock_shared(hashtextextended('server-ownership',0))")
        .execute(&mut *tx)
        .await?;
    if let Some(result) = previous(&mut tx, &scope, &request.idempotency_key, &request).await? {
        return Ok(Json(result));
    }
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(&record.account_id)
        .execute(&mut *tx)
        .await?;
    let locked: bool = sqlx::query_scalar(
        "SELECT locked_down FROM server_owner_control WHERE singleton=TRUE FOR SHARE",
    )
    .fetch_one(&mut *tx)
    .await?;
    if locked {
        return Err(super::ApiError(
            super::StatusCode::CONFLICT,
            "OWNER_RECOVERY_REQUIRED",
        ));
    }
    let head: String =
        sqlx::query_scalar("SELECT authorization_record FROM account_heads WHERE account_id=$1")
            .bind(&record.account_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(denied)?;
    let head: Signed<DeviceAuthorization> = parse(&head)?;
    request.record.verify(&head.value.root_public_key)?;
    let latest: Option<String> = sqlx::query_scalar("SELECT record FROM device_revocations WHERE account_id=$1 ORDER BY generation DESC LIMIT 1")
        .bind(&record.account_id).fetch_optional(&mut *tx).await?;
    let mut generation = head.value.generation;
    let mut previous_hash = hash(canonical(&head)?);
    if let Some(latest) = latest {
        let latest: Signed<DeviceRevocation> = parse(&latest)?;
        if latest.value.generation > generation {
            generation = latest.value.generation;
            previous_hash = hash(canonical(&latest)?);
        }
    }
    if record.generation != generation.checked_add(1).ok_or_else(bad)?
        || record.previous_state_hash != previous_hash
    {
        return Err(conflict());
    }
    let owner: Option<String> =
        sqlx::query_scalar("SELECT account_id FROM device_contacts WHERE device_id=$1")
            .bind(&record.revoked_device_id)
            .fetch_optional(&mut *tx)
            .await?;
    if owner.as_deref() != Some(&record.account_id) {
        return Err(denied());
    }
    sqlx::query("INSERT INTO device_revocations(device_id,account_id,generation,record) VALUES($1,$2,$3,$4)")
        .bind(&record.revoked_device_id).bind(&record.account_id).bind(int(record.generation)?).bind(json(&request.record)?).execute(&mut *tx).await?;
    sqlx::query("UPDATE memberships SET active=FALSE WHERE device_id=$1")
        .bind(&record.revoked_device_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE sessions SET active=FALSE WHERE device_id=$1")
        .bind(&record.revoked_device_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO pending_policy_removals(channel_id,device_id) SELECT channel_id,device_id FROM channel_members WHERE device_id=$1 AND active ON CONFLICT DO NOTHING")
        .bind(&record.revoked_device_id).execute(&mut *tx).await?;
    sqlx::query("UPDATE channel_members SET delivery_active=FALSE WHERE device_id=$1")
        .bind(&record.revoked_device_id)
        .execute(&mut *tx)
        .await?;
    remember(&mut tx, &scope, &request.idempotency_key, &request, &true).await?;
    tx.commit().await?;
    Ok(Json(true))
}
