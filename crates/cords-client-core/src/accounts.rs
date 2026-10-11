//! Device-local account registry. It contains lock-screen metadata, never vault secrets.
use crate::client::Client;
use anyhow::{Context as _, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use sqlx::{Row as _, SqlitePool};
use std::{
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use zeroize::Zeroizing;

const DEFAULT_AUTO_LOCK_MINUTES: i64 = 15;
const COMMON_PASSWORDS: &[&str] = &[
    "password",
    "password123",
    "letmein",
    "qwerty",
    "123456789012",
    "iloveyou",
    "correcthorsebatterystaple",
];

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)] // These are independent user-facing policy toggles.
pub struct AccountSummary {
    pub account_id: String,
    pub nickname: String,
    pub avatar_data: String,
    pub genericize: bool,
    pub hide_nickname_on_lock: bool,
    pub auto_lock_minutes: Option<u32>,
    pub lock_on_os_lock: bool,
    pub lock_on_suspend: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct PasswordAssessment {
    pub weak: bool,
    pub reasons: Vec<String>,
}

/// Assess locally obvious password weaknesses. This never transmits or persists the password.
#[must_use]
pub fn assess_password(password: &str, nickname: &str) -> PasswordAssessment {
    let folded = password.to_lowercase();
    let nickname = nickname.trim().to_lowercase();
    let mut reasons = Vec::new();
    if password.chars().count() < 12 {
        reasons.push("Use at least 12 characters.".into());
    }
    if !nickname.is_empty() && nickname.chars().count() >= 3 && folded.contains(&nickname) {
        reasons.push("The password contains the account nickname.".into());
    }
    let compact: String = folded.chars().filter(|c| !c.is_whitespace()).collect();
    if COMMON_PASSWORDS
        .iter()
        .any(|candidate| compact.contains(candidate))
    {
        reasons.push("The password contains a widely used or example password.".into());
    }
    let chars: Vec<char> = folded.chars().collect();
    if chars.len() >= 12 && chars.windows(2).all(|pair| pair[0] == pair[1]) {
        reasons.push("The password repeats one character.".into());
    }
    PasswordAssessment {
        weak: !reasons.is_empty(),
        reasons,
    }
}

#[derive(Clone, Debug)]
pub struct AccountRegistry {
    root: PathBuf,
    migrations: PathBuf,
    ca: Option<PathBuf>,
    pool: SqlitePool,
}

impl AccountRegistry {
    /// Open or initialize the non-secret device registry.
    /// # Errors
    /// Returns an error when the registry directory or database cannot be opened or initialized.
    pub async fn open(root: &Path, migrations: &Path, ca: Option<&Path>) -> Result<Self> {
        std::fs::create_dir_all(root)?;
        let database = root.join("accounts.db");
        let pool =
            SqlitePool::connect(&format!("sqlite://{}?mode=rwc", database.display())).await?;
        sqlx::raw_sql(
            "PRAGMA journal_mode=WAL;
             CREATE TABLE IF NOT EXISTS local_accounts(
               account_id TEXT PRIMARY KEY,
               local_id TEXT NOT NULL UNIQUE,
               nickname TEXT NOT NULL,
               avatar_data TEXT NOT NULL DEFAULT '',
               genericize INTEGER NOT NULL DEFAULT 0,
               hide_nickname_on_lock INTEGER NOT NULL DEFAULT 0,
               auto_lock_minutes INTEGER,
               lock_on_os_lock INTEGER NOT NULL DEFAULT 1,
               lock_on_suspend INTEGER NOT NULL DEFAULT 1,
               created_at INTEGER NOT NULL
             );",
        )
        .execute(&pool)
        .await?;
        Ok(Self {
            root: root.to_path_buf(),
            migrations: migrations.to_path_buf(),
            ca: ca.map(Path::to_path_buf),
            pool,
        })
    }

    #[must_use]
    pub fn legacy_vault_exists(&self) -> bool {
        self.root.join("client.db").is_file()
    }

