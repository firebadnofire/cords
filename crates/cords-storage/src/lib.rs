//! Phase 0 `PostgreSQL` and `SQLite` lifecycle adapters.

use sqlx::{PgPool, SqlitePool, migrate::Migrator};
use std::path::Path;
use thiserror::Error;

#[derive(Clone, Debug)]
pub struct PostgresStore {
    pool: PgPool,
}

impl PostgresStore {
    /// Use an existing pool, including a separately scoped integration database.
    #[must_use]
    pub fn from_pool(pool: PgPool) -> Self {
        Self { pool }
    }
    /// Database adapter access for transactional server repositories.
    #[must_use]
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
    /// Connect to `PostgreSQL`.
    ///
    /// # Errors
    /// Returns [`StorageError`] if the database cannot be reached.
    pub async fn connect(url: &str) -> Result<Self, StorageError> {
        Ok(Self {
            pool: PgPool::connect(url).await?,
        })
    }

    /// Apply all migrations in `path`.
    ///
    /// # Errors
    /// Returns [`StorageError`] if migrations cannot be loaded or applied.
    pub async fn migrate(&self, path: &Path) -> Result<(), StorageError> {
        Migrator::new(path).await?.run(&self.pool).await?;
        Ok(())
    }

    /// Verify the current schema version rather than merely the table's existence.
    ///
    /// # Errors
    /// Returns [`StorageError`] if the schema is absent or inaccessible.
    pub async fn require_current(&self) -> Result<(), StorageError> {
        let version: Option<i64> =
            sqlx::query_scalar("SELECT version FROM cords_schema_metadata WHERE singleton = TRUE")
                .fetch_optional(&self.pool)
                .await?;
        if version != Some(12) {
            return Err(StorageError::SchemaVersion);
        }
        Ok(())
    }

    /// Read the identity already bound to this database.
    ///
    /// # Errors
    /// Returns an error when persistent state cannot be read.
    pub async fn server_identity(&self) -> Result<Option<String>, StorageError> {
        Ok(
            sqlx::query_scalar(
                "SELECT server_id FROM cords_server_identity WHERE singleton = TRUE",
            )
            .fetch_optional(&self.pool)
            .await?,
        )
    }

    /// Bind a database once, refusing replacement of an established identity.
    ///
    /// # Errors
    /// Returns an error on storage failure or identity mismatch.
    pub async fn bind_server_identity(&self, server_id: &str) -> Result<(), StorageError> {
        let mut transaction = self.pool.begin().await?;
        sqlx::query("INSERT INTO cords_server_identity(singleton, server_id) VALUES(TRUE, $1) ON CONFLICT(singleton) DO NOTHING")
            .bind(server_id).execute(&mut *transaction).await?;
        let existing: String = sqlx::query_scalar(
            "SELECT server_id FROM cords_server_identity WHERE singleton = TRUE FOR UPDATE",
        )
        .fetch_one(&mut *transaction)
        .await?;
        if existing != server_id {
            return Err(StorageError::ServerIdentityMismatch);
        }
        transaction.commit().await?;
        Ok(())
    }

