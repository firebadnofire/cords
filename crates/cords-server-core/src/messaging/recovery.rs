//! Explicit owner-authorized abandonment of a fenced encrypted predecessor.
use super::*;
use cords_protocol::roles::{ChannelRecoveryCandidate, Permission};

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RecoveryQuery {
    #[serde(default)]
    after: String,
}

async fn pending_digest(tx: &mut Tx<'_>, route: &str) -> Result<Option<String>> {
    let pending: Vec<String> = sqlx::query_scalar("SELECT device_id FROM pending_policy_removals WHERE channel_id=$1 ORDER BY device_id LIMIT 1001")
        .bind(route).fetch_all(&mut **tx).await?;
    let operations: Vec<(String, String, String, i64)> = sqlx::query_as("SELECT id,target,action,base_epoch FROM roster_operations WHERE channel_id=$1 AND NOT complete AND NOT cancelled ORDER BY id LIMIT 1001")
        .bind(route).fetch_all(&mut **tx).await?;
    if pending.len() > 1000 || operations.len() > 1000 {
        return Err(ApiError(StatusCode::CONFLICT, "CORDS_RECOVERY_QUEUE_LIMIT"));
    }
    if pending.is_empty() && !operations.iter().any(|operation| operation.2 == "remove") {
        return Ok(None);
    }
    Ok(Some(hash(canonical(&(pending, operations))?)))
}

pub(super) async fn candidate(
    s: &Service,
    tx: &mut Tx<'_>,
    member: &Membership,
    route: &str,
) -> Result<Option<ChannelRecoveryCandidate>> {
    use crate::authorization::{persistence, runtime};
    s.require_current_policy(tx, member, Permission::CreateChannels, None)
        .await?;
    s.require_current_policy(tx, member, Permission::ManageChannel, Some(route))
        .await?;
    s.require_current_policy(tx, member, Permission::RetireChannel, Some(route))
        .await?;
    let snapshot = persistence::load(tx).await?;
    if !snapshot.active {
        return Ok(None);
    }
    let row = sqlx::query("SELECT name,creator,created_at,epoch,retired,confidentiality_mode FROM channels WHERE id=$1 FOR SHARE")
        .bind(route).fetch_optional(&mut **tx).await?.ok_or_else(denied)?;
    if row.try_get::<bool, _>("retired")?
        || row.try_get::<String, _>("confidentiality_mode")? != "encrypted"
    {
        return Ok(None);
    }
    let Some(pending_digest) = pending_digest(tx, route).await? else {
        return Ok(None);
    };
    let committer_available = runtime::has_eligible_committer(
        tx,
        route,
        &s.0.identity.server_id(),
        &encode(s.0.identity.signing_key.verifying_key().to_bytes()),
        now()?,
    )
    .await?;
    Ok(Some(ChannelRecoveryCandidate {
        version: 1,
        server_id: s.0.identity.server_id(),
        account_id: member.account_id.clone(),
        device_id: member.device_id.clone(),
        predecessor: s.sign(ChannelIdentity {
            version: 1,
            server_id: s.0.identity.server_id(),
            channel_id: route.into(),
            name: row.try_get("name")?,
            creator_device_id: row.try_get("creator")?,
            confidentiality_mode: ConfidentialityMode::Encrypted,
            created_at: uint(row.try_get("created_at")?)?,
        })?,
        policy_revision: uint(snapshot.revision)?,
        channel_epoch: uint(row.try_get("epoch")?)?,
        pending_digest,
        committer_available,
    }))
}