    /// List lock-screen-safe local account metadata.
    /// # Errors
    /// Returns an error when registry rows are unavailable or invalid.
    pub async fn list(&self) -> Result<Vec<AccountSummary>> {
        let rows = sqlx::query("SELECT account_id,nickname,avatar_data,genericize,hide_nickname_on_lock,auto_lock_minutes,lock_on_os_lock,lock_on_suspend FROM local_accounts ORDER BY created_at,account_id")
            .fetch_all(&self.pool).await?;
        rows.into_iter()
            .map(|row| {
                let minutes: Option<i64> = row.try_get("auto_lock_minutes")?;
                Ok(AccountSummary {
                    account_id: row.try_get("account_id")?,
                    nickname: row.try_get("nickname")?,
                    avatar_data: row.try_get("avatar_data")?,
                    genericize: row.try_get::<i64, _>("genericize")? != 0,
                    hide_nickname_on_lock: row.try_get::<i64, _>("hide_nickname_on_lock")? != 0,
                    auto_lock_minutes: minutes.map(u32::try_from).transpose()?,
                    lock_on_os_lock: row.try_get::<i64, _>("lock_on_os_lock")? != 0,
                    lock_on_suspend: row.try_get::<i64, _>("lock_on_suspend")? != 0,
                })
            })
            .collect()
    }

    async fn location(&self, account_id: &str) -> Result<PathBuf> {
        let local_id: String =
            sqlx::query_scalar("SELECT local_id FROM local_accounts WHERE account_id=?1")
                .bind(account_id)
                .fetch_optional(&self.pool)
                .await?
                .context("local account not found")?;
        ensure!(
            uuid::Uuid::parse_str(&local_id).is_ok(),
            "invalid local account registry entry"
        );
        Ok(self.root.join("accounts").join(local_id))
    }

    /// Create and register an independently protected local account vault.
    /// # Errors
    /// Returns an error for invalid input, weak-password confirmation, or failed durable storage.
    pub async fn create(&self, nickname: &str, password: &str, allow_weak: bool) -> Result<Client> {
        let nickname = nickname.trim();
        ensure!(
            !nickname.is_empty() && nickname.chars().count() <= 100,
            "enter a nickname of 1 to 100 characters"
        );
        let assessment = assess_password(password, nickname);
        ensure!(
            password.chars().count() >= 12,
            "use a password of at least 12 characters"
        );
        ensure!(
            !assessment.weak || allow_weak,
            "weak password requires explicit confirmation"
        );
        let local_id = uuid::Uuid::now_v7().to_string();
        let directory = self.root.join("accounts").join(&local_id);
        std::fs::create_dir_all(&directory)?;
        let secret = Zeroizing::new(password.as_bytes().to_vec());
        let mut client =
            match Client::create(&directory, &self.migrations, &secret, self.ca.as_deref()).await {
                Ok(client) => client,
                Err(error) => {
                    let _ = std::fs::remove_dir(&directory);
                    return Err(error);
                }
            };
        if let Err(error) = client
            .save_ui_preferences(serde_json::json!({"version":1,"displayName":nickname}))
            .await
        {
            client.shutdown().await;
            std::fs::remove_dir_all(&directory)
                .context("remove unregistered account vault after profile initialization failed")?;
            return Err(error);
        }
        let account_id = client.status().account_id;
        let created_at = i64::try_from(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs())?;
        if let Err(error) = sqlx::query("INSERT INTO local_accounts(account_id,local_id,nickname,auto_lock_minutes,created_at) VALUES(?1,?2,?3,?4,?5)")
            .bind(&account_id).bind(&local_id).bind(nickname).bind(DEFAULT_AUTO_LOCK_MINUTES).bind(created_at).execute(&self.pool).await {
            client.shutdown().await;
            std::fs::remove_dir_all(&directory).context("remove unregistered account vault")?;
            return Err(error.into());
        }
        Ok(client)
    }

