use super::*;
use anyhow::{Context as _, Result};
use cords_crypto::conversation::ConversationCrypto;
use cords_identity::{Identity, sign};
use cords_protocol::messaging::{MlsBinding, canonical, encode, hash};
use ed25519_dalek::SigningKey;
use rand_core::OsRng;
use std::{path::Path, str::FromStr as _};

const NOW: u64 = 1_800_000_000;
const NEXT: &str = "202610150001_enforced_rbac.sql";

async fn fixture() -> Result<(PostgresStore, tempfile::TempDir, SigningKey, String)> {
    let url = std::env::var("CORDS_TEST_DATABASE_URL")
        .context("set CORDS_TEST_DATABASE_URL to an isolated PostgreSQL service")?;
    let admin = sqlx::PgPool::connect(&url).await?;
    let schema = format!("cords_rbac_migration_{}", uuid::Uuid::now_v7().simple());
    sqlx::query(&format!("CREATE SCHEMA {schema}"))
        .execute(&admin)
        .await?;
    admin.close().await;
    let options = sqlx::postgres::PgConnectOptions::from_str(&url)?
        .options([("search_path", schema.as_str())]);
    let store = PostgresStore::from_pool(
        sqlx::postgres::PgPoolOptions::new()
            .max_connections(3)
            .connect_with(options)
            .await?,
    );
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../migrations/postgres");
    let staged = tempfile::tempdir()?;
    for file in std::fs::read_dir(source)? {
        let file = file?;
        std::fs::copy(file.path(), staged.path().join(file.file_name()))?;
    }
    store.migrate(staged.path()).await?;
    store.require_current().await?;
    let key = SigningKey::generate(&mut OsRng);
    let server = cords_protocol::server_id_from_public_key(&key.verifying_key().to_bytes());
    store.bind_server_identity(&server).await?;
    Ok((store, staged, key, server))
}

fn contact() -> Result<(Identity, Contact)> {
    let identity = Identity::generate(NOW)?;
    let crypto = ConversationCrypto::new(identity.authorization.value.device_id.clone())?;
    let contact = identity.contact(&crypto.public_key())?;
    Ok((identity, contact))
}

fn sibling(identity: &Identity, previous: &Signed<DeviceAuthorization>) -> Result<Contact> {
    let key = SigningKey::generate(&mut OsRng);
    let mut authorization = previous.value.clone();
    authorization.device_id = uuid::Uuid::now_v7().to_string();
    authorization.device_public_key = encode(key.verifying_key().to_bytes());
    authorization.generation += 1;
    authorization.previous_record_hash = Some(hash(canonical(previous)?));
    let authorization = identity.sign_root(authorization)?;
    let crypto = ConversationCrypto::new(authorization.value.device_id.clone())?;
    Ok(Contact {
        mls_binding: sign(
            &key,
            MlsBinding {
                version: 1,
                account_id: authorization.value.account_id.clone(),
                device_id: authorization.value.device_id.clone(),
                authorization_hash: hash(canonical(&authorization)?),
                mls_public_key: encode(crypto.public_key()),
            },
        )?,
        authorization,
        user_card: None,
    })
}

async fn install(
    store: &PostgresStore,
    contact: &Contact,
    key: &SigningKey,
    server: &str,
) -> Result<()> {
    let authorization = &contact.authorization.value;
    let encoded = serde_json::to_string(&contact.authorization)?;
    sqlx::query("INSERT INTO account_heads VALUES($1,$2) ON CONFLICT(account_id) DO UPDATE SET authorization_record=EXCLUDED.authorization_record")
        .bind(&authorization.account_id).bind(encoded).execute(store.pool()).await?;
    sqlx::query("INSERT INTO device_contacts VALUES($1,$2,$3)")
        .bind(&authorization.device_id)
        .bind(&authorization.account_id)
        .bind(serde_json::to_string(contact)?)
        .execute(store.pool())
        .await?;
    let membership = sign(
        key,
        Membership {
            version: 1,
            server_id: server.into(),
            member_id: uuid::Uuid::now_v7().to_string(),
            account_id: authorization.account_id.clone(),
            device_id: authorization.device_id.clone(),
            generation: 1,
            issued_at: NOW,
            capabilities: vec!["channel.read".into()],
            status: "active".into(),
        },
    )?;
    sqlx::query("INSERT INTO memberships(device_id,credential) VALUES($1,$2)")
        .bind(&authorization.device_id)
        .bind(serde_json::to_string(&membership)?)
        .execute(store.pool())
        .await?;
    Ok(())
}

