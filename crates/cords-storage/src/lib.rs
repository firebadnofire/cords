//! Phase 0 `PostgreSQL` and `SQLite` lifecycle adapters.

use sqlx::{PgPool, SqlitePool, migrate::Migrator};
use std::path::Path;
use thiserror::Error;

#[derive(Clone, Debug)]
pub struct PostgresStore {
    pool: PgPool,
}

impl PostgresStore {
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

    /// Verify the Phase 0 schema sentinel.
    ///
    /// # Errors
    /// Returns [`StorageError`] if the schema is absent or inaccessible.
    pub async fn require_current(&self) -> Result<(), StorageError> {
        sqlx::query("SELECT version FROM cords_schema_metadata WHERE singleton = TRUE")
            .execute(&self.pool)
            .await?;
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
    /// Open `SQLite`.
    ///
    /// # Errors
    /// Returns [`StorageError`] if the database cannot be opened.
    pub async fn connect(url: &str) -> Result<Self, StorageError> {
        Ok(Self {
            pool: SqlitePool::connect(url).await?,
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
