//! Public Cords protocol v1 types and deterministic encodings.

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use ed25519_dalek::{Signature, VerifyingKey};
use minicbor::Encoder;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;
use thiserror::Error;
use url::Url;

/// The only protocol version implemented during Phase 0.
pub const PROTOCOL_V1: u16 = 1;
/// Domain separation for signed server metadata.
pub const SERVER_METADATA_DOMAIN_V1: &[u8] = b"CORDS-SERVER-METADATA-V1";

/// A normalized HTTPS server origin without path, query, or fragment.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ServerOrigin(String);

impl ServerOrigin {
    /// Parse and normalize an HTTPS origin.
    ///
    /// # Errors
    /// Returns [`ProtocolError::InvalidOrigin`] for anything other than a bare HTTPS origin.
    pub fn parse(input: &str) -> Result<Self, ProtocolError> {
        let url = Url::parse(input).map_err(|_| ProtocolError::InvalidOrigin)?;
        if url.scheme() != "https"
            || url.host().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || (url.path() != "/" && !url.path().is_empty())
        {
            return Err(ProtocolError::InvalidOrigin);
        }
        let host = match url.host() {
            Some(url::Host::Ipv6(host)) => format!("[{host}]"),
            Some(host) => host.to_string(),
            None => return Err(ProtocolError::InvalidOrigin),
        };
        let port = url
            .port()
            .map_or_else(String::new, |value| format!(":{value}"));
        Ok(Self(format!("https://{host}{port}")))
    }

    /// Return the normalized origin.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Construct a URL below the origin.
    ///
    /// # Errors
    /// Returns [`ProtocolError::InvalidOrigin`] if the resulting URL is invalid.
    pub fn join(&self, path: &str) -> Result<Url, ProtocolError> {
        Url::parse(&format!("{}{path}", self.0)).map_err(|_| ProtocolError::InvalidOrigin)
    }
}

impl fmt::Display for ServerOrigin {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Discovery metadata signed by the server identity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ServerMetadataV1 {
    pub protocol_min: u16,
    pub protocol_max: u16,
    pub server_id: String,
    pub server_name: String,
    pub api_base: String,
    pub websocket_path: String,
    pub server_signing_key: String,
    pub join_policy: Vec<String>,
    pub features: Vec<String>,
}

impl ServerMetadataV1 {
    /// Encode this object with the Phase 0 deterministic CBOR profile.
    ///
    /// # Errors
    /// Returns [`ProtocolError::Encoding`] if deterministic encoding fails.
    pub fn signing_bytes(&self) -> Result<Vec<u8>, ProtocolError> {
        let mut bytes = Vec::with_capacity(256);
        bytes.extend_from_slice(SERVER_METADATA_DOMAIN_V1);
        let mut encoder = Encoder::new(&mut bytes);
        encoder
            .array(9)?
            .u16(self.protocol_min)?
            .u16(self.protocol_max)?;
        encoder.str(&self.server_id)?.str(&self.server_name)?;
        encoder.str(&self.api_base)?.str(&self.websocket_path)?;
        encoder.str(&self.server_signing_key)?;
        encode_strings(&mut encoder, &self.join_policy)?;
        encode_strings(&mut encoder, &self.features)?;
        Ok(bytes)
    }

    /// Verify the key fingerprint and Ed25519 signature on a response.
    ///
    /// # Errors
    /// Returns a validation error for an invalid key, fingerprint, encoding, or signature.
    pub fn verify(&self, signature: &str) -> Result<(), ProtocolError> {
        let key_bytes = URL_SAFE_NO_PAD
            .decode(&self.server_signing_key)
            .map_err(|_| ProtocolError::InvalidSigningKey)?;
        let key_array: [u8; 32] = key_bytes
            .try_into()
            .map_err(|_| ProtocolError::InvalidSigningKey)?;
        if server_id_from_public_key(&key_array) != self.server_id {
            return Err(ProtocolError::ServerIdMismatch);
        }
        let verifying_key =
            VerifyingKey::from_bytes(&key_array).map_err(|_| ProtocolError::InvalidSigningKey)?;
        let signature_bytes = URL_SAFE_NO_PAD
            .decode(signature)
            .map_err(|_| ProtocolError::InvalidSignature)?;
        let signature =
            Signature::from_slice(&signature_bytes).map_err(|_| ProtocolError::InvalidSignature)?;
        verifying_key
            .verify_strict(&self.signing_bytes()?, &signature)
            .map_err(|_| ProtocolError::InvalidSignature)
    }
}

fn encode_strings<W: minicbor::encode::Write>(
    encoder: &mut Encoder<W>,
    values: &[String],
) -> Result<(), minicbor::encode::Error<W::Error>> {
    encoder.array(values.len() as u64)?;
    for value in values {
        encoder.str(value)?;
    }
    Ok(())
}

/// JSON response carrying signed discovery metadata.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SignedServerMetadataV1 {
    #[serde(flatten)]
    pub metadata: ServerMetadataV1,
    pub signature: String,
}

