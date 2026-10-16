//! Authenticated account profile history and separate, bounded JPEG XL storage.
use super::*;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use cords_protocol::messaging::DeviceAuthorization;
use cords_protocol::profiles::{
    ProfileGraph, ProfileHistoryPage, ProfilePublish, ProfileReceipt, ProfileRecord,
};

const HISTORY_BYTES: i64 = 16 * 1024 * 1024;

async fn readable(
    s: &Service,
    tx: &mut Tx<'_>,
    headers: &HeaderMap,
    member: &Signed<Membership>,
    account: &str,
) -> Result<()> {
    if decode(account)?.len() != 32 {
        return Err(bad());
    }
    s.authorize_session(tx, headers, member).await?;
    s.require_current_policy(
        tx,
        &member.value,
        cords_protocol::roles::Permission::ViewServer,
        None,
    )
    .await?;
    let burned: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM identity_burns WHERE account_id=$1)")
            .bind(account)
            .fetch_one(&mut **tx)
            .await?;
    if burned {
        return Err(denied());
    }
    Ok(())
}

async fn heads(tx: &mut Tx<'_>, account: &str) -> Result<Vec<String>> {
    let result: Vec<String> = sqlx::query_scalar(
        "SELECT p.id FROM profile_revisions p WHERE p.account_id=$1 AND NOT EXISTS(SELECT 1 FROM profile_revisions child WHERE child.account_id=p.account_id AND p.id=ANY(child.parents)) ORDER BY p.id COLLATE \"C\" LIMIT 65"
    ).bind(account).fetch_all(&mut **tx).await?;
    if result.len() > 64 {
        return Err(conflict());
    }
    Ok(result)
}

pub(super) async fn history(
    State(s): State<Service>,
    headers: HeaderMap,
    Path(account): Path<String>,
    Query(page): Query<MembershipPage>,
) -> Result<Json<ProfileHistoryPage>> {
    if page.after.len() > 43 || (!page.after.is_empty() && decode(&page.after)?.len() != 32) {
        return Err(bad());
    }
    let member = s.authenticate(&headers, "channel.read").await?;
    let mut tx = s.0.store.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")
        .execute(&mut *tx)
        .await?;
    readable(&s, &mut tx, &headers, &member, &account).await?;
    let rows = sqlx::query("SELECT id,record FROM profile_revisions WHERE account_id=$1 AND id COLLATE \"C\">$2 COLLATE \"C\" ORDER BY id COLLATE \"C\" LIMIT 17")
        .bind(&account).bind(&page.after).fetch_all(&mut *tx).await?;
    let records = rows
        .iter()
        .take(16)
        .map(|row| parse(row.try_get("record")?))
        .collect::<Result<Vec<ProfileRecord>>>()?;
    let next_after = if rows.len() > 16 {
        Some(rows[15].try_get("id")?)
    } else {
        None
    };
    let response = ProfileHistoryPage {
        version: 1,
        server_id: s.0.identity.server_id(),
        account_id: account.clone(),
        records,
        heads: heads(&mut tx, &account).await?,
        next_after,
    };
    tx.commit().await?;
    Ok(Json(response))
}

pub(super) async fn image(
    State(s): State<Service>,
    headers: HeaderMap,
    Path((account, image)): Path<(String, String)>,
) -> Result<Response> {
    if decode(&image)?.len() != 32 {
        return Err(bad());
    }
    let member = s.authenticate(&headers, "channel.read").await?;
    let mut tx = s.0.store.pool().begin().await?;
    readable(&s, &mut tx, &headers, &member, &account).await?;
    let bytes: Vec<u8> = sqlx::query_scalar("SELECT i.encoded FROM profile_images i WHERE i.sha256=$2 AND EXISTS(SELECT 1 FROM profile_revisions p WHERE p.account_id=$1 AND p.avatar_hash=i.sha256)")
        .bind(&account).bind(&image).fetch_optional(&mut *tx).await?.ok_or(ApiError(StatusCode::NOT_FOUND, "CORDS_PROFILE_IMAGE_MISSING"))?;
    tx.commit().await?;
    Ok((
        [
            ("content-type", "image/jxl"),
            ("cache-control", "no-store"),
            ("x-content-type-options", "nosniff"),
        ],
        bytes,
    )
        .into_response())
}

