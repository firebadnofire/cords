//! Versioned identity and encrypted-channel contracts. No private material belongs here.
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use ed25519_dalek::{Signature, VerifyingKey};
use minicbor::Encoder;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

pub const MAX_ENVELOPE: usize = 1024 * 1024;
pub const MAX_TEXT: usize = 64 * 1024;
pub const WS_PROTOCOL: &str = "cords.v1.cbor";

#[derive(Debug, thiserror::Error)]
#[error("invalid signed protocol object")]
pub struct InvalidObject;

pub trait Statement: Serialize {
    const DOMAIN: &'static str;
}

/// RFC 8949 length-first deterministic CBOR for the restricted JSON data model.
/// Only integers, booleans, null, strings, arrays, and string-keyed maps are accepted.
/// # Errors
/// Returns an error if encoding, public-key parsing, or signature validation fails.
pub fn canonical<T: Serialize>(value: &T) -> Result<Vec<u8>, InvalidObject> {
    fn encode(value: &serde_json::Value, e: &mut Encoder<Vec<u8>>) -> Result<(), InvalidObject> {
        use serde_json::Value;
        match value {
            Value::Null => {
                e.null().map_err(|_| InvalidObject)?;
            }
            Value::Bool(v) => {
                e.bool(*v).map_err(|_| InvalidObject)?;
            }
            Value::Number(v) => {
                if let Some(v) = v.as_u64() {
                    e.u64(v).map_err(|_| InvalidObject)?;
                } else if let Some(v) = v.as_i64() {
                    e.i64(v).map_err(|_| InvalidObject)?;
                } else {
                    return Err(InvalidObject);
                }
            }
            Value::String(v) => {
                e.str(v).map_err(|_| InvalidObject)?;
            }
            Value::Array(v) => {
                e.array(v.len() as u64).map_err(|_| InvalidObject)?;
                for item in v {
                    encode(item, e)?;
                }
            }
            Value::Object(v) => {
                let mut fields: Vec<_> = v.iter().collect();
                fields.sort_by(|(a, _), (b, _)| {
                    a.len()
                        .cmp(&b.len())
                        .then_with(|| a.as_bytes().cmp(b.as_bytes()))
                });
                e.map(fields.len() as u64).map_err(|_| InvalidObject)?;
                for (key, value) in fields {
                    e.str(key).map_err(|_| InvalidObject)?;
                    encode(value, e)?;
                }
            }
        }
        Ok(())
    }
    let value = serde_json::to_value(value).map_err(|_| InvalidObject)?;
    let mut encoder = Encoder::new(Vec::new());
    encode(&value, &mut encoder)?;
    Ok(encoder.into_writer())
}

