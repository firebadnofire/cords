//! Paginated admitted accounts, independent of device contact discovery.
use super::*;
use cords_protocol::directory::{AccountDirectoryPage, DirectoryAccount};

pub(super) async fn accounts(
    State(s): State<Service>,
    headers: HeaderMap,
    Query(page): Query<MembershipPage>,
) -> Result<Json<AccountDirectoryPage>> {
    if page.after.len() > 128 {
        return Err(bad());
    }
    let member = s.authenticate(&headers, "channel.read").await?;
    let mut tx = s.0.store.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")
        .execute(&mut *tx)
        .await?;
    // Recheck session, revocation and current policy inside the read transaction.
    s.authorize_session(&mut tx, &headers, &member).await?;
    s.require_current_policy(
        &mut tx,
        &member.value,
        cords_protocol::roles::Permission::ViewServer,
        None,
    )
    .await?;
    let ids: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT d.account_id COLLATE \"C\" AS account_id FROM device_contacts d JOIN memberships m ON m.device_id=d.device_id \
         WHERE m.active AND d.account_id COLLATE \"C\">$1 COLLATE \"C\" \
         AND NOT EXISTS(SELECT 1 FROM device_revocations r WHERE r.device_id=d.device_id) \
         AND NOT EXISTS(SELECT 1 FROM identity_burns b WHERE b.account_id=d.account_id) \
         ORDER BY account_id LIMIT 17",
    ).bind(&page.after).fetch_all(&mut *tx).await?;
    let mut accounts = Vec::new();
    let mut remaining_bytes = 4 * 1024 * 1024_i64;
    for account_id in ids.iter().take(16) {
        let bytes: i64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(octet_length(d.contact)),0)::BIGINT FROM device_contacts d JOIN memberships m ON m.device_id=d.device_id \
             WHERE d.account_id=$1 AND m.active AND NOT EXISTS(SELECT 1 FROM device_revocations r WHERE r.device_id=d.device_id)",
        ).bind(account_id).fetch_one(&mut *tx).await?;
        remaining_bytes = remaining_bytes
            .checked_sub(bytes)
            .filter(|left| *left >= 0)
            .ok_or(ApiError(
                StatusCode::PAYLOAD_TOO_LARGE,
                "CORDS_DIRECTORY_RESPONSE_LIMIT",
            ))?;
        let records: Vec<String> = sqlx::query_scalar(
            "SELECT d.contact FROM device_contacts d JOIN memberships m ON m.device_id=d.device_id \
             WHERE d.account_id=$1 AND m.active \
             AND NOT EXISTS(SELECT 1 FROM device_revocations r WHERE r.device_id=d.device_id) \
             ORDER BY d.device_id LIMIT 65",
        )
        .bind(account_id)
        .fetch_all(&mut *tx)
        .await?;
        if records.len() > 64 {
            return Err(ApiError(
                StatusCode::PAYLOAD_TOO_LARGE,
                "CORDS_DIRECTORY_DEVICE_LIMIT",
            ));
        }
        let devices = records
            .iter()
            .map(|record| parse(record))
            .collect::<Result<Vec<_>>>()?;
        accounts.push(DirectoryAccount {
            account_id: account_id.clone(),
            devices,
        });
    }
    let next_after = (ids.len() > 16).then(|| ids[15].clone());
    let result = AccountDirectoryPage {
        version: 1,
        server_id: s.0.identity.server_id(),
        accounts,
        next_after,
    };
    tx.commit().await?;
    Ok(Json(result))
}