pub(super) async fn publish(
    State(s): State<Service>,
    request: axum::extract::Request,
) -> Result<Json<ProfileReceipt>> {
    let headers = request.headers().clone();
    let member = s.authenticate(&headers, "channel.read").await?;
    s.limit_authentication()?;
    // Bound body parsing, validation and graph reconstruction as one operation.
    let permit =
        s.0.profile_validation
            .clone()
            .try_acquire_owned()
            .map_err(|_| ApiError(StatusCode::TOO_MANY_REQUESTS, "CORDS_PROFILE_BUSY"))?;
    let bytes = axum::body::to_bytes(request.into_body(), 8 * 1024 * 1024)
        .await
        .map_err(|_| bad())?;
    let request: ProfilePublish = serde_json::from_slice(&bytes)?;
    drop(bytes);
    if json(&request.record)?.len() > 32768
        || request.record.revision.value.account_id != member.value.account_id
    {
        return Err(denied());
    }
    request.record.verify()?;
    // Decode before taking policy/account locks. Revocation must never wait on a
    // 15-second codec operation; authorization is checked again when committing.
    let (prepared, _permit) = prepare_image(&s, &request, permit).await?;
    let id = request.record.id()?;
    let account = &request.record.revision.value.account_id;
    let mut tx = s.0.store.pool().begin().await?;
    // Account authorization uses the same lock as revocation/burn. Upgrade the
    // account lock before authorize_session's shared lock to avoid upgrade races.
    sqlx::query("SELECT pg_advisory_xact_lock_shared(hashtextextended('server-ownership',0))")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(account)
        .execute(&mut *tx)
        .await?;
    readable(&s, &mut tx, &headers, &member, account).await?;
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM profile_revisions WHERE id=$1 AND account_id=$2)",
    )
    .bind(&id)
    .bind(account)
    .fetch_one(&mut *tx)
    .await?;
    if !exists {
        authorize_author(&mut tx, &request.record).await?;
        let mut graph = load_graph(&mut tx, account).await?;
        if request
            .record
            .revision
            .value
            .parents
            .iter()
            .any(|parent| !graph.records.contains_key(parent))
        {
            return Err(ApiError(
                StatusCode::CONFLICT,
                "CORDS_PROFILE_ANCESTRY_REQUIRED",
            ));
        }
        graph.insert(request.record.clone())?;
        let state = graph.state()?;
        if !state.pending.is_empty() {
            return Err(conflict());
        }
        if let Some(bytes) = prepared {
            let image = request
                .record
                .revision
                .value
                .profile
                .avatar
                .as_ref()
                .ok_or_else(bad)?;
            sqlx::query("INSERT INTO profile_images(sha256,encoded,width,height) VALUES($1,$2,$3,$4) ON CONFLICT(sha256) DO NOTHING")
                .bind(&image.sha256).bind(bytes).bind(i32::from(image.width)).bind(i32::from(image.height)).execute(&mut *tx).await?;
        }
        let r = &request.record.revision.value;
        sqlx::query("INSERT INTO profile_revisions(id,account_id,device_id,generation,record,avatar_hash,parents) VALUES($1,$2,$3,$4,$5,$6,$7)")
            .bind(&id).bind(account).bind(&r.device_id).bind(int(r.generation)?).bind(json(&request.record)?)
            .bind(r.profile.avatar.as_ref().map(|image| &image.sha256)).bind(&r.parents).execute(&mut *tx).await?;
    }
    let receipt = ProfileReceipt {
        id,
        heads: heads(&mut tx, account).await?,
    };
    tx.commit().await?;
    Ok(Json(receipt))
}

async fn authorize_author(tx: &mut Tx<'_>, record: &ProfileRecord) -> Result<()> {
    cords_identity::validate_authorization(&record.authorization, now()?)?;
    let a = &record.authorization.value;
    let revoked: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM device_revocations WHERE device_id=$1)")
            .bind(&a.device_id)
            .fetch_one(&mut **tx)
            .await?;
    if revoked {
        return Err(denied());
    }
    let head: Option<String> =
        sqlx::query_scalar("SELECT authorization_record FROM account_heads WHERE account_id=$1")
            .bind(&a.account_id)
            .fetch_optional(&mut **tx)
            .await?;
    let head: Signed<DeviceAuthorization> = parse(head.as_deref().ok_or_else(denied)?)?;
    if head.value.root_public_key != a.root_public_key {
        return Err(denied());
    }
    // Forwarding another device's signed history is allowed only if its root
    // authorization is already observed, or validly extends the observed head.
    let known: Option<String> = sqlx::query_scalar(
        "SELECT contact FROM device_contacts WHERE account_id=$1 AND device_id=$2",
    )
    .bind(&a.account_id)
    .bind(&a.device_id)
    .fetch_optional(&mut **tx)
    .await?;
    if let Some(known) = known {
        let contact: Contact = parse(&known)?;
        if canonical(&contact.authorization)? != canonical(&record.authorization)? {
            return Err(denied());
        }
    } else {
        lifecycle::validate_current_head(tx, &head, &record.authorization).await?;
    }
    Ok(())
}