    /// Check the database connection.
    ///
    /// # Errors
    /// Returns [`StorageError`] if the database is unavailable.
    pub async fn health(&self) -> Result<(), StorageError> {
        sqlx::query("SELECT 1").execute(&self.pool).await?;
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct SqliteStore {
    pool: SqlitePool,
}

impl SqliteStore {
    /// Access the `SQLite` transaction adapter for atomic client units of work.
    #[must_use]
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
    /// Open `SQLite`.
    ///
    /// # Errors
    /// Returns [`StorageError`] if the database cannot be opened.
    pub async fn connect(url: &str) -> Result<Self, StorageError> {
        Ok(Self {
            pool: sqlx::sqlite::SqlitePoolOptions::new()
                .max_connections(1)
                .connect_with(
                    url.parse::<sqlx::sqlite::SqliteConnectOptions>()?
                        .synchronous(sqlx::sqlite::SqliteSynchronous::Full)
                        .foreign_keys(true),
                )
                .await?,
        })
    }

    /// Apply all migrations in `path`.
    ///
    /// # Errors
    /// Returns [`StorageError`] if migrations cannot be loaded or applied.
    pub async fn migrate(&self, path: &Path) -> Result<(), StorageError> {
        Migrator::new(path).await?.run(&self.pool).await?;
        Ok(())
    }

    /// Check the database connection.
    ///
    /// # Errors
    /// Returns [`StorageError`] if the database is unavailable.
    pub async fn health(&self) -> Result<(), StorageError> {
        sqlx::query("SELECT 1").execute(&self.pool).await?;
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("database schema is not supported; run the packaged migrations before serving")]
    SchemaVersion,
    #[error(
        "server signing identity does not match this database; restore the matching persistent key"
    )]
    ServerIdentityMismatch,
    #[error("database operation failed")]
    Database(#[from] sqlx::Error),
    #[error("database migration failed")]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("migration directory could not be read")]
    MigrationSource(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn sqlite_migration_and_health_are_functional() -> Result<(), Box<dyn std::error::Error>>
    {
        let directory = tempfile::tempdir()?;
        let database = directory.path().join("cords.db");
        let url = format!("sqlite://{}?mode=rwc", database.display());
        let store = SqliteStore::connect(&url).await?;
        let migrations = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../migrations/sqlite");
        store.migrate(&migrations).await?;
        store.health().await?;
        Ok(())
    }

    // Opaque bytes deliberately test database preservation only. Actual sealed
    // vault/MLS continuity is exercised separately by client-core regressions.
    #[tokio::test]
    async fn sqlite_legacy_fixture_survives_failed_migration_retry_and_repeated_startup()
    -> Result<(), Box<dyn std::error::Error>> {
        const MESSAGE: &[u8] = b"opaque sealed message fixture";
        const STATE: &[u8] = b"opaque sealed state fixture";
        const ENVELOPE: &[u8] = b"opaque relay envelope fixture";
        let directory = tempfile::tempdir()?;
        let staged = directory.path().join("migrations");
        std::fs::create_dir(&staged)?;
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../migrations/sqlite");
        for name in ["202609290001_phase_0.sql", "202609300001_client.sql"] {
            std::fs::copy(source.join(name), staged.join(name))?;
        }
        let database = directory.path().join("legacy.db");
        let url = format!("sqlite://{}?mode=rwc", database.display());
        let store = SqliteStore::connect(&url).await?;
        store.migrate(&staged).await?;
        sqlx::query("INSERT INTO client_state(singleton,sealed) VALUES(1,?1)")
            .bind(STATE)
            .execute(store.pool())
            .await?;
        sqlx::query("INSERT INTO message_cache(route_id,message_id,sequence,sealed) VALUES('route-original','message-original',17,?1)")
            .bind(MESSAGE).execute(store.pool()).await?;
        sqlx::query("INSERT INTO ciphertext_cache(route_id,sequence,envelope) VALUES('route-original',17,?1)")
            .bind(ENVELOPE).execute(store.pool()).await?;

        let next = "202610100001_multiserver.sql";
        let migration = std::fs::read_to_string(source.join(next))?;
        // Fail after the migration's destructive table replacement and version
        // update. SQLx must roll the whole transaction back, not lose old rows.
        std::fs::write(
            staged.join(next),
            format!("{migration}\nSELECT * FROM cords_c0_missing_table;\n"),
        )?;
        assert!(store.migrate(&staged).await.is_err());
        let version: i64 = sqlx::query_scalar("SELECT version FROM cords_schema_metadata")
            .fetch_one(store.pool())
            .await?;
        assert_eq!(version, 2);
        let legacy: (String, String, i64, Vec<u8>) =
            sqlx::query_as("SELECT route_id,message_id,sequence,sealed FROM message_cache")
                .fetch_one(store.pool())
                .await?;
        assert_eq!(
            legacy,
            (
                "route-original".into(),
                "message-original".into(),
                17,
                MESSAGE.to_vec()
            )
        );
        let applied: i64 =
            sqlx::query_scalar("SELECT count(*) FROM _sqlx_migrations WHERE version=202610100001")
                .fetch_one(store.pool())
                .await?;
        assert_eq!(applied, 0);
        store.pool().close().await;

        std::fs::write(staged.join(next), migration)?;
        for _ in 0..2 {
            let store = SqliteStore::connect(&url).await?;
            store.migrate(&staged).await?;
            store.migrate(&staged).await?;
            let version: i64 = sqlx::query_scalar("SELECT version FROM cords_schema_metadata")
                .fetch_one(store.pool())
                .await?;
            assert_eq!(version, 3);
            let message: (String, String, String, i64, Vec<u8>) = sqlx::query_as(
                "SELECT server_id,route_id,message_id,sequence,sealed FROM message_cache",
            )
            .fetch_one(store.pool())
            .await?;
            // Empty server namespace is the genuine schema migration result.
            // Client-core must rekey it under the original pin at vault unlock.
            assert_eq!(
                message,
                (
                    String::new(),
                    "route-original".into(),
                    "message-original".into(),
                    17,
                    MESSAGE.to_vec()
                )
            );
            let envelope: (String, String, i64, Vec<u8>) =
                sqlx::query_as("SELECT server_id,route_id,sequence,envelope FROM ciphertext_cache")
                    .fetch_one(store.pool())
                    .await?;
            assert_eq!(
                envelope,
                (
                    String::new(),
                    "route-original".into(),
                    17,
                    ENVELOPE.to_vec()
                )
            );
            let state: Vec<u8> = sqlx::query_scalar("SELECT sealed FROM client_state")
                .fetch_one(store.pool())
                .await?;
            assert_eq!(state, STATE);
            store.pool().close().await;
        }

        Ok(())
    }

    #[tokio::test]
    async fn sqlite_upgraded_fixture_rejects_older_migration_set()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let database = directory.path().join("upgraded.db");
        let url = format!("sqlite://{}?mode=rwc", database.display());
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../migrations/sqlite");
        let store = SqliteStore::connect(&url).await?;
        store.migrate(&source).await?;
        // An older migration set must reject this isolated upgraded fixture;
        // it cannot silently downgrade its schema.
        let older = directory.path().join("older-migrations");
        std::fs::create_dir(&older)?;
        for name in ["202609290001_phase_0.sql", "202609300001_client.sql"] {
            std::fs::copy(source.join(name), older.join(name))?;
        }
        assert!(matches!(
            store.migrate(&older).await,
            Err(StorageError::Migration(
                sqlx::migrate::MigrateError::VersionMissing(202_610_100_001)
            ))
        ));
        let version: i64 = sqlx::query_scalar("SELECT version FROM cords_schema_metadata")
            .fetch_one(store.pool())
            .await?;
        assert_eq!(version, 3);
        store.pool().close().await;
        Ok(())
    }
}
