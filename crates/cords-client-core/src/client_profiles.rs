//! Verified account presentation and sealed, account-isolated media cache.
use super::*;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use cords_protocol::profiles::{
    AccountProfile, ProfileAvatar, ProfileHistoryPage, ProfilePublish, ProfileReceipt,
    ProfileRecord, ProfileRevision, ProfileState,
};

#[derive(Debug, Serialize)]
pub struct ProfileView {
    pub state: ProfileState,
    pub image: Option<String>,
    pub pending_publication: Option<String>,
    pub candidates: Vec<ProfileCandidate>,
}

#[derive(Debug, Serialize)]
pub struct ProfileCandidate {
    pub id: String,
    pub profile: AccountProfile,
}

fn transport_unavailable(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        cause
            .downcast_ref::<reqwest::Error>()
            .is_some_and(|error| error.is_connect() || error.is_timeout())
    })
}

impl Client {
    /// Exercise the server's account/session binding using an already verified
    /// public revision. Available only in explicit integration-test builds.
    /// # Errors
    /// Returns the server refusal, or fails if no verified foreign record exists.
    #[cfg(feature = "fault-injection")]
    pub async fn test_publish_foreign_profile(&mut self, account: &str) -> Result<ProfileReceipt> {
        ensure!(
            account != self.identity.authorization.value.account_id,
            "Foreign account required"
        );
        self.account_profile(account).await?;
        let record = self
            .durable
            .account_profiles
            .get(account)
            .and_then(|graph| graph.records.values().next())
            .context("No verified foreign profile record")?
            .clone();
        self.post(
            "/api/v1/account-profiles",
            &ProfilePublish { record, image: None },
            true,
        )
        .await
    }

    async fn profile_compatible(&mut self) -> Result<()> {
        self.ensure_session().await?;
        let discovery: SignedServerMetadataV1 = Self::response(
            self.http
                .get(format!("{}/.well-known/cords/server", self.durable.origin))
                .send()
                .await?,
        )
        .await?;
        discovery.metadata.verify(&discovery.signature)?;
        ensure!(
            discovery.metadata.server_id == self.durable.server_id
                && discovery.metadata.server_signing_key == self.durable.server_key,
            "CORDS_SERVER_KEY_CHANGED"
        );
        ensure!(
            discovery
                .metadata
                .features
                .iter()
                .any(|feature| feature == cords_protocol::profiles::FEATURE),
            "This server needs an update for account profiles (account-profile-history-v1)"
        );
        Ok(())
    }

    pub(super) async fn cached_profile_image(
        &self,
        image: &ProfileAvatar,
    ) -> Result<Option<Vec<u8>>> {
        let sealed: Option<Vec<u8>> =
            sqlx::query_scalar("SELECT sealed FROM profile_images WHERE hash=?1")
                .bind(&image.sha256)
                .fetch_optional(self.store.pool())
                .await?;
        let Some(sealed) = sealed else {
            return Ok(None);
        };
        let bytes = protection::open(
            &self.key,
            format!("{}/profile-image/{}/v1", self.installation, image.sha256).as_bytes(),
            &sealed,
        )?;
        ensure!(
            bytes.len() == image.bytes as usize && hash(&bytes) == image.sha256,
            "Corrupt sealed profile picture"
        );
        sqlx::query("UPDATE profile_images SET accessed=?2 WHERE hash=?1")
            .bind(&image.sha256)
            .bind(i64::try_from(now()?)?)
            .execute(self.store.pool())
            .await?;
        Ok(Some(bytes.to_vec()))
    }

