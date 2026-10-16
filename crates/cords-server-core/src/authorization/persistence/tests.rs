use super::*;
use anyhow::{Context as _, Result};
use cords_storage::PostgresStore;
use std::{collections::BTreeSet, path::Path, str::FromStr as _};

async fn store() -> Result<PostgresStore> {
    let url = std::env::var("CORDS_TEST_DATABASE_URL")
        .context("set CORDS_TEST_DATABASE_URL to an isolated PostgreSQL service")?;
    let admin = sqlx::PgPool::connect(&url).await?;
    let schema = format!("cords_roles_{}", uuid::Uuid::now_v7().simple());
    sqlx::query(&format!("CREATE SCHEMA {schema}"))
        .execute(&admin)
        .await?;
    let options = sqlx::postgres::PgConnectOptions::from_str(&url)?
        .options([("search_path", schema.as_str())]);
    Ok(PostgresStore::from_pool(
        sqlx::postgres::PgPoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await?,
    ))
}

fn actor() -> Subject {
    Subject {
        account_id: "owner".into(),
        accepted: true,
        device_authorized: true,
        device_approved: true,
        revoked_or_burned: false,
        owner: true,
        roles: BTreeSet::new(),
    }
}
fn request(revision: i64, change: Change) -> Request {
    Request {
        expected_revision: revision,
        request_id: uuid::Uuid::now_v7().to_string(),
        change,
    }
}
async fn commit(store: &PostgresStore, request: &Request) -> Result<i64, Error> {
    let mut tx = store.pool().begin().await?;
    let revision = apply(&mut tx, &actor(), "owner-device", None, request).await?;
    tx.commit().await?;
    Ok(revision)
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL; tests repository transactions, not session authentication"]
async fn role_transactions_serialize_retries_rollback_and_assignments() -> Result<()> {
    let store = store().await?;
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../migrations/postgres");
    store.migrate(&source).await?;
    store.migrate(&source).await?;
    store.require_current().await?;
    // Opaque identity fixtures deliberately test storage only; the live-route tests
    // separately require cryptographically valid identities and session proofs.
    sqlx::query("INSERT INTO account_heads VALUES('owner','opaque'),('member','opaque')")
        .execute(store.pool())
        .await?;
    sqlx::query("INSERT INTO device_contacts VALUES('owner-device','owner','opaque')")
        .execute(store.pool())
        .await?;
    let init = request(0, Change::Initialize);
    assert_eq!(commit(&store, &init).await?, 1);
    assert_eq!(commit(&store, &init).await?, 1);
    let mut substituted = init.clone();
    substituted.change = Change::DeleteRole {
        role_id: "builtin:default".into(),
    };
    assert!(matches!(
        commit(&store, &substituted).await,
        Err(Error::Replay)
    ));

    let role = Role {
        id: uuid::Uuid::now_v7().to_string(),
        name: "Build team".into(),
        color: "#223344".into(),
        description: String::new(),
        priority: 5,
        permissions: BTreeSet::new(),
        mentionable: false,
    };
    let create = request(1, Change::PutRole { role: role.clone() });
    let mut aborted = store.pool().begin().await?;
    assert_eq!(
        apply(&mut aborted, &actor(), "owner-device", None, &create).await?,
        2
    );
    aborted.rollback().await?;
    assert_eq!(commit(&store, &create).await?, 2);
    let mut renamed_a = role.clone();
    renamed_a.name = "Team A".into();
    let mut renamed_b = role.clone();
    renamed_b.name = "Team B".into();
    let a = request(2, Change::PutRole { role: renamed_a });
    let b = request(2, Change::PutRole { role: renamed_b });
    let (a, b) = tokio::join!(commit(&store, &a), commit(&store, &b));
    assert!(matches!(
        (&a, &b),
        (Ok(3), Err(Error::Conflict)) | (Err(Error::Conflict), Ok(3))
    ));

    let mut member = actor();
    member.account_id = "member".into();
    member.owner = false;
    let assign = request(
        3,
        Change::AssignRole {
            account_id: "member".into(),
            role_id: role.id.clone(),
        },
    );
    let mut tx = store.pool().begin().await?;
    assert_eq!(
        apply(&mut tx, &actor(), "owner-device", Some(&member), &assign).await?,
        4
    );
    tx.commit().await?;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM account_roles")
        .fetch_one(store.pool())
        .await?;
    assert_eq!(count, 1);
    let remove = request(
        4,
        Change::DeleteRole {
            role_id: role.id.clone(),
        },
    );
    assert_eq!(commit(&store, &remove).await?, 5);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM account_roles")
        .fetch_one(store.pool())
        .await?;
    assert_eq!(count, 0);
    let delete_default = request(
        5,
        Change::DeleteRole {
            role_id: "builtin:default".into(),
        },
    );
    assert!(commit(&store, &delete_default).await.is_err());
    // New connection and repeated startup retain the durable head and exact audit count.
    let mut tx = store.pool().begin().await?;
    let snapshot = load(&mut tx).await?;
    assert_eq!(snapshot.revision, 5);
    assert!(!snapshot.active);
    assert_eq!(snapshot.policy.context("initialized")?.roles().len(), 3);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM role_policy_changes")
        .fetch_one(&mut *tx)
        .await?;
    assert_eq!(count, 5);
    tx.rollback().await?;
    store.migrate(&source).await?;
    Ok(())
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn additive_role_migration_preserves_legacy_and_rolls_back_failure() -> Result<()> {
    let store = store().await?;
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../migrations/postgres");
    let staged = tempfile::tempdir()?;
    let next = "202610120001_role_policy.sql";
    for file in std::fs::read_dir(&source)? {
        let file = file?;
        if file.file_name().to_string_lossy().as_ref() < next {
            std::fs::copy(file.path(), staged.path().join(file.file_name()))?;
        }
    }
    store.migrate(staged.path()).await?;
    assert!(store.require_current().await.is_err());
    sqlx::query("INSERT INTO account_heads VALUES('unchanged','original signed record')")
        .execute(store.pool())
        .await?;
    store.bind_server_identity("original-server").await?;
    let migration = std::fs::read_to_string(source.join(next))?;
    std::fs::write(
        staged.path().join(next),
        format!("{migration}\nSELECT * FROM missing_table_for_failure_test;"),
    )?;
    assert!(store.migrate(staged.path()).await.is_err());
    let version: i64 = sqlx::query_scalar("SELECT version FROM cords_schema_metadata")
        .fetch_one(store.pool())
        .await?;
    assert_eq!(version, 12);
    let policy_table: Option<String> =
        sqlx::query_scalar("SELECT to_regclass('role_policy')::text")
            .fetch_one(store.pool())
            .await?;
    assert!(policy_table.is_none());
    std::fs::write(staged.path().join(next), migration)?;
    let defaults = "202610140001_role_catalog_defaults.sql";
    std::fs::copy(source.join(defaults), staged.path().join(defaults))?;
    copy_later_migrations(&source, staged.path(), defaults)?;
    for _ in 0..2 {
        store.migrate(staged.path()).await?;
        store.require_current().await?;
    }
    let record: String = sqlx::query_scalar(
        "SELECT authorization_record FROM account_heads WHERE account_id='unchanged'",
    )
    .fetch_one(store.pool())
    .await?;
    assert_eq!(record, "original signed record");
    assert_eq!(
        store.server_identity().await?.as_deref(),
        Some("original-server")
    );
    let mut tx = store.pool().begin().await?;
    let snapshot = load(&mut tx).await?;
    assert_eq!(snapshot.revision, 0);
    assert!(!snapshot.active);
    assert_eq!(snapshot.policy.context("preinitialized")?.roles().len(), 3);
    tx.rollback().await?;
    Ok(())
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn role_defaults_migrate_atomically_and_preserve_existing_edits() -> Result<()> {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../migrations/postgres");
    let next = "202610140001_role_catalog_defaults.sql";
    for edited in [false, true] {
        let store = store().await?;
        let staged = tempfile::tempdir()?;
        for file in std::fs::read_dir(&source)? {
            let file = file?;
            if file.file_name().to_string_lossy().as_ref() < next {
                std::fs::copy(file.path(), staged.path().join(file.file_name()))?;
            }
        }
        store.migrate(staged.path()).await?;
        assert!(store.require_current().await.is_err());
        if edited {
            sqlx::query("INSERT INTO account_heads VALUES('owner','opaque')")
                .execute(store.pool())
                .await?;
            sqlx::query("INSERT INTO device_contacts VALUES('owner-device','owner','opaque')")
                .execute(store.pool())
                .await?;
            commit(&store, &request(0, Change::Initialize)).await?;
            let mut role = default_roles().remove(1);
            role.description = "Existing operator customization".into();
            commit(&store, &request(1, Change::PutRole { role })).await?;
        }
        let before: Vec<(String, String)> =
            sqlx::query_as("SELECT id,definition FROM server_roles ORDER BY id")
                .fetch_all(store.pool())
                .await?;
        let migration = std::fs::read_to_string(source.join(next))?;
        std::fs::write(
            staged.path().join(next),
            format!("{migration}\nSELECT * FROM missing_defaults_failure;"),
        )?;
        assert!(store.migrate(staged.path()).await.is_err());
        let version: i64 = sqlx::query_scalar("SELECT version FROM cords_schema_metadata")
            .fetch_one(store.pool())
            .await?;
        assert_eq!(version, 13);
        let after: Vec<(String, String)> =
            sqlx::query_as("SELECT id,definition FROM server_roles ORDER BY id")
                .fetch_all(store.pool())
                .await?;
        assert_eq!(after, before);
        std::fs::write(staged.path().join(next), migration)?;
        copy_later_migrations(&source, staged.path(), next)?;
        for _ in 0..2 {
            store.migrate(staged.path()).await?;
            store.require_current().await?;
        }
        let mut tx = store.pool().begin().await?;
        let snapshot = load(&mut tx).await?;
        assert!(!snapshot.active);
        assert_eq!(snapshot.revision, if edited { 2 } else { 0 });
        let policy = snapshot.policy.context("defaults exist without setup")?;
        if edited {
            assert_eq!(
                policy.roles()[cords_protocol::roles::MODERATOR_ROLE].description,
                "Existing operator customization"
            );
            let after: Vec<(String, String)> =
                sqlx::query_as("SELECT id,definition FROM server_roles ORDER BY id")
                    .fetch_all(&mut *tx)
                    .await?;
            assert_eq!(after, before);
        } else {
            let expected = Policy::new(default_roles(), DeviceAdmission::AccountAuthorized)?;
            assert_eq!(policy.roles(), expected.roles());
        }
        let audits: i64 = sqlx::query_scalar("SELECT count(*) FROM role_policy_changes")
            .fetch_one(&mut *tx)
            .await?;
        assert_eq!(audits, if edited { 2 } else { 0 });
        tx.rollback().await?;
        if !edited {
            sqlx::query("INSERT INTO account_heads VALUES('owner','opaque')")
                .execute(store.pool())
                .await?;
            sqlx::query("INSERT INTO device_contacts VALUES('owner-device','owner','opaque')")
                .execute(store.pool())
                .await?;
            let mut role = default_roles().remove(1);
            role.description = "First edit without setup".into();
            let edit = request(0, Change::PutRole { role });
            assert_eq!(commit(&store, &edit).await?, 1);
            assert_eq!(commit(&store, &edit).await?, 1);
        }
    }
    Ok(())
}

fn copy_later_migrations(source: &Path, staged: &Path, after: &str) -> Result<()> {
    for file in std::fs::read_dir(source)? {
        let file = file?;
        if file.file_name().to_string_lossy().as_ref() > after {
            std::fs::copy(file.path(), staged.join(file.file_name()))?;
        }
    }
    Ok(())
}
