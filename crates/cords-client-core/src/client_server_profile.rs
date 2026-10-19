//! Verified, account-isolated server presentation cache.
use super::*;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use cords_protocol::server_profile::{
    FEATURE, ServerIconChange, ServerProfile, ServerProfileChange, valid_icon,
};

impl Client {
    #[must_use]
    pub fn supports_server_profile(&self) -> bool {
        self.durable
            .server_features
            .get(&self.durable.server_id)
            .is_some_and(|features| features.iter().any(|feature| feature == FEATURE))
    }

    #[must_use]
    pub fn cached_server_profile(&self) -> Option<&ServerProfile> {
        self.durable.server_profiles.get(&self.durable.server_id)
    }

    #[must_use]
    pub fn server_name(&self, server: &str) -> String {
        self.durable
            .server_profiles
            .get(server)
            .map(|p| &p.name)
            .filter(|n| !n.is_empty())
            .or_else(|| self.durable.server_names.get(server))
            .cloned()
            .unwrap_or_default()
    }

    async fn accept_server_profile(
        &mut self,
        signed: Signed<ServerProfile>,
    ) -> Result<ServerProfile> {
        signed.verify(&self.durable.server_key)?;
        let value = signed.value;
        ensure!(
            value.version == 1
                && value.server_id == self.durable.server_id
                && value.name.chars().count() <= 100
                && value.description.chars().count() <= 2000
                && cords_protocol::server_profile::valid_background(&value.background)
                && value.icon.as_ref().is_none_or(valid_icon),
            "Invalid signed server profile"
        );
        if let Some(previous) = self.cached_server_profile() {
            ensure!(
                value.revision >= previous.revision,
                "Server profile rollback detected"
            );
            ensure!(
                value.revision != previous.revision || &value == previous,
                "Conflicting server profile revision"
            );
        }
        self.durable
            .server_profiles
            .insert(value.server_id.clone(), value.clone());
        self.persist(&[], &[]).await?;
        Ok(value)
    }

    /// # Errors
    /// Returns authorization, compatibility, trust, rollback or transport failures.
    pub async fn server_profile(&mut self) -> Result<ServerProfile> {
        self.ensure_session().await?;
        if !self.supports_server_profile() {
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
            self.durable
                .server_features
                .insert(self.durable.server_id.clone(), discovery.metadata.features);
            self.durable.server_names.insert(
                self.durable.server_id.clone(),
                discovery.metadata.server_name,
            );
            self.persist(&[], &[]).await?;
        }
        ensure!(
            self.supports_server_profile(),
            "Server profile editing requires a server update"
        );
        let signed = self.get("/api/v1/server-profile").await?;
        let profile = self.accept_server_profile(signed).await?;
        self.fetch_server_icon().await?;
        Ok(profile)
    }

    /// # Errors
    /// Returns validation, stale-revision, authorization, storage or transport failures.
    pub async fn save_server_profile(
        &mut self,
        request: ServerProfileChange,
    ) -> Result<ServerProfile> {
        ensure!(
            request.valid(),
            "Use a server name of 1–100 characters and a description of at most 2000 characters"
        );
        self.ensure_session().await?;
        ensure!(
            self.supports_server_profile(),
            "Server profile editing requires a server update"
        );
        let signed: Signed<ServerProfile> = self.post("/api/v1/server-profile", &request, true).await.map_err(|error| {
            if error.downcast_ref::<ApiFailure>().is_some_and(|failure| failure.code == "CORDS_SERVER_PROFILE_CONFLICT") {
                error.context("This server profile changed. Discard the draft and refresh before saving again")
            } else { error }
        })?;
        ensure!(
            signed.value.revision
                == request
                    .expected_revision
                    .checked_add(1)
                    .context("Server revision exhausted")?
                && signed.value.name == request.name
                && signed.value.description == request.description
                && signed.value.background == request.background,
            "Server profile response does not match the requested change"
        );
        self.accept_server_profile(signed).await
    }

    /// # Errors
    /// Rejects corrupt sealed media. This never performs network requests.
    pub async fn cached_server_icon(&self, server: &str) -> Result<Option<(String, String)>> {
        let Some(icon) = self
            .durable
            .server_profiles
            .get(server)
            .and_then(|profile| profile.icon.as_ref())
        else {
            return Ok(None);
        };
        Ok(self
            .cached_profile_image(icon)
            .await?
            .map(|bytes| (STANDARD.encode(bytes), icon.shape.clone())))
    }

    async fn fetch_server_icon(&mut self) -> Result<()> {
        let Some(icon) = self
            .cached_server_profile()
            .and_then(|profile| profile.icon.clone())
        else {
            return Ok(());
        };
        if self.cached_profile_image(&icon).await?.is_some() {
            return Ok(());
        }
        self.ensure_session().await?;
        let token = &self
            .durable
            .session
            .as_ref()
            .context("No authenticated session")?
            .token;
        let mut response = self
            .http
            .get(format!(
                "{}/api/v1/server-profile/images/{}",
                self.durable.origin, icon.sha256
            ))
            .bearer_auth(token)
            .send()
            .await?;
        ensure!(
            response.status().is_success(),
            "Server icon retrieval failed: {}",
            response.status()
        );
        ensure!(
            response
                .content_length()
                .is_none_or(|n| n <= u64::from(icon.bytes)),
            "Server icon exceeds signed size"
        );
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            ensure!(
                bytes.len().saturating_add(chunk.len()) <= icon.bytes as usize,
                "Server icon exceeds signed size"
            );
            bytes.extend_from_slice(&chunk);
        }
        self.cache_profile_image(&icon, &bytes).await
    }

    /// # Errors
    /// Returns image validation, authority, revision, trust or transport errors.
    pub async fn save_server_icon(&mut self, request: ServerIconChange) -> Result<ServerProfile> {
        ensure!(
            request.valid(),
            "Invalid server icon; choose a square JXL image of at most 5 MB"
        );
        self.ensure_session().await?;
        ensure!(
            self.supports_server_profile(),
            "Server profile editing requires a server update"
        );
        let signed: Signed<ServerProfile> = self
            .post("/api/v1/server-profile/icon", &request, true)
            .await?;
        ensure!(
            signed.value.revision
                == request
                    .expected_revision
                    .checked_add(1)
                    .context("Server revision exhausted")?
                && signed.value.icon == request.icon,
            "Server icon response does not match the requested change"
        );
        let profile = self.accept_server_profile(signed).await?;
        if let (Some(icon), Some(image)) = (request.icon, request.image) {
            self.cache_profile_image(&icon, &STANDARD.decode(image)?)
                .await?;
        }
        Ok(profile)
    }
}