fn stage(staged: &Path) -> Result<()> {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../migrations/rbac")
        .join(NEXT);
    std::fs::copy(source, staged.join(NEXT))?;
    Ok(())
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL; cryptographically valid migration fixtures"]
async fn rbac_migration_preserves_device_exclusions_and_archives_authority() -> Result<()> {
    let (store, staged, key, server) = fixture().await?;
    let (identity, admitted) = contact()?;
    install(&store, &admitted, &key, &server).await?;
    let excluded = sibling(&identity, &identity.authorization)?;
    install(&store, &excluded, &key, &server).await?;
    sqlx::query("INSERT INTO server_ownership VALUES(TRUE,$1,$2,$3)")
        .bind(&admitted.authorization.value.account_id)
        .bind(&admitted.authorization.value.device_id)
        .bind(i64::try_from(NOW)?)
        .execute(store.pool())
        .await?;
    sqlx::query("INSERT INTO channels(id,name,creator,epoch) VALUES('original','restricted',$1,17),('empty','empty',$1,0)")
        .bind(&admitted.authorization.value.device_id).execute(store.pool()).await?;
    sqlx::query("INSERT INTO channel_members(channel_id,device_id,joined_sequence,active,delivery_active) VALUES('original',$1,42,TRUE,TRUE),('original',$2,9,TRUE,FALSE)")
        .bind(&admitted.authorization.value.device_id).bind(&excluded.authorization.value.device_id)
        .execute(store.pool()).await?;
    let (revoked_identity, revoked) = contact()?;
    install(&store, &revoked, &key, &server).await?;
    sqlx::query("INSERT INTO channel_members(channel_id,device_id) VALUES('original',$1)")
        .bind(&revoked.authorization.value.device_id)
        .execute(store.pool())
        .await?;
    sqlx::query("INSERT INTO device_revocations VALUES($1,$2,2,$3)")
        .bind(&revoked.authorization.value.device_id)
        .bind(&revoked.authorization.value.account_id)
        .bind(serde_json::to_string(&revoked_identity.revoke(NOW)?)?)
        .execute(store.pool())
        .await?;
    let (_, outsider) = contact()?;
    install(&store, &outsider, &key, &server).await?;
    sqlx::query("INSERT INTO account_roles VALUES($1,'builtin:admin')")
        .bind(&outsider.authorization.value.account_id)
        .execute(store.pool())
        .await?;
    let before: Vec<(String, bool, bool, i64)> = sqlx::query_as(
        "SELECT device_id,active,delivery_active,joined_sequence FROM channel_members ORDER BY device_id",
    ).fetch_all(store.pool()).await?;
    stage(staged.path())?;
    store.migrate(staged.path()).await?;
    // The currently shipped legacy service cannot open the candidate schema.
    assert!(store.require_current().await.is_err());
    let public = encode(key.verifying_key().to_bytes());
    prepare(&store, &server, &public, NOW).await?;
    prepare(&store, &server, &public, NOW + 1).await?;
    store.migrate(staged.path()).await?;
    let access: Vec<(String, String)> = sqlx::query_as(
        "SELECT channel_id,account_id FROM channel_account_access ORDER BY channel_id,account_id",
    )
    .fetch_all(store.pool())
    .await?;
    assert_eq!(
        access,
        vec![(
            "original".into(),
            admitted.authorization.value.account_id.clone()
        )]
    );
    let excluded_present: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM channel_device_exclusions WHERE channel_id='original' AND device_id=$1)")
        .bind(&excluded.authorization.value.device_id).fetch_one(store.pool()).await?;
    assert!(excluded_present);
    let revoked_excluded: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM channel_device_exclusions WHERE channel_id='original' AND device_id=$1)")
        .bind(&revoked.authorization.value.device_id).fetch_one(store.pool()).await?;
    assert!(revoked_excluded);
    let after: Vec<(String, bool, bool, i64)> = sqlx::query_as(
        "SELECT device_id,active,delivery_active,joined_sequence FROM channel_members ORDER BY device_id",
    ).fetch_all(store.pool()).await?;
    assert_eq!(
        after, before,
        "account grants must never reactivate a device or move history boundaries"
    );
    assert_archived(&store, &outsider.authorization.value.account_id).await?;
    let owner: String = sqlx::query_scalar("SELECT account_id FROM server_ownership")
        .fetch_one(store.pool())
        .await?;
    assert_eq!(owner, admitted.authorization.value.account_id);
    let epoch: i64 = sqlx::query_scalar("SELECT epoch FROM channels WHERE id='original'")
        .fetch_one(store.pool())
        .await?;
    assert_eq!(epoch, 17);
    let checked: i64 = sqlx::query_scalar("SELECT checked_at FROM rbac_migration")
        .fetch_one(store.pool())
        .await?;
    assert_eq!(checked, i64::try_from(NOW)?);
    let wrong = SigningKey::generate(&mut OsRng);
    assert!(matches!(
        prepare(
            &store,
            &server,
            &encode(wrong.verifying_key().to_bytes()),
            NOW
        )
        .await,
        Err(Error::Identity)
    ));
    store.pool().close().await;
    Ok(())
}