/// Protocol and feature support advertised at `/api/v1/capabilities`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CapabilitiesV1 {
    pub protocol_min: u16,
    pub protocol_max: u16,
    pub features: Vec<String>,
}

impl CapabilitiesV1 {
    /// Find the highest common protocol version.
    #[must_use]
    pub fn negotiate(&self, client_min: u16, client_max: u16) -> Option<u16> {
        let minimum = self.protocol_min.max(client_min);
        let maximum = self.protocol_max.min(client_max);
        (minimum <= maximum).then_some(maximum)
    }
}

/// Safe retry classification for public failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetryClass {
    Never,
    Retry,
    Reconnect,
}

/// Stable display-safe public error.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PublicError {
    pub code: String,
    pub message: String,
    pub request_id: String,
    pub retry: RetryClass,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

/// Derive the server identifier from its signing public key.
#[must_use]
pub fn server_id_from_public_key(public_key: &[u8; 32]) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(public_key))
}

/// Phase 0 protocol validation failures.
#[derive(Debug, Error, Eq, PartialEq)]
pub enum ProtocolError {
    #[error("server origin must be an HTTPS origin without credentials, path, query, or fragment")]
    InvalidOrigin,
    #[error("server metadata signing key is invalid")]
    InvalidSigningKey,
    #[error("server metadata signature is invalid")]
    InvalidSignature,
    #[error("server ID does not match the signing key")]
    ServerIdMismatch,
    #[error("deterministic CBOR encoding failed")]
    Encoding,
}

impl<E> From<minicbor::encode::Error<E>> for ProtocolError {
    fn from(_: minicbor::encode::Error<E>) -> Self {
        Self::Encoding
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer as _, SigningKey};

    fn metadata() -> ServerMetadataV1 {
        let key = SigningKey::from_bytes(&[7; 32]);
        let public = key.verifying_key().to_bytes();
        ServerMetadataV1 {
            protocol_min: 1,
            protocol_max: 1,
            server_id: server_id_from_public_key(&public),
            server_name: "Test Cords".into(),
            api_base: "/api/v1".into(),
            websocket_path: "/api/v1/events".into(),
            server_signing_key: URL_SAFE_NO_PAD.encode(public),
            join_policy: vec![],
            features: vec![],
        }
    }

    #[test]
    fn origins_are_strict_and_normalized() {
        assert_eq!(
            ServerOrigin::parse("https://Example.COM:4848/").map(|origin| origin.to_string()),
            Ok("https://example.com:4848".into())
        );
        assert_eq!(
            ServerOrigin::parse("http://example.com"),
            Err(ProtocolError::InvalidOrigin)
        );
        assert_eq!(
            ServerOrigin::parse("https://example.com/path"),
            Err(ProtocolError::InvalidOrigin)
        );
    }

    #[test]
    fn metadata_signature_and_fingerprint_are_bound() -> Result<(), ProtocolError> {
        let key = SigningKey::from_bytes(&[7; 32]);
        let metadata = metadata();
        let signature = URL_SAFE_NO_PAD.encode(key.sign(&metadata.signing_bytes()?).to_bytes());
        assert_eq!(metadata.verify(&signature), Ok(()));

        let mut tampered = metadata;
        tampered.server_name = "Mallory".into();
        assert_eq!(
            tampered.verify(&signature),
            Err(ProtocolError::InvalidSignature)
        );
        Ok(())
    }

    #[test]
    fn deterministic_encoding_is_stable() -> Result<(), ProtocolError> {
        let first = metadata().signing_bytes()?;
        let second = metadata().signing_bytes()?;
        assert_eq!(first, second);
        assert_eq!(
            hex::encode(first),
            "434f5244532d5345525645522d4d455441444154412d5631890101782b5f6f457345764f72544f61735862616177314c3542737362456539442d7a50695575395f3956496d4f496b6a5465737420436f726473672f6170692f76316e2f6170692f76312f6576656e7473782b366b7073592d4b635567712d39564237457937462d5a56486471362d766e755351683771615252473069778080"
        );
        Ok(())
    }

    #[test]
    fn negotiation_selects_highest_overlap() {
        let capabilities = CapabilitiesV1 {
            protocol_min: 1,
            protocol_max: 2,
            features: vec![],
        };
        assert_eq!(capabilities.negotiate(2, 3), Some(2));
        assert_eq!(capabilities.negotiate(3, 4), None);
    }
}
