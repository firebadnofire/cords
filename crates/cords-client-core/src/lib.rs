//! UI-facing Phase 0 client services. No private key material crosses this API.

pub mod accounts;
pub mod client;

use cords_protocol::{PROTOCOL_V1, ServerOrigin, SignedServerMetadataV1};
use serde::Serialize;
use thiserror::Error;

#[derive(Clone, Debug)]
pub struct ServerInspector {
    client: reqwest::Client,
}

impl Default for ServerInspector {
    fn default() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

impl ServerInspector {
    #[must_use]
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }

    /// Fetch and verify signed discovery metadata over trusted HTTPS.
    ///
    /// # Errors
    /// Returns [`InspectError`] for invalid origins, TLS/network failures, invalid metadata, or
    /// incompatible protocol ranges.
    pub async fn inspect(&self, input: &str) -> Result<InspectedServerViewModel, InspectError> {
        let origin = ServerOrigin::parse(input)?;
        let url = origin.join("/.well-known/cords/server")?;
        let response = self.client.get(url).send().await?.error_for_status()?;
        let signed: SignedServerMetadataV1 = response.json().await?;
        signed.metadata.verify(&signed.signature)?;
        let selected_protocol = (signed.metadata.protocol_min <= PROTOCOL_V1
            && signed.metadata.protocol_max >= PROTOCOL_V1)
            .then_some(PROTOCOL_V1)
            .ok_or(InspectError::NoCommonProtocol)?;
        Ok(InspectedServerViewModel {
            origin: origin.to_string(),
            server_id: signed.metadata.server_id,
            server_name: signed.metadata.server_name,
            protocol: selected_protocol,
            features: signed.metadata.features,
            join_policy: signed.metadata.join_policy,
            signature_status: SignatureStatus::Valid,
            trust_status: TrustStatus::NotPinned,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InspectedServerViewModel {
    pub origin: String,
    pub server_id: String,
    pub server_name: String,
    pub protocol: u16,
    pub features: Vec<String>,
    pub join_policy: Vec<String>,
    pub signature_status: SignatureStatus,
    pub trust_status: TrustStatus,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SignatureStatus {
    Valid,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustStatus {
    NotPinned,
}

#[derive(Debug, Error)]
pub enum InspectError {
    #[error("Enter a valid HTTPS Cords server origin without a path.")]
    Protocol(#[from] cords_protocol::ProtocolError),
    #[error("The server could not be reached over trusted HTTPS.")]
    Network(#[from] reqwest::Error),
    #[error("This client and server do not share a protocol version.")]
    NoCommonProtocol,
}