async fn load_graph(tx: &mut Tx<'_>, account: &str) -> Result<ProfileGraph> {
    let size: i64 = sqlx::query_scalar("SELECT COALESCE(SUM(octet_length(record)),0)::BIGINT FROM profile_revisions WHERE account_id=$1")
        .bind(account).fetch_one(&mut **tx).await?;
    if size > HISTORY_BYTES - 32768 {
        return Err(ApiError(
            StatusCode::INSUFFICIENT_STORAGE,
            "CORDS_PROFILE_HISTORY_QUOTA",
        ));
    }
    let rows =
        sqlx::query("SELECT id,record FROM profile_revisions WHERE account_id=$1 LIMIT 16385")
            .bind(account)
            .fetch_all(&mut **tx)
            .await?;
    if rows.len() >= 16384 {
        return Err(ApiError(
            StatusCode::INSUFFICIENT_STORAGE,
            "CORDS_PROFILE_HISTORY_QUOTA",
        ));
    }
    let mut graph = ProfileGraph::default();
    for row in rows {
        graph
            .records
            .insert(row.try_get("id")?, parse(row.try_get("record")?)?);
    }
    Ok(graph)
}

async fn prepare_image(
    s: &Service,
    request: &ProfilePublish,
    permit: tokio::sync::OwnedSemaphorePermit,
) -> Result<(Option<Vec<u8>>, tokio::sync::OwnedSemaphorePermit)> {
    let Some(image) = &request.record.revision.value.profile.avatar else {
        if request.image.is_some() {
            return Err(bad());
        }
        return Ok((None, permit));
    };
    let supplied = if let Some(encoded) = request.image.as_deref() {
        if encoded.len() > 6_666_668 {
            return Err(bad());
        }
        let bytes = STANDARD.decode(encoded).map_err(|_| bad())?;
        if bytes.len() != image.bytes as usize
            || STANDARD.encode(&bytes) != encoded
            || hash(&bytes) != image.sha256
        {
            return Err(bad());
        }
        Some(bytes)
    } else {
        None
    };
    let existing = sqlx::query(
        "SELECT width,height,octet_length(encoded) AS size FROM profile_images WHERE sha256=$1",
    )
    .bind(&image.sha256)
    .fetch_optional(s.0.store.pool())
    .await?;
    if let Some(existing) = existing {
        if existing.try_get::<i32, _>("width")? != i32::from(image.width)
            || existing.try_get::<i32, _>("height")? != i32::from(image.height)
            || existing.try_get::<i32, _>("size")?
                != i32::try_from(image.bytes).map_err(|_| bad())?
        {
            return Err(bad());
        }
        // Repeated uploads still authenticate their signed record. No digest cache
        // bypasses identity checks, and no extra unreferenced blob is stored.
        return Ok((None, permit));
    }
    let bytes = supplied.ok_or(ApiError(
        StatusCode::CONFLICT,
        "CORDS_PROFILE_IMAGE_REQUIRED",
    ))?;
    let (bytes, result, permit) = tokio::task::spawn_blocking(move || {
        let result = cords_avatar_codec::validate(&bytes);
        (bytes, result, permit) // Cancellation keeps the permit until validation ends.
    })
    .await
    .map_err(|_| {
        ApiError(
            StatusCode::SERVICE_UNAVAILABLE,
            "CORDS_PROFILE_CODEC_UNAVAILABLE",
        )
    })?;
    let info =
        result.map_err(|_| ApiError(StatusCode::BAD_REQUEST, "CORDS_PROFILE_IMAGE_INVALID"))?;
    if info.width != u32::from(image.width) || info.height != u32::from(image.height) {
        return Err(bad());
    }
    Ok((Some(bytes), permit))
}
