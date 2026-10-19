//! Revision-checked presentation with current-policy authorization.
use super::*;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use cords_protocol::server_profile::{ServerIconChange, ServerProfile, ServerProfileChange};

async fn load(s: &Service, tx: &mut Tx<'_>) -> Result<ServerProfile> {
    let row = sqlx::query(
        "SELECT revision,name,description,background,icon FROM server_profile WHERE singleton=TRUE",
    )
    .fetch_one(&mut **tx)
    .await?;
    Ok(ServerProfile {
        version: 1,
        server_id: s.0.identity.server_id(),
        revision: uint(row.try_get("revision")?)?,
        name: row.try_get("name")?,
        description: row.try_get("description")?,
        background: row.try_get("background")?,
        icon: row
            .try_get::<Option<String>, _>("icon")?
            .map(|value| parse(&value))
            .transpose()?,
    })
}

pub(super) async fn read(
    State(s): State<Service>,
    headers: HeaderMap,
) -> Result<Json<Signed<ServerProfile>>> {
    let member = s.authenticate(&headers, "channel.read").await?;
    let mut tx = s.0.store.pool().begin().await?;
    s.authorize_session(&mut tx, &headers, &member).await?;
    s.require_current_policy(
        &mut tx,
        &member.value,
        cords_protocol::roles::Permission::ViewServer,
        None,
    )
    .await?;
    let response = s.sign(load(&s, &mut tx).await?)?;
    tx.commit().await?;
    Ok(Json(response))
}

pub(super) async fn update(
    State(s): State<Service>,
    headers: HeaderMap,
    Json(request): Json<ServerProfileChange>,
) -> Result<Json<Signed<ServerProfile>>> {
    if !request.valid() {
        return Err(bad());
    }
    let member = s.authenticate(&headers, "channel.read").await?;
    let mut tx = s.0.store.pool().begin().await?;
    s.authorize_mutation(&mut tx, &headers, &member, None)
        .await?;
    roles::administrative_subject(
        &s,
        &mut tx,
        &member.value,
        &[cords_protocol::roles::Permission::ManageServer],
    )
    .await?;
    let scope = format!("server-profile:{}", member.value.device_id);
    if let Some(response) =
        previous::<_, Signed<ServerProfile>>(&mut tx, &scope, &request.request_id, &request).await?
    {
        tx.commit().await?;
        return Ok(Json(response));
    }
    sqlx::query("SELECT singleton FROM server_profile WHERE singleton=TRUE FOR UPDATE")
        .fetch_one(&mut *tx)
        .await?;
    let current = load(&s, &mut tx).await?;
    if current.revision != request.expected_revision {
        return Err(ApiError(
            StatusCode::CONFLICT,
            "CORDS_SERVER_PROFILE_CONFLICT",
        ));
    }
    let revision = current.revision.checked_add(1).ok_or_else(bad)?;
    sqlx::query(
        "UPDATE server_profile SET revision=$1,name=$2,description=$3,background=$4 WHERE singleton=TRUE",
    )
    .bind(int(revision)?)
    .bind(&request.name)
    .bind(&request.description)
    .bind(&request.background)
    .execute(&mut *tx)
    .await?;
    sqlx::query("INSERT INTO server_profile_audit(revision,actor_device_id,request_id,committed_at) VALUES($1,$2,$3,$4)")
        .bind(int(revision)?).bind(&member.value.device_id).bind(&request.request_id).bind(int(now()?)?).execute(&mut *tx).await?;
    let response = s.sign(load(&s, &mut tx).await?)?;
    remember(&mut tx, &scope, &request.request_id, &request, &response).await?;
    tx.commit().await?;
    Ok(Json(response))
}