pub(super) async fn list(
    State(s): State<Service>,
    headers: HeaderMap,
    Query(query): Query<RecoveryQuery>,
) -> Result<Json<Vec<Signed<ChannelRecoveryCandidate>>>> {
    if query.after.len() > 128 {
        return Err(bad());
    }
    let member = s.authenticate(&headers, "channel.read").await?;
    let mut tx = s.0.store.pool().begin().await?;
    s.authorize_mutation(&mut tx, &headers, &member, None)
        .await?;
    let owner: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM server_ownership WHERE account_id=$1)")
            .bind(&member.value.account_id)
            .fetch_one(&mut *tx)
            .await?;
    if !owner {
        return Err(denied());
    }
    let active: bool = sqlx::query_scalar("SELECT active FROM role_policy WHERE singleton=TRUE")
        .fetch_one(&mut *tx)
        .await?;
    let mut result = Vec::new();
    if active {
        s.require_current_policy(&mut tx, &member.value, Permission::CreateChannels, None)
            .await?;
        let routes: Vec<String> = sqlx::query_scalar("SELECT c.id FROM channels c WHERE c.id>$1 AND NOT c.retired AND c.confidentiality_mode='encrypted' AND (EXISTS(SELECT 1 FROM pending_policy_removals p WHERE p.channel_id=c.id) OR EXISTS(SELECT 1 FROM roster_operations o WHERE o.channel_id=c.id AND o.action='remove' AND NOT o.complete AND NOT o.cancelled)) ORDER BY c.id LIMIT 16")
            .bind(query.after).fetch_all(&mut *tx).await?;
        for route in routes {
            if let Some(value) = candidate(&s, &mut tx, &member.value, &route).await? {
                result.push(s.sign(value)?);
            }
        }
    }
    tx.commit().await?;
    Ok(Json(result))
}

pub(super) async fn validate(
    s: &Service,
    tx: &mut Tx<'_>,
    member: &Membership,
    route: &str,
    request: &ChannelReplace,
) -> Result<()> {
    let proof = request.recovery.as_ref().ok_or_else(bad)?;
    let v = &request.record.value;
    proof.verify(&v.initiator.authorization.value.device_public_key)?;
    let current = candidate(s, tx, member, route)
        .await?
        .ok_or_else(conflict)?;
    let p = &proof.value;
    if current.committer_available
        || v.confidentiality_mode != ConfidentialityMode::Encrypted
        || p.version != 1
        || p.server_id != current.server_id
        || p.account_id != member.account_id
        || p.device_id != member.device_id
        || p.channel_id != route
        || p.policy_revision != current.policy_revision
        || p.channel_epoch != current.channel_epoch
        || p.pending_digest != current.pending_digest
        || p.succession_digest != hash(canonical(&request.record)?)
        || canonical(&v.predecessor)? != canonical(&current.predecessor)?
    {
        return Err(ApiError(
            StatusCode::CONFLICT,
            "CORDS_RECOVERY_STATE_CHANGED",
        ));
    }
    Ok(())
}

pub(super) async fn record(tx: &mut Tx<'_>, route: &str, request: &ChannelReplace) -> Result<()> {
    let proof = request.recovery.as_ref().ok_or_else(bad)?;
    let operations: Vec<String> = sqlx::query_scalar("UPDATE roster_operations SET cancelled=TRUE WHERE channel_id=$1 AND NOT complete AND NOT cancelled RETURNING id")
        .bind(route).fetch_all(&mut **tx).await?;
    // A prepared Welcome may already have used these one-time packages locally.
    // Burn their reservations; never return them to the public pool for reuse.
    sqlx::query("UPDATE key_packages SET consumed=TRUE WHERE reserved_by=ANY($1)")
        .bind(&operations)
        .execute(&mut **tx)
        .await?;
    let p = &proof.value;
    sqlx::query("INSERT INTO channel_recovery_audit(predecessor_id,successor_id,actor_device_id,policy_revision,channel_epoch,pending_digest,recovery,succession,cancelled_operations) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)")
        .bind(route).bind(&request.record.value.successor_channel_id).bind(&p.device_id)
        .bind(int(p.policy_revision)?).bind(int(p.channel_epoch)?).bind(&p.pending_digest)
        .bind(json(proof)?).bind(json(&request.record)?).bind(json(&operations)?).execute(&mut **tx).await?;
    Ok(())
}