    pub(super) async fn cache_profile_image(
        &self,
        image: &ProfileAvatar,
        bytes: &[u8],
    ) -> Result<()> {
        ensure!(
            bytes.len() == image.bytes as usize
                && bytes.len() <= 5_000_000
                && hash(bytes) == image.sha256,
            "Profile-picture hash or size mismatch"
        );
        let sealed = protection::seal(
            &self.key,
            format!("{}/profile-image/{}/v1", self.installation, image.sha256).as_bytes(),
            bytes,
        )?;
        let mut tx = self.store.pool().begin().await?;
        sqlx::query("INSERT INTO profile_images(hash,sealed,accessed) VALUES(?1,?2,?3) ON CONFLICT(hash) DO UPDATE SET accessed=excluded.accessed").bind(&image.sha256).bind(sealed).bind(i64::try_from(now()?)?).execute(&mut *tx).await?;
        // Own revision images remain available for offline publication to other
        // servers. Evict only remote cache entries; never compact signed history.
        let own = &self.identity.authorization.value.account_id;
        let protected: Vec<String> = self
            .durable
            .account_profiles
            .get(own)
            .into_iter()
            .flat_map(|graph| graph.records.values())
            .filter_map(|record| {
                record
                    .revision
                    .value
                    .profile
                    .avatar
                    .as_ref()
                    .map(|a| a.sha256.clone())
            })
            .chain(std::iter::once(image.sha256.clone()))
            .collect();
        let rows = sqlx::query(
            "SELECT hash,length(sealed) AS bytes FROM profile_images ORDER BY accessed DESC,hash",
        )
        .fetch_all(&mut *tx)
        .await?;
        let mut total = 0_i64;
        for row in rows {
            total += row.try_get::<i64, _>("bytes")?;
            let hash: String = row.try_get("hash")?;
            if total > 128 * 1024 * 1024 && !protected.contains(&hash) {
                sqlx::query("DELETE FROM profile_images WHERE hash=?1")
                    .bind(hash)
                    .execute(&mut *tx)
                    .await?;
                total -= row.try_get::<i64, _>("bytes")?;
            }
        }
        ensure!(
            total <= 128 * 1024 * 1024,
            "Profile-picture cache is full; preserved own history cannot be evicted"
        );
        tx.commit().await?;
        Ok(())
    }