async fn assert_archived(store: &PostgresStore, account: &str) -> Result<()> {
    let assigned: i64 = sqlx::query_scalar("SELECT count(*) FROM account_roles")
        .fetch_one(store.pool())
        .await?;
    assert_eq!(assigned, 0);
    let archived: (String, String) =
        sqlx::query_as("SELECT account_id,role_id FROM archived_role_assignments")
            .fetch_one(store.pool())
            .await?;
    assert_eq!(archived, (account.into(), "builtin:admin".into()));
    Ok(())
}

async fn assert_admission(store: &PostgresStore, contact: &Contact, approved: bool) -> Result<()> {
    let actual: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM server_device_admission WHERE device_id=$1 AND approved)",
    )
    .bind(&contact.authorization.value.device_id)
    .fetch_one(store.pool())
    .await?;
    assert_eq!(
        actual, approved,
        "device approval is independent of account access"
    );
    Ok(())
}

async fn decision(
    store: &PostgresStore,
    key: &SigningKey,
    server: &str,
    contact: &Contact,
    channel: Option<&str>,
) -> Result<super::super::runtime::Decision, super::super::runtime::Error> {
    let mut tx = store.pool().begin().await?;
    let result = super::super::runtime::load(
        &mut tx,
        &contact.authorization.value.device_id,
        server,
        &encode(key.verifying_key().to_bytes()),
        NOW,
        channel,
    )
    .await;
    tx.rollback().await?;
    result
}