    /// Unlock exactly one registered local account.
    /// # Errors
    /// Returns an error when the account is absent, mismatched, or cannot be authenticated.
    pub async fn unlock(&self, account_id: &str, password: &str) -> Result<Client> {
        let directory = self.location(account_id).await?;
        let secret = Zeroizing::new(password.as_bytes().to_vec());
        let mut client =
            Client::open_existing(&directory, &self.migrations, &secret, self.ca.as_deref())
                .await?;
        ensure!(
            client.status().account_id == account_id,
            "vault identity does not match account registry"
        );
        let mut preferences = client.ui_preferences();
        if preferences.get("displayName").is_none() {
            let nickname: String =
                sqlx::query_scalar("SELECT nickname FROM local_accounts WHERE account_id=?1")
                    .bind(account_id)
                    .fetch_one(&self.pool)
                    .await?;
            if !preferences.is_object() {
                preferences = serde_json::json!({"version":1});
            }
            preferences["displayName"] = serde_json::Value::String(nickname);
            client.save_ui_preferences(preferences).await?;
        }
        Ok(client)
    }

    /// Update the explicitly non-secret account-picker metadata and lock policy.
    /// # Errors
    /// Returns an error for invalid metadata or failed registry storage.
    pub async fn update_metadata(&self, summary: &AccountSummary) -> Result<()> {
        ensure!(
            summary.nickname.chars().count() <= 100,
            "nickname is too long"
        );
        ensure!(
            summary.avatar_data.is_empty()
                || (summary.avatar_data.starts_with("data:image/png;base64,")
                    && summary.avatar_data.len() < 900_000),
            "invalid cached avatar"
        );
        ensure!(
            matches!(summary.auto_lock_minutes, None | Some(1 | 5 | 15 | 30 | 60)),
            "invalid auto-lock interval"
        );
        let changed = sqlx::query("UPDATE local_accounts SET nickname=?2,avatar_data=?3,genericize=?4,hide_nickname_on_lock=?5,auto_lock_minutes=?6,lock_on_os_lock=?7,lock_on_suspend=?8 WHERE account_id=?1")
            .bind(&summary.account_id).bind(&summary.nickname).bind(&summary.avatar_data)
            .bind(summary.genericize).bind(summary.hide_nickname_on_lock)
            .bind(summary.auto_lock_minutes.map(i64::from)).bind(summary.lock_on_os_lock).bind(summary.lock_on_suspend)
            .execute(&self.pool).await?.rows_affected();
        ensure!(changed == 1, "local account not found");
        Ok(())
    }

    /// Authenticate and remove one local vault without issuing identity or device revocation.
    /// # Errors
    /// Returns an error when authentication, database closure, registry mutation, or deletion fails.
    pub async fn remove(&self, account_id: &str, password: &str) -> Result<()> {
        let directory = self.location(account_id).await?;
        self.unlock(account_id, password).await?.shutdown().await;
        let mut tx = self.pool.begin().await?;
        let changed = sqlx::query("DELETE FROM local_accounts WHERE account_id=?1")
            .bind(account_id)
            .execute(&mut *tx)
            .await?
            .rows_affected();
        ensure!(changed == 1, "local account not found");
        if let Err(error) = std::fs::remove_dir_all(&directory) {
            tx.rollback().await?;
            return Err(error).context("delete authenticated local account vault");
        }
        tx.commit().await?;
        Ok(())
    }