/// # Errors
/// Returns an error if encoding, public-key parsing, or signature validation fails.
pub fn signing_bytes<T: Statement>(value: &T) -> Result<Vec<u8>, InvalidObject> {
    let mut bytes = T::DOMAIN.as_bytes().to_vec();
    bytes.extend(canonical(value)?);
    Ok(bytes)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Signed<T> {
    pub value: T,
    pub signature: String,
}

impl<T: Statement> Signed<T> {
    /// # Errors
    /// Returns an error if encoding, public-key parsing, or signature validation fails.
    pub fn verify(&self, public_key: &str) -> Result<(), InvalidObject> {
        let key: [u8; 32] = decode(public_key)?.try_into().map_err(|_| InvalidObject)?;
        let signature =
            Signature::from_slice(&decode(&self.signature)?).map_err(|_| InvalidObject)?;
        VerifyingKey::from_bytes(&key)
            .map_err(|_| InvalidObject)?
            .verify_strict(&signing_bytes(&self.value)?, &signature)
            .map_err(|_| InvalidObject)
    }
}

pub fn encode(bytes: impl AsRef<[u8]>) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}
/// # Errors
/// Returns an error if encoding, public-key parsing, or signature validation fails.
pub fn decode(value: &str) -> Result<Vec<u8>, InvalidObject> {
    URL_SAFE_NO_PAD.decode(value).map_err(|_| InvalidObject)
}
pub fn hash(bytes: impl AsRef<[u8]>) -> String {
    encode(Sha256::digest(bytes.as_ref()))
}
#[must_use]
pub fn account_id(key: &[u8; 32]) -> String {
    let mut bytes = b"CORDS-ACCOUNT-ID-V1".to_vec();
    bytes.extend(key);
    hash(bytes)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceAuthorization {
    pub version: u16,
    pub account_id: String,
    pub root_public_key: String,
    pub device_id: String,
    pub device_public_key: String,
    pub generation: u64,
    pub created_at: u64,
    pub expires_at: Option<u64>,
    pub previous_record_hash: Option<String>,
    pub capabilities: Vec<String>,
    pub revoked: bool,
}
impl Statement for DeviceAuthorization {
    const DOMAIN: &'static str = "CORDS-DEVICE-AUTHORIZATION-V1";
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceRevocation {
    pub version: u16,
    pub account_id: String,
    pub revoked_device_id: String,
    pub generation: u64,
    pub issued_at: u64,
    pub previous_state_hash: String,
}
impl Statement for DeviceRevocation {
    const DOMAIN: &'static str = "CORDS-DEVICE-REVOCATION-V1";
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RevocationRequest {
    pub record: Signed<DeviceRevocation>,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MlsBinding {
    pub version: u16,
    pub account_id: String,
    pub device_id: String,
    pub authorization_hash: String,
    pub mls_public_key: String,
}
impl Statement for MlsBinding {
    const DOMAIN: &'static str = "CORDS-MLS-BINDING-V1";
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Contact {
    pub authorization: Signed<DeviceAuthorization>,
    pub mls_binding: Signed<MlsBinding>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Challenge {
    pub version: u16,
    pub challenge_id: String,
    pub server_id: String,
    pub account_id: String,
    pub device_id: String,
    pub authorization_hash: String,
    pub purpose: String,
    pub nonce: String,
    pub expires_at: u64,
}
impl Statement for Challenge {
    const DOMAIN: &'static str = "CORDS-DEVICE-CHALLENGE-V1";
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Membership {
    pub version: u16,
    pub server_id: String,
    pub member_id: String,
    pub account_id: String,
    pub device_id: String,
    pub generation: u64,
    pub issued_at: u64,
    pub capabilities: Vec<String>,
    pub status: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MembershipRequest {
    pub account_id: String,
    pub device_id: String,
    pub requested_at: u64,
    pub status: String,
}
impl Statement for Membership {
    const DOMAIN: &'static str = "CORDS-MEMBERSHIP-V1";
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChallengeRequest {
    pub contact: Contact,
    pub purpose: String,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionRequest {
    pub proof: Signed<Challenge>,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnershipState {
    pub version: u16,
    pub server_id: String,
    pub state: String,
    pub generation: u64,
}
impl Statement for OwnershipState {
    const DOMAIN: &'static str = "CORDS-OWNERSHIP-STATE-V1";
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnershipProof {
    pub version: u16,
    pub server_id: String,
    pub account_id: String,
    pub device_id: String,
    pub challenge_hash: String,
}
impl Statement for OwnershipProof {
    const DOMAIN: &'static str = "CORDS-OWNERSHIP-PROOF-V1";
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnershipClaim {
    pub claim_code: String,
    pub contact: Contact,
    pub device_proof: Signed<Challenge>,
    pub root_proof: Signed<OwnershipProof>,
    pub idempotency_key: String,
}
impl std::fmt::Debug for OwnershipClaim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OwnershipClaim")
            .field("claim_code", &"[REDACTED]")
            .field("contact", &self.contact)
            .field("device_proof", &self.device_proof)
            .field("root_proof", &self.root_proof)
            .field("idempotency_key", &self.idempotency_key)
            .finish()
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Session {
    pub token: String,
    pub expires_at: u64,
    pub membership: Signed<Membership>,
}
impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("expires_at", &self.expires_at)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyPackageUpload {
    pub key_package: String,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChannelCreate {
    pub name: String,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Channel {
    pub channel_id: String,
    pub name: String,
    pub creator_device_id: String,
    pub epoch: u64,
    pub members: Vec<Contact>,
    pub binding: Option<Signed<GroupBinding>>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupBinding {
    pub version: u16,
    pub server_id: String,
    pub channel_id: String,
    pub creator_device_id: String,
    pub group_id: String,
}
impl Statement for GroupBinding {
    const DOMAIN: &'static str = "CORDS-CHANNEL-GROUP-V1";
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChannelBind {
    pub binding: Signed<GroupBinding>,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RosterRequest {
    #[serde(default)]
    pub action: RosterAction,
    pub target_device_id: String,
    pub idempotency_key: String,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RosterAction {
    #[default]
    Add,
    Remove,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RosterOperation {
    #[serde(default)]
    pub action: RosterAction,
    pub operation_id: String,
    pub base_epoch: u64,
    pub target: Contact,
    pub key_package: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommitUpload {
    pub operation_id: String,
    pub base_epoch: u64,
    pub commit: String,
    pub welcome: String,
    pub binding: Signed<GroupBinding>,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventUpload {
    pub protocol_version: u16,
    pub event_id: String,
    pub route_kind: String,
    pub route_id: String,
    pub sender_member_id: String,
    pub sender_device_id: String,
    pub client_created_at: u64,
    pub content_encoding: String,
    pub ciphertext: String,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteEvent {
    pub envelope: EventUpload,
    pub server_sequence: u64,
    pub server_received_at: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Message {
    pub schema_version: u16,
    pub message_id: String,
    pub sender_account_id: String,
    pub sender_device_id: String,
    pub client_timestamp: u64,
    pub event_kind: String,
    pub body: String,
}

/// Binary CBOR array [version, kind, route, high-water sequence].
/// # Errors
/// Returns an error if encoding, public-key parsing, or signature validation fails.
pub fn notification(route: &str, sequence: u64) -> Result<Vec<u8>, InvalidObject> {
    canonical(&(1u16, "route.advanced", route, sequence))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ownership_claim_debug_redacts_the_secret() -> Result<(), Box<dyn std::error::Error>> {
        let contact: Contact = serde_json::from_value(serde_json::json!({
            "authorization": {"value": {"version":1,"account_id":"a","root_public_key":"r","device_id":"d","device_public_key":"k","generation":1,"created_at":1,"expires_at":null,"previous_record_hash":null,"capabilities":[],"revoked":false},"signature":"s"},
            "mls_binding": {"value": {"version":1,"account_id":"a","device_id":"d","authorization_hash":"h","mls_public_key":"m"},"signature":"s"}
        }))?;
        let challenge = Challenge {
            version: 1,
            challenge_id: "c".into(),
            server_id: "s".into(),
            account_id: "a".into(),
            device_id: "d".into(),
            authorization_hash: "h".into(),
            purpose: "claim_ownership".into(),
            nonce: "n".into(),
            expires_at: 2,
        };
        let request = OwnershipClaim {
            claim_code: "secret-owner-code-that-must-not-appear".into(),
            contact,
            device_proof: Signed {
                value: challenge.clone(),
                signature: "s".into(),
            },
            root_proof: Signed {
                value: OwnershipProof {
                    version: 1,
                    server_id: "s".into(),
                    account_id: "a".into(),
                    device_id: "d".into(),
                    challenge_hash: hash(canonical(&challenge)?),
                },
                signature: "s".into(),
            },
            idempotency_key: "request-id".into(),
        };
        let debug = format!("{request:?}");
        assert!(!debug.contains(&request.claim_code));
        assert!(debug.contains("[REDACTED]"));
        Ok(())
    }
    #[test]
    fn canonical_vector_and_float_rejection() -> Result<(), InvalidObject> {
        assert_eq!(
            encode(canonical(&serde_json::json!({"b": 2, "a": 1}))?),
            "omFhAWFiAg"
        );
        assert!(canonical(&1.5).is_err());
        assert_eq!(
            notification("r", 3)?,
            [
                132, 1, 110, 114, 111, 117, 116, 101, 46, 97, 100, 118, 97, 110, 99, 101, 100, 97,
                114, 3
            ]
        );
        Ok(())
    }
}