async fn enable(store: &PostgresStore, key: &SigningKey, server: &str, path: &Path) -> Result<()> {
    stage(path)?;
    store.migrate(path).await?;
    prepare(store, server, &encode(key.verifying_key().to_bytes()), NOW).await?;
    // Test-only activation: production remains fenced until endpoint integration.
    sqlx::query("UPDATE role_policy SET active=TRUE WHERE singleton=TRUE")
        .execute(store.pool())
        .await?;
    Ok(())
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn runtime_requires_native_completion_and_active_policy() -> Result<()> {
    use super::super::runtime::Error;
    let (store, path, key, server) = fixture().await?;
    let (_, contact) = contact()?;
    install(&store, &contact, &key, &server).await?;
    stage(path.path())?;
    store.migrate(path.path()).await?;
    assert!(matches!(
        decision(&store, &key, &server, &contact, None).await,
        Err(Error::NotReady)
    ));
    sqlx::query("UPDATE role_policy SET active=TRUE WHERE singleton=TRUE")
        .execute(store.pool())
        .await?;
    assert!(matches!(
        decision(&store, &key, &server, &contact, None).await,
        Err(Error::NotReady)
    ));
    prepare(
        &store,
        &server,
        &encode(key.verifying_key().to_bytes()),
        NOW,
    )
    .await?;
    let current = decision(&store, &key, &server, &contact, None).await?;
    assert!(current.subject.accepted && current.subject.device_authorized);
    assert!(
        current
            .permissions
            .contains(&cords_protocol::roles::Permission::ViewServer)
    );
    assert!(
        !current
            .permissions
            .contains(&cords_protocol::roles::Permission::ReadMessages)
    );
    store.pool().close().await;
    Ok(())
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn runtime_account_grant_never_reactivates_sibling_devices_or_owner_mls() -> Result<()> {
    use cords_protocol::roles::Permission::{CommitMls, ManageChannel, ReadMessages, SendMessages};
    let (store, path, key, server) = fixture().await?;
    let (identity, admitted) = contact()?;
    let excluded = sibling(&identity, &admitted.authorization)?;
    let outsider = sibling(&identity, &excluded.authorization)?;
    for contact in [&admitted, &excluded, &outsider] {
        install(&store, contact, &key, &server).await?;
    }
    sqlx::query(
        "INSERT INTO channels(id,name,creator,epoch) VALUES('original','restricted',$1,17)",
    )
    .bind(&admitted.authorization.value.device_id)
    .execute(store.pool())
    .await?;
    for (contact, active) in [(&admitted, true), (&excluded, false)] {
        sqlx::query("INSERT INTO channel_members(channel_id,device_id,active,delivery_active,joined_sequence) VALUES('original',$1,$2,$2,42)")
            .bind(&contact.authorization.value.device_id).bind(active).execute(store.pool()).await?;
    }
    enable(&store, &key, &server, path.path()).await?;
    // The owner has the same account as all three devices. Ownership must not
    // grant group access to either excluded or not-yet-enrolled devices.
    sqlx::query("INSERT INTO server_ownership VALUES(TRUE,$1,$2,$3)")
        .bind(&identity.authorization.value.account_id)
        .bind(&admitted.authorization.value.device_id)
        .bind(i64::try_from(NOW)?)
        .execute(store.pool())
        .await?;
    let allowed = decision(&store, &key, &server, &admitted, Some("original")).await?;
    assert!(allowed.account_eligible && allowed.device_participating);
    allowed.require(ReadMessages)?;
    for contact in [&excluded, &outsider] {
        let denied = decision(&store, &key, &server, contact, Some("original")).await?;
        assert!(denied.account_eligible && !denied.device_participating);
        denied.require(ManageChannel)?;
        for permission in [ReadMessages, SendMessages, CommitMls] {
            assert!(denied.require(permission).is_err());
        }
    }
    // Even stale true roster and approval flags cannot bypass a root revocation.
    sqlx::query("INSERT INTO device_revocations VALUES($1,$2,4,'accepted root revocation')")
        .bind(&admitted.authorization.value.device_id)
        .bind(&identity.authorization.value.account_id)
        .execute(store.pool())
        .await?;
    let revoked = decision(&store, &key, &server, &admitted, Some("original")).await?;
    assert!(revoked.permissions.is_empty() && !revoked.account_eligible);
    assert!(revoked.subject.revoked_or_burned);
    store.pool().close().await;
    Ok(())
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn runtime_reloads_roles_policy_category_boundary_and_device_approval() -> Result<()> {
    use cords_protocol::roles::{ADMIN_ROLE, DEFAULT_ROLE, Override, Overrides, Permission};
    let (store, path, key, server) = fixture().await?;
    let (_, contact) = contact()?;
    install(&store, &contact, &key, &server).await?;
    sqlx::query("INSERT INTO channels(id,name,creator,confidentiality_mode) VALUES('public','public',$1,'public')")
        .bind(&contact.authorization.value.device_id).execute(store.pool()).await?;
    enable(&store, &key, &server, path.path()).await?;
    let initial = decision(&store, &key, &server, &contact, Some("public")).await?;
    initial.require(Permission::SendMessages)?;
    assert!(initial.require(Permission::ManageRoles).is_err());
    sqlx::query("INSERT INTO account_roles VALUES($1,$2)")
        .bind(&contact.authorization.value.account_id)
        .bind(ADMIN_ROLE)
        .execute(store.pool())
        .await?;
    decision(&store, &key, &server, &contact, None)
        .await?
        .require(Permission::ManageRoles)?;
    sqlx::query("DELETE FROM account_roles WHERE account_id=$1")
        .bind(&contact.authorization.value.account_id)
        .execute(store.pool())
        .await?;
    assert!(
        decision(&store, &key, &server, &contact, None)
            .await?
            .require(Permission::ManageRoles)
            .is_err()
    );
    let denied: Overrides = [(
        DEFAULT_ROLE.into(),
        Override {
            allow: std::collections::BTreeSet::new(),
            deny: [Permission::ViewChannel].into(),
        },
    )]
    .into();
    let allowed: Overrides = [(
        DEFAULT_ROLE.into(),
        Override {
            allow: [Permission::ViewChannel].into(),
            deny: std::collections::BTreeSet::new(),
        },
    )]
    .into();
    sqlx::query("INSERT INTO channel_categories VALUES('private','private',$1)")
        .bind(serde_json::to_string(&denied)?)
        .execute(store.pool())
        .await?;
    sqlx::query("UPDATE channel_access_policy SET category_id='private',overrides=$1 WHERE channel_id='public'")
        .bind(serde_json::to_string(&allowed)?).execute(store.pool()).await?;
    let hidden = decision(&store, &key, &server, &contact, Some("public")).await?;
    assert!(!hidden.account_eligible && !hidden.device_participating);
    assert!(hidden.require(Permission::ReadMessages).is_err());
    sqlx::query("UPDATE role_policy SET device_admission='administrator_approval'")
        .execute(store.pool())
        .await?;
    sqlx::query("UPDATE server_device_admission SET approved=FALSE")
        .execute(store.pool())
        .await?;
    let unapproved = decision(&store, &key, &server, &contact, None).await?;
    assert!(unapproved.subject.accepted && unapproved.subject.device_authorized);
    assert!(unapproved.permissions.is_empty());
    store.pool().close().await;
    Ok(())
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn runtime_serializes_revocation_until_the_authorized_transaction_finishes() -> Result<()> {
    use cords_protocol::roles::Permission;
    let (store, path, key, server) = fixture().await?;
    let (_, contact) = contact()?;
    install(&store, &contact, &key, &server).await?;
    enable(&store, &key, &server, path.path()).await?;
    let account = contact.authorization.value.account_id.clone();
    let device = contact.authorization.value.device_id.clone();
    let mut reader = store.pool().begin().await?;
    let before = super::super::runtime::load(
        &mut reader,
        &device,
        &server,
        &encode(key.verifying_key().to_bytes()),
        NOW,
        None,
    )
    .await?;
    before.require(Permission::ViewServer)?;
    let pool = store.pool().clone();
    let (starting, started) = tokio::sync::oneshot::channel();
    let mut writer = tokio::spawn(async move {
        let mut tx = pool.begin().await?;
        let _ = starting.send(());
        // This is the existing exclusive identity-transition lock, exercised
        // against a second real PostgreSQL connection.
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
            .bind(&account)
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO device_revocations VALUES($1,$2,2,'durably accepted revocation')")
            .bind(device)
            .bind(account)
            .execute(&mut *tx)
            .await?;
        tx.commit().await
    });
    started.await?;
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(100), &mut writer)
            .await
            .is_err(),
        "revocation must wait for the already-authorized transaction"
    );
    reader.commit().await?;
    tokio::time::timeout(std::time::Duration::from_secs(5), writer).await???;
    let after = decision(&store, &key, &server, &contact, None).await?;
    assert!(after.subject.revoked_or_burned && after.permissions.is_empty());
    assert!(after.require(Permission::ViewServer).is_err());
    store.pool().close().await;
    Ok(())
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn rbac_migration_shared_account_never_restores_revoked_or_excluded_leaves() -> Result<()> {
    let (store, staged, key, server) = fixture().await?;
    let (identity, admitted) = contact()?;
    let excluded = sibling(&identity, &identity.authorization)?;
    let revoked = sibling(&identity, &excluded.authorization)?;
    for contact in [&admitted, &excluded, &revoked] {
        install(&store, contact, &key, &server).await?;
    }
    sqlx::query("INSERT INTO channels(id,name,creator,epoch) VALUES('shared','restricted',$1,3)")
        .bind(&admitted.authorization.value.device_id)
        .execute(store.pool())
        .await?;
    for (contact, active, delivery) in [
        (&admitted, true, true),
        (&excluded, false, false),
        (&revoked, true, true),
    ] {
        sqlx::query("INSERT INTO channel_members(channel_id,device_id,active,delivery_active,joined_sequence) VALUES('shared',$1,$2,$3,71)")
            .bind(&contact.authorization.value.device_id).bind(active).bind(delivery).execute(store.pool()).await?;
    }
    // The actual revocation signature is checked by its original acceptance path;
    // migration must retain its exclusion even when legacy roster flags lag behind.
    let revocation = identity.sign_root(cords_protocol::messaging::DeviceRevocation {
        version: 1,
        account_id: identity.authorization.value.account_id.clone(),
        revoked_device_id: revoked.authorization.value.device_id.clone(),
        generation: 4,
        issued_at: NOW,
        previous_state_hash: hash(canonical(&revoked.authorization)?),
    })?;
    sqlx::query("INSERT INTO device_revocations VALUES($1,$2,4,$3)")
        .bind(&revoked.authorization.value.device_id)
        .bind(&revoked.authorization.value.account_id)
        .bind(serde_json::to_string(&revocation)?)
        .execute(store.pool())
        .await?;
    sqlx::query("INSERT INTO pending_policy_removals VALUES('shared',$1)")
        .bind(&revoked.authorization.value.device_id)
        .execute(store.pool())
        .await?;
    stage(staged.path())?;
    store.migrate(staged.path()).await?;
    prepare(
        &store,
        &server,
        &encode(key.verifying_key().to_bytes()),
        NOW,
    )
    .await?;
    let account_access: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM channel_account_access WHERE channel_id='shared' AND account_id=$1)")
        .bind(&identity.authorization.value.account_id).fetch_one(store.pool()).await?;
    assert!(account_access);
    assert_admission(&store, &admitted, true).await?;
    // Server approval of the excluded device preserves admission only: the
    // channel exclusion below still prevents any restored participation.
    assert_admission(&store, &excluded, true).await?;
    assert_admission(&store, &revoked, false).await?;
    for contact in [&excluded, &revoked] {
        let denied: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM channel_device_exclusions WHERE channel_id='shared' AND device_id=$1)")
            .bind(&contact.authorization.value.device_id).fetch_one(store.pool()).await?;
        assert!(
            denied,
            "account authorization must not clear a device exclusion"
        );
    }
    let pending: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pending_policy_removals WHERE channel_id='shared'",
    )
    .fetch_one(store.pool())
    .await?;
    assert_eq!(pending, 1);
    let excluded_flags: (bool,bool,i64) = sqlx::query_as("SELECT active,delivery_active,joined_sequence FROM channel_members WHERE channel_id='shared' AND device_id=$1")
        .bind(&excluded.authorization.value.device_id).fetch_one(store.pool()).await?;
    assert_eq!(excluded_flags, (false, false, 71));
    store.pool().close().await;
    Ok(())
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn rbac_migration_failure_rolls_back_and_fresh_install_is_reentrant() -> Result<()> {
    let (store, staged, key, server) = fixture().await?;
    stage(staged.path())?;
    let migration = std::fs::read_to_string(staged.path().join(NEXT))?;
    std::fs::write(
        staged.path().join(NEXT),
        format!("{migration}\nSELECT * FROM missing_rbac_failure;"),
    )?;
    assert!(store.migrate(staged.path()).await.is_err());
    store.require_current().await?;
    let absent: Option<String> =
        sqlx::query_scalar("SELECT to_regclass('archived_role_assignments')::text")
            .fetch_one(store.pool())
            .await?;
    assert!(absent.is_none());
    std::fs::write(staged.path().join(NEXT), migration)?;
    for _ in 0..2 {
        store.migrate(staged.path()).await?;
        prepare(
            &store,
            &server,
            &encode(key.verifying_key().to_bytes()),
            NOW,
        )
        .await?;
    }
    let access: i64 = sqlx::query_scalar("SELECT count(*) FROM channel_account_access")
        .fetch_one(store.pool())
        .await?;
    assert_eq!(access, 0);
    let roles: i64 = sqlx::query_scalar("SELECT count(*) FROM server_roles")
        .fetch_one(store.pool())
        .await?;
    assert_eq!(roles, 3);
    store.pool().close().await;
    Ok(())
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn rbac_migration_rejects_expired_forged_burned_inactive_and_malformed_evidence() -> Result<()>
{
    for condition in ["expired", "forged", "burned", "inactive", "malformed"] {
        let (store, staged, key, server) = fixture().await?;
        let (identity, mut admitted) = contact()?;
        if condition == "expired" {
            let mut authorization = identity.authorization.value.clone();
            authorization.created_at = NOW - 100;
            authorization.expires_at = Some(NOW - 1);
            admitted.authorization = identity.sign_root(authorization)?;
            let mut binding = admitted.mls_binding.value.clone();
            binding.authorization_hash = hash(canonical(&admitted.authorization)?);
            admitted.mls_binding = identity.sign_device(binding)?;
        }
        install(&store, &admitted, &key, &server).await?;
        let device = &admitted.authorization.value.device_id;
        sqlx::query("INSERT INTO channels(id,name,creator) VALUES('original','restricted',$1)")
            .bind(device)
            .execute(store.pool())
            .await?;
        sqlx::query("INSERT INTO channel_members(channel_id,device_id) VALUES('original',$1)")
            .bind(device)
            .execute(store.pool())
            .await?;
        match condition {
            "forged" => {
                let raw: String =
                    sqlx::query_scalar("SELECT credential FROM memberships WHERE device_id=$1")
                        .bind(device)
                        .fetch_one(store.pool())
                        .await?;
                let mut membership: Signed<Membership> = serde_json::from_str(&raw)?;
                membership.value.member_id = "substituted".into();
                sqlx::query("UPDATE memberships SET credential=$1 WHERE device_id=$2")
                    .bind(serde_json::to_string(&membership)?)
                    .bind(device)
                    .execute(store.pool())
                    .await?;
            }
            "burned" => {
                sqlx::query("INSERT INTO identity_burns VALUES($1,'retained burn',1)")
                    .bind(&admitted.authorization.value.account_id)
                    .execute(store.pool())
                    .await?;
            }
            "inactive" => {
                sqlx::query("UPDATE memberships SET active=FALSE WHERE device_id=$1")
                    .bind(device)
                    .execute(store.pool())
                    .await?;
            }
            "malformed" => {
                sqlx::query("UPDATE device_contacts SET contact='malformed' WHERE device_id=$1")
                    .bind(device)
                    .execute(store.pool())
                    .await?;
            }
            _ => {}
        }
        stage(staged.path())?;
        store.migrate(staged.path()).await?;
        let result = prepare(
            &store,
            &server,
            &encode(key.verifying_key().to_bytes()),
            NOW,
        )
        .await;
        if condition == "malformed" {
            assert!(matches!(result, Err(Error::Encoding(_))));
            let completed: bool = sqlx::query_scalar("SELECT completed FROM rbac_migration")
                .fetch_one(store.pool())
                .await?;
            assert!(
                !completed,
                "native validation failure must remain safely retryable"
            );
        } else {
            result?;
        }
        let grants: i64 = sqlx::query_scalar("SELECT count(*) FROM channel_account_access")
            .fetch_one(store.pool())
            .await?;
        assert_eq!(
            grants, 0,
            "{condition} evidence must not grant channel access"
        );
        store.pool().close().await;
    }
    Ok(())
}
