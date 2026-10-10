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
        if version != Some(9) {
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
}