pub(super) async fn update_icon(
    State(s): State<Service>,
    request: axum::extract::Request,
) -> Result<Json<Signed<ServerProfile>>> {
    let headers = request.headers().clone();
    s.limit_authentication()?;
    let member = s.authenticate(&headers, "channel.read").await?;
    let mut tx = s.0.store.pool().begin().await?;
    s.authorize_mutation(&mut tx, &headers, &member, None)
        .await?;
    roles::administrative_subject(
        &s,
        &mut tx,
        &member.value,
        &[cords_protocol::roles::Permission::ManageServer],
    )
    .await?;
    tx.commit().await?;
    let permit =
        s.0.profile_validation
            .clone()
            .try_acquire_owned()
            .map_err(|_| ApiError(StatusCode::TOO_MANY_REQUESTS, "CORDS_PROFILE_BUSY"))?;
    let body = axum::body::to_bytes(request.into_body(), 8 * 1024 * 1024)
        .await
        .map_err(|_| bad())?;
    let request: ServerIconChange = serde_json::from_slice(&body)?;
    drop(body);
    if !request.valid() {
        return Err(bad());
    }
    let (image, _permit) = if let (Some(icon), Some(encoded)) = (&request.icon, &request.image) {
        let bytes = STANDARD.decode(encoded).map_err(|_| bad())?;
        if bytes.len() != icon.bytes as usize
            || hash(&bytes) != icon.sha256
            || STANDARD.encode(&bytes) != *encoded
        {
            return Err(bad());
        }
        let (bytes, info, permit) = tokio::task::spawn_blocking(move || {
            let info = cords_avatar_codec::validate(&bytes);
            (bytes, info, permit)
        })
        .await
        .map_err(|_| {
            ApiError(
                StatusCode::SERVICE_UNAVAILABLE,
                "CORDS_PROFILE_CODEC_UNAVAILABLE",
            )
        })?;
        let info =
            info.map_err(|_| ApiError(StatusCode::BAD_REQUEST, "CORDS_PROFILE_IMAGE_INVALID"))?;
        if info.width != u32::from(icon.width) || info.height != u32::from(icon.height) {
            return Err(bad());
        }
        (Some(bytes), permit)
    } else {
        (None, permit)
    };
    // The codec holds no policy/identity locks. Revalidate current authority now.
    let mut tx = s.0.store.pool().begin().await?;
    s.authorize_mutation(&mut tx, &headers, &member, None)
        .await?;
    roles::administrative_subject(
        &s,
        &mut tx,
        &member.value,
        &[cords_protocol::roles::Permission::ManageServer],
    )
    .await?;
    let scope = format!("server-icon:{}", member.value.device_id);
    if let Some(response) =
        previous::<_, Signed<ServerProfile>>(&mut tx, &scope, &request.request_id, &request).await?
    {
        tx.commit().await?;
        return Ok(Json(response));
    }
    sqlx::query("SELECT singleton FROM server_profile WHERE singleton=TRUE FOR UPDATE")
        .fetch_one(&mut *tx)
        .await?;
    let current = load(&s, &mut tx).await?;
    if current.revision != request.expected_revision {
        return Err(ApiError(
            StatusCode::CONFLICT,
            "CORDS_SERVER_PROFILE_CONFLICT",
        ));
    }
    let revision = current.revision.checked_add(1).ok_or_else(bad)?;
    let icon = request.icon.as_ref().map(json).transpose()?;
    sqlx::query("UPDATE server_profile SET revision=$1,icon=$2,icon_image=$3 WHERE singleton=TRUE")
        .bind(int(revision)?)
        .bind(icon)
        .bind(image)
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO server_profile_audit(revision,actor_device_id,request_id,committed_at) VALUES($1,$2,$3,$4)")
        .bind(int(revision)?).bind(&member.value.device_id).bind(&request.request_id).bind(int(now()?)?).execute(&mut *tx).await?;
    let response = s.sign(load(&s, &mut tx).await?)?;
    remember(&mut tx, &scope, &request.request_id, &request, &response).await?;
    tx.commit().await?;
    Ok(Json(response))
}

pub(super) async fn image(
    State(s): State<Service>,
    headers: HeaderMap,
    Path(digest): Path<String>,
) -> Result<impl IntoResponse> {
    let member = s.authenticate(&headers, "channel.read").await?;
    let mut tx = s.0.store.pool().begin().await?;
    s.authorize_session(&mut tx, &headers, &member).await?;
    s.require_current_policy(
        &mut tx,
        &member.value,
        cords_protocol::roles::Permission::ViewServer,
        None,
    )
    .await?;
    let profile = load(&s, &mut tx).await?;
    if !profile.icon.is_some_and(|icon| icon.sha256 == digest) {
        return Err(ApiError(
            StatusCode::NOT_FOUND,
            "CORDS_PROFILE_IMAGE_NOT_FOUND",
        ));
    }
    let bytes: Vec<u8> =
        sqlx::query_scalar("SELECT icon_image FROM server_profile WHERE singleton=TRUE")
            .fetch_one(&mut *tx)
            .await?;
    if hash(&bytes) != digest {
        return Err(ApiError(
            StatusCode::CONFLICT,
            "CORDS_PROFILE_IMAGE_CHANGED",
        ));
    }
    tx.commit().await?;
    Ok((
        [
            ("content-type", "image/jxl"),
            ("cache-control", "private, no-store"),
        ],
        bytes,
    ))
}
