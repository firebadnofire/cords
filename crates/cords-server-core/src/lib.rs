//! Phase 0 server identity and HTTP application services.

pub mod messaging;

use axum::{Json, Router, extract::State, http::StatusCode, routing::get};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use cords_protocol::{
    CapabilitiesV1, PROTOCOL_V1, ServerMetadataV1, SignedServerMetadataV1,
    server_id_from_public_key,
};
use ed25519_dalek::{Signer as _, SigningKey};
use rand_core::OsRng;
use std::{fs, io::Write as _, path::Path};
use tempfile::NamedTempFile;
use thiserror::Error;
use tower_http::{
    catch_panic::CatchPanicLayer,
    limit::RequestBodyLimitLayer,
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    timeout::TimeoutLayer,
    trace::TraceLayer,
};
use zeroize::Zeroizing;

#[derive(Clone, Debug)]
pub struct ServerIdentity {
    signing_key: SigningKey,
}

impl ServerIdentity {
    /// Load the persistent identity, generating it atomically when absent.
    ///
    /// # Errors
    /// Returns [`IdentityError`] if the directory is unavailable or the key file is invalid.
    pub fn load_or_create(path: &Path) -> Result<(Self, bool), IdentityError> {
        if path.exists() {
            return Ok((Self::load(path)?, false));
        }
        let parent = path.parent().ok_or(IdentityError::InvalidPath)?;
        fs::create_dir_all(parent)?;
        let signing_key = SigningKey::generate(&mut OsRng);
        let mut temporary = NamedTempFile::new_in(parent)?;
        temporary.write_all(&signing_key.to_bytes())?;
        temporary.as_file().sync_all()?;
        set_private_permissions(temporary.path())?;
        temporary
            .persist_noclobber(path)
            .map_err(|error| error.error)?;
        sync_directory(parent)?;
        Ok((Self { signing_key }, true))
    }

    fn load(path: &Path) -> Result<Self, IdentityError> {
        validate_private_permissions(path)?;
        let bytes = Zeroizing::new(fs::read(path)?);
        let secret: [u8; 32] = bytes
            .as_slice()
            .try_into()
            .map_err(|_| IdentityError::InvalidKeyFile)?;
        Ok(Self {
            signing_key: SigningKey::from_bytes(&secret),
        })
    }

    #[must_use]
    pub fn server_id(&self) -> String {
        server_id_from_public_key(&self.signing_key.verifying_key().to_bytes())
    }

    /// Sign Phase 0 discovery metadata with this server identity.
    ///
    /// # Errors
    /// Returns [`IdentityError`] when deterministic protocol encoding fails.
    pub fn signed_metadata(
        &self,
        server_name: &str,
    ) -> Result<SignedServerMetadataV1, IdentityError> {
        let metadata = ServerMetadataV1 {
            protocol_min: PROTOCOL_V1,
            protocol_max: PROTOCOL_V1,
            server_id: self.server_id(),
            server_name: server_name.to_owned(),
            api_base: "/api/v1".into(),
            websocket_path: "/api/v1/events".into(),
            server_signing_key: URL_SAFE_NO_PAD.encode(self.signing_key.verifying_key().to_bytes()),
            join_policy: vec!["public".into()],
            features: vec!["mls-v1".into(), "channel-sync-v1".into()],
        };
        let signature = self.signing_key.sign(&metadata.signing_bytes()?).to_bytes();
        Ok(SignedServerMetadataV1 {
            metadata,
            signature: URL_SAFE_NO_PAD.encode(signature),
        })
    }
}

#[cfg(unix)]
fn set_private_permissions(path: &Path) -> Result<(), std::io::Error> {
    use std::os::unix::fs::PermissionsExt as _;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
}

#[cfg(unix)]
fn validate_private_permissions(path: &Path) -> Result<(), IdentityError> {
    use std::os::unix::fs::PermissionsExt as _;
    let mode = fs::metadata(path)?.permissions().mode();
    if mode & 0o077 != 0 {
        return Err(IdentityError::InsecurePermissions);
    }
    Ok(())
}