    async fn fetch_profile_image(&self, account: &str, image: &ProfileAvatar) -> Result<Vec<u8>> {
        if let Some(bytes) = self.cached_profile_image(image).await? {
            return Ok(bytes);
        }
        let session = self.durable.session.as_ref().context("not authenticated")?;
        let mut response = self
            .http
            .get(format!(
                "{}/api/v1/account-profiles/{account}/images/{}",
                self.durable.origin, image.sha256
            ))
            .bearer_auth(&session.token)
            .send()
            .await?;
        ensure!(
            response.status().is_success(),
            "Profile-picture retrieval failed: {}",
            response.status()
        );
        ensure!(
            response.content_length().is_none_or(|n| n <= 5_000_000),
            "Profile picture exceeds 5 MB"
        );
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            ensure!(
                bytes.len().saturating_add(chunk.len()) <= image.bytes as usize,
                "Profile picture exceeds signed size"
            );
            bytes.extend_from_slice(&chunk);
        }
        self.cache_profile_image(image, &bytes).await?;
        Ok(bytes)
    }

    /// Synchronize signed history and present only the causally agreed profile.
    /// # Errors
    /// Rejects incompatible servers, signature failures, missing ancestry, quota
    /// violations, stale identity state and unavailable authenticated transport.
    pub async fn account_profile(&mut self, account: &str) -> Result<ProfileView> {
        if account == self.identity.authorization.value.account_id
            && self.durable.session.is_none()
            && self.durable.admission_state != "active"
        {
            return self.local_profile_view(account, Some("Saved on this device. Connect to an approved, compatible server to publish your profile.".into())).await;
        }
        match self.synchronize_account_profile(account).await {
            Ok(view) => Ok(view),
            Err(error) if transport_unavailable(&error) => self.local_profile_view(account, Some(format!("Offline: showing verified cached profile. Synchronization will retry: {error:#}"))).await,
            Err(error) => Err(error),
        }
    }

    /// Read canonical presentation metadata already verified in this vault.
    /// This performs no network request and does not choose a conflicting field.
    /// # Errors
    /// Rejects an invalid account fingerprint or inconsistent cached profile graph.
    pub fn cached_account_profile_state(&self, account: &str) -> Result<ProfileState> {
        ensure!(decode(account)?.len() == 32, "Invalid account fingerprint");
        Ok(self
            .durable
            .account_profiles
            .get(account)
            .cloned()
            .unwrap_or_default()
            .state()?)
    }

    async fn local_profile_view(
        &self,
        account: &str,
        pending_publication: Option<String>,
    ) -> Result<ProfileView> {
        let graph = self
            .durable
            .account_profiles
            .get(account)
            .cloned()
            .unwrap_or_default();
        let state = graph.state()?;
        let image = match state
            .agreed
            .as_ref()
            .and_then(|profile| profile.avatar.as_ref())
        {
            Some(image) => self
                .cached_profile_image(image)
                .await?
                .map(|bytes| STANDARD.encode(bytes)),
            None => None,
        };
        let candidates = state
            .heads
            .iter()
            .map(|id| ProfileCandidate {
                id: id.clone(),
                profile: graph.records[id].revision.value.profile.clone(),
            })
            .collect();
        Ok(ProfileView {
            state,
            image,
            pending_publication,
            candidates,
        })
    }

    async fn synchronize_account_profile(&mut self, account: &str) -> Result<ProfileView> {
        ensure!(decode(account)?.len() == 32, "Invalid account fingerprint");
        self.profile_compatible().await?;
        let remote = self.pull_profile_history(account).await?;
        let own = account == self.identity.authorization.value.account_id;
        let state = self.durable.account_profiles[account].state()?;
        if own
            && state.heads.len() > 1
            && state.conflicts.is_empty()
            && state.pending.is_empty()
            && let Some(candidate) = state.merge_candidate
        {
            self.sign_profile(candidate).await?;
        }
        let mut pending_publication = None;
        if own {
            let mut records: Vec<_> = self.durable.account_profiles[account]
                .records
                .values()
                .cloned()
                .collect();
            records.sort_by_key(|record| record.revision.value.generation);
            for record in records {
                if !remote.contains(&record.id()?) {
                    let image = match &record.revision.value.profile.avatar {
                        Some(image) => Some(
                            STANDARD.encode(
                                self.cached_profile_image(image)
                                    .await?
                                    .context("An offline profile image is missing")?,
                            ),
                        ),
                        None => None,
                    };
                    // Durable signed records precede submission. A lost response
                    // is retried idempotently on the next synchronization.
                    let receipt: Result<ProfileReceipt> = self
                        .post(
                            "/api/v1/account-profiles",
                            &ProfilePublish {
                                record: record.clone(),
                                image,
                            },
                            true,
                        )
                        .await;
                    match receipt {
                        Ok(receipt) => {
                            ensure!(receipt.id == record.id()?, "Substituted profile receipt");
                        }
                        Err(error) => {
                            pending_publication =
                                Some(format!("Saved locally; publication will retry: {error:#}"));
                            break;
                        }
                    }
                }
            }
        }
        let state = self.durable.account_profiles[account].state()?;
        let image = match state
            .agreed
            .as_ref()
            .and_then(|profile| profile.avatar.as_ref())
        {
            Some(image) => Some(STANDARD.encode(self.fetch_profile_image(account, image).await?)),
            None => None,
        };
        Ok(ProfileView {
            candidates: state
                .heads
                .iter()
                .map(|id| ProfileCandidate {
                    id: id.clone(),
                    profile: self.durable.account_profiles[account].records[id]
                        .revision
                        .value
                        .profile
                        .clone(),
                })
                .collect(),
            state,
            image,
            pending_publication,
        })
    }

    /// Retrieve only a hash already bound by a verified record in this vault.
    /// # Errors
    /// Unknown hashes and unavailable or corrupt authenticated media are rejected.
    pub async fn account_profile_image(&mut self, account: &str, sha256: &str) -> Result<String> {
        let image = self
            .durable
            .account_profiles
            .get(account)
            .and_then(|graph| {
                graph.records.values().find_map(|record| {
                    record
                        .revision
                        .value
                        .profile
                        .avatar
                        .as_ref()
                        .filter(|image| image.sha256 == sha256)
                })
            })
            .cloned()
            .context("Picture is not bound by a verified account profile")?;
        if let Some(bytes) = self.cached_profile_image(&image).await? {
            return Ok(STANDARD.encode(bytes));
        }
        self.ensure_session().await?;
        Ok(STANDARD.encode(self.fetch_profile_image(account, &image).await?))
    }

    async fn pull_profile_history(
        &mut self,
        account: &str,
    ) -> Result<std::collections::BTreeSet<String>> {
        let mut graph = self
            .durable
            .account_profiles
            .get(account)
            .cloned()
            .unwrap_or_default();
        let mut after = String::new();
        let mut remote = std::collections::BTreeSet::new();
        loop {
            let page: ProfileHistoryPage = self
                .get(&format!("/api/v1/account-profiles/{account}?after={after}"))
                .await?;
            ensure!(
                page.version == 1
                    && page.server_id == self.durable.server_id
                    && page.account_id == account
                    && page.records.len() <= 16
                    && page.heads.len() <= 64,
                "Invalid profile-history page"
            );
            let mut previous = after.clone();
            for record in page.records {
                let id = record.id()?;
                ensure!(
                    id > previous && record.revision.value.account_id == account,
                    "Substituted or unordered profile history"
                );
                previous.clone_from(&id);
                remote.insert(id);
                graph.insert(record)?;
            }
            ensure!(
                graph.records.len() <= 16384
                    && serde_json::to_vec(&graph)?.len() <= 16 * 1024 * 1024,
                "Profile history exceeds local limit; existing history was preserved"
            );
            match page.next_after {
                Some(next) => {
                    ensure!(
                        next == previous && next > after,
                        "Invalid profile-history cursor"
                    );
                    after = next;
                }
                None => break,
            }
        }
        graph.state()?;
        self.durable.account_profiles.insert(account.into(), graph);
        self.persist(&[], &[]).await?;
        Ok(remote)
    }

    async fn sign_profile(&mut self, profile: AccountProfile) -> Result<()> {
        ensure!(
            !self.durable.revoked && !self.durable.burned,
            "This device cannot publish profiles"
        );
        let account = self.identity.authorization.value.account_id.clone();
        let mut graph = self
            .durable
            .account_profiles
            .get(&account)
            .cloned()
            .unwrap_or_default();
        let state = graph.state()?;
        ensure!(
            state.pending.is_empty(),
            "Profile ancestry is missing; synchronize before editing"
        );
        let changed = graph.changed_fields(&profile)?;
        if state.heads.len() == 1 && changed.is_empty() {
            return Ok(());
        }
        let generation = state
            .heads
            .iter()
            .map(|head| graph.records[head].revision.value.generation)
            .max()
            .map_or(Some(0), |generation| generation.checked_add(1))
            .context("Profile generation overflow")?;
        let record = ProfileRecord {
            authorization: self.identity.authorization.clone(),
            revision: self.identity.sign_device(ProfileRevision {
                version: 1,
                account_id: account.clone(),
                device_id: self.identity.authorization.value.device_id.clone(),
                authorization_hash: hash(canonical(&self.identity.authorization)?),
                parents: state.heads,
                generation,
                changed,
                profile,
            })?,
        };
        graph.insert(record)?;
        graph.state()?;
        self.durable.account_profiles.insert(account, graph);
        self.persist(&[], &[]).await
    }

    /// Explicitly sign the reviewed name and complete avatar field. Passing the
    /// expected heads prevents a concurrent update being silently resolved.
    /// # Errors
    /// Rejects stale review state, invalid media, failed persistence or transport.
    pub async fn save_account_profile(
        &mut self,
        profile: AccountProfile,
        image: Option<String>,
        expected_heads: Vec<String>,
    ) -> Result<ProfileView> {
        let account = self.identity.authorization.value.account_id.clone();
        let current = self.account_profile(&account).await?;
        ensure!(
            current.state.heads == expected_heads,
            "Profile changed while editing. Review the new profile before saving."
        );
        if let Some(image) = image {
            let avatar = profile
                .avatar
                .as_ref()
                .context("Missing avatar descriptor")?;
            ensure!(image.len() <= 6_666_668, "Profile image exceeds 5 MB");
            self.cache_profile_image(avatar, &STANDARD.decode(image)?)
                .await?;
        }
        self.sign_profile(profile).await?;
        self.account_profile(&account).await
    }
}