    /// Copy, rewrap, verify, and register a legacy single-installation vault.
    /// # Errors
    /// Returns an error without registering the copy when any unlock, copy, or verification fails.
    pub async fn migrate_legacy(
        &self,
        current_password: Option<&str>,
        new_password: &str,
        nickname: &str,
        allow_weak: bool,
    ) -> Result<Client> {
        ensure!(self.legacy_vault_exists(), "legacy vault not found");
        ensure!(
            new_password.chars().count() >= 12,
            "use a password of at least 12 characters"
        );
        ensure!(
            !assess_password(new_password, nickname).weak || allow_weak,
            "weak password requires explicit confirmation"
        );
        let current = current_password.map(|value| Zeroizing::new(value.as_bytes().to_vec()));
        let legacy = Box::pin(Client::open(
            &self.root,
            &self.migrations,
            current.as_deref().map(Vec::as_slice),
            self.ca.as_deref(),
        ))
        .await?;
        let expected = legacy.status();
        legacy.shutdown().await;
        let local_id = uuid::Uuid::now_v7().to_string();
        let directory = self.root.join("accounts").join(&local_id);
        std::fs::create_dir_all(&directory)?;
        std::fs::copy(self.root.join("client.db"), directory.join("client.db"))
            .context("copy legacy vault")?;
        let mut copied = match Box::pin(Client::open(
            &directory,
            &self.migrations,
            current.as_deref().map(Vec::as_slice),
            self.ca.as_deref(),
        ))
        .await
        {
            Ok(client) => client,
            Err(error) => {
                std::fs::remove_dir_all(&directory)?;
                return Err(error);
            }
        };
        copied.rewrap_password(new_password.as_bytes()).await?;
        copied.shutdown().await;
        let verified = Box::pin(Client::open_existing(
            &directory,
            &self.migrations,
            new_password.as_bytes(),
            self.ca.as_deref(),
        ))
        .await?;
        ensure!(
            verified.status().account_id == expected.account_id
                && verified.status().device_id == expected.device_id,
            "migrated vault identity verification failed"
        );
        let created_at = i64::try_from(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs())?;
        if sqlx::query("INSERT INTO local_accounts(account_id,local_id,nickname,auto_lock_minutes,created_at) VALUES(?1,?2,?3,?4,?5)")
            .bind(&expected.account_id).bind(&local_id).bind(nickname.trim()).bind(DEFAULT_AUTO_LOCK_MINUTES).bind(created_at).execute(&self.pool).await.is_err() {
            verified.shutdown().await;
            std::fs::remove_dir_all(&directory)?;
            bail!("legacy account is already registered");
        }
        Ok(verified)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn migrations() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../migrations/sqlite")
    }
    #[tokio::test]
    async fn accounts_are_independent_and_removal_requires_the_right_password() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let registry = AccountRegistry::open(temp.path(), &migrations(), None).await?;
        let first = registry
            .create("Alice", "first account passphrase", false)
            .await?;
        let first_id = first.status().account_id;
        first.shutdown().await;
        let second = registry
            .create("Bob", "second account passphrase", false)
            .await?;
        let second_id = second.status().account_id;
        second.shutdown().await;
        assert_ne!(first_id, second_id);
        assert!(
            registry
                .unlock(&first_id, "second account passphrase")
                .await
                .is_err()
        );
        assert!(
            registry
                .unlock(&second_id, "first account passphrase")
                .await
                .is_err()
        );
        registry
            .unlock(&first_id, "first account passphrase")
            .await?
            .shutdown()
            .await;
        assert!(
            registry
                .remove(&first_id, "wrong password here")
                .await
                .is_err()
        );
        registry
            .remove(&first_id, "first account passphrase")
            .await?;
        assert!(
            registry
                .unlock(&first_id, "first account passphrase")
                .await
                .is_err()
        );
        registry
            .unlock(&second_id, "second account passphrase")
            .await?
            .shutdown()
            .await;
        Ok(())
    }

    #[tokio::test]
    async fn legacy_migration_preserves_identity_and_source_vault() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let old_password = b"legacy storage passphrase";
        let legacy = Client::open(temp.path(), &migrations(), Some(old_password), None).await?;
        let expected = legacy.status();
        legacy.shutdown().await;
        let registry = AccountRegistry::open(temp.path(), &migrations(), None).await?;
        let migrated = registry
            .migrate_legacy(
                Some("legacy storage passphrase"),
                "new independent account passphrase",
                "Migrated account",
                false,
            )
            .await?;
        assert_eq!(migrated.status().account_id, expected.account_id);
        assert_eq!(migrated.status().device_id, expected.device_id);
        let account_id = migrated.status().account_id;
        migrated.shutdown().await;
        assert!(temp.path().join("client.db").is_file());
        assert!(
            registry
                .unlock(&account_id, "legacy storage passphrase")
                .await
                .is_err()
        );
        registry
            .unlock(&account_id, "new independent account passphrase")
            .await?
            .shutdown()
            .await;
        Ok(())
    }

    #[test]
    fn password_policy_counts_unicode_and_requires_explicit_weak_override() {
        assert!(assess_password("password123456", "Alice").weak);
        assert!(!assess_password("fjord lamp cedar orbit", "Alice").weak);
        assert!("🧶".repeat(12).chars().count() >= 12);
    }
}