#[cfg(not(unix))]
#[allow(clippy::unnecessary_wraps)] // Matches the Unix validation contract at the shared call site.
fn validate_private_permissions(_path: &Path) -> Result<(), IdentityError> {
    Ok(())
}

#[cfg(not(unix))]
#[allow(clippy::unnecessary_wraps)] // Matches the Unix permission-setting contract.
fn set_private_permissions(_path: &Path) -> Result<(), std::io::Error> {
    Ok(())
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> Result<(), std::io::Error> {
    fs::File::open(path)?.sync_all()
}

#[cfg(not(unix))]
#[allow(clippy::unnecessary_wraps)] // Matches the Unix durability contract at the shared call site.
fn sync_directory(_path: &Path) -> Result<(), std::io::Error> {
    Ok(())
}

#[derive(Clone, Debug)]
pub struct AppState {
    metadata: SignedServerMetadataV1,
    store: Option<cords_storage::PostgresStore>,
}

impl AppState {
    #[must_use]
    pub fn new(metadata: SignedServerMetadataV1) -> Self {
        Self {
            metadata,
            store: None,
        }
    }

    #[must_use]
    pub fn with_store(mut self, store: cords_storage::PostgresStore) -> Self {
        self.store = Some(store);
        self
    }
}

/// Create the Phase 0 Axum router. Readiness is added after storage startup succeeds.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health/live", get(live))
        .route("/health/ready", get(ready))
        .route("/.well-known/cords/server", get(metadata))
        .route("/api/v1/capabilities", get(capabilities))
        .route("/api/v1/servers/self", get(metadata))
        .with_state(state)
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(TraceLayer::new_for_http())
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            std::time::Duration::from_secs(15),
        ))
        .layer(RequestBodyLimitLayer::new(1024 * 1024))
        .layer(CatchPanicLayer::new())
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
}

async fn live() -> StatusCode {
    StatusCode::NO_CONTENT
}
async fn ready(State(state): State<AppState>) -> StatusCode {
    match &state.store {
        Some(store) if store.health().await.is_ok() => StatusCode::NO_CONTENT,
        _ => StatusCode::SERVICE_UNAVAILABLE,
    }
}
async fn metadata(State(state): State<AppState>) -> Json<SignedServerMetadataV1> {
    Json(state.metadata)
}
async fn capabilities() -> Json<CapabilitiesV1> {
    Json(CapabilitiesV1 {
        protocol_min: PROTOCOL_V1,
        protocol_max: PROTOCOL_V1,
        features: vec!["mls-v1".into(), "channel-sync-v1".into()],
    })
}

#[derive(Debug, Error)]
pub enum IdentityError {
    #[error("server identity path has no parent directory")]
    InvalidPath,
    #[error("server identity file is invalid")]
    InvalidKeyFile,
    #[error("server identity file permissions are too broad")]
    InsecurePermissions,
    #[error("server identity storage failed")]
    Io(#[from] std::io::Error),
    #[error("server metadata encoding failed")]
    Protocol(#[from] cords_protocol::ProtocolError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use tower::ServiceExt as _;

    #[test]
    fn identity_survives_restart_and_signs_metadata() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("server.key");
        let (first, created) = ServerIdentity::load_or_create(&path)?;
        let (second, recreated) = ServerIdentity::load_or_create(&path)?;
        assert!(created);
        assert!(!recreated);
        assert_eq!(first.server_id(), second.server_id());
        let signed = second.signed_metadata("Test")?;
        signed.metadata.verify(&signed.signature)?;
        Ok(())
    }

    #[tokio::test]
    async fn discovery_and_health_are_available() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let (identity, _) = ServerIdentity::load_or_create(&directory.path().join("server.key"))?;
        let app = router(AppState::new(identity.signed_metadata("Test")?));
        let health = app
            .clone()
            .oneshot(Request::get("/health/ready").body(Body::empty())?)
            .await?;
        assert_eq!(health.status(), StatusCode::SERVICE_UNAVAILABLE);
        let discovery = app
            .oneshot(Request::get("/.well-known/cords/server").body(Body::empty())?)
            .await?;
        assert_eq!(discovery.status(), StatusCode::OK);
        Ok(())
    }
}
