//! Native account and device authority. Secret exports are for encrypted native storage only.
#![forbid(unsafe_code)]
use cords_protocol::messaging::{
    Contact, DeviceAuthorization, DeviceRevocation, InvalidObject, MlsBinding, Signed, Statement,
    account_id, canonical, decode, encode, hash, signing_bytes,
};
use ed25519_dalek::{Signer as _, SigningKey};
use rand_core::OsRng;
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

/// Creation timestamps are metadata, not delayed-activation instructions.
/// Bound ordinary clock skew without extending any expiration deadline.
pub const STATEMENT_CLOCK_SKEW_SECONDS: u64 = 5;

pub struct Identity {
    root: SigningKey,
    device: SigningKey,
    pub authorization: Signed<DeviceAuthorization>,
}
impl std::fmt::Debug for Identity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Identity")
            .field("device_id", &self.authorization.value.device_id)
            .finish_non_exhaustive()
    }
}
#[derive(Serialize, Deserialize)]
struct StoredIdentity {
    root: [u8; 32],
    device: [u8; 32],
    authorization: Signed<DeviceAuthorization>,
}
impl Drop for StoredIdentity {
    fn drop(&mut self) {
        use zeroize::Zeroize as _;
        self.root.zeroize();
        self.device.zeroize();
    }
}

/// # Errors
/// Returns an error for invalid identity material, authorization, signatures, or state continuity.
pub fn sign<T: Statement>(key: &SigningKey, value: T) -> Result<Signed<T>, InvalidObject> {
    let signature = encode(key.sign(&signing_bytes(&value)?).to_bytes());
    Ok(Signed { value, signature })
}

impl Identity {
    /// Authorize removal of this installation's device with its account root.
    /// # Errors
    /// Returns an error on generation overflow or invalid signing encoding.
    pub fn revoke(&self, now: u64) -> Result<Signed<DeviceRevocation>, InvalidObject> {
        sign(
            &self.root,
            DeviceRevocation {
                version: 1,
                account_id: self.authorization.value.account_id.clone(),
                revoked_device_id: self.authorization.value.device_id.clone(),
                generation: self
                    .authorization
                    .value
                    .generation
                    .checked_add(1)
                    .ok_or(InvalidObject)?,
                issued_at: now,
                previous_state_hash: hash(canonical(&self.authorization)?),
            },
        )
    }
    /// # Errors
    /// Returns an error for invalid identity material, authorization, signatures, or state continuity.
    pub fn generate(now: u64) -> Result<Self, InvalidObject> {
        let root = SigningKey::generate(&mut OsRng);
        let device = SigningKey::generate(&mut OsRng);
        let authorization = sign(
            &root,
            DeviceAuthorization {
                version: 1,
                account_id: account_id(&root.verifying_key().to_bytes()),
                root_public_key: encode(root.verifying_key().to_bytes()),
                device_id: uuid::Uuid::now_v7().to_string(),
                device_public_key: encode(device.verifying_key().to_bytes()),
                generation: 1,
                created_at: now,
                expires_at: None,
                previous_record_hash: None,
                capabilities: vec!["device.authenticate".into(), "mls.bind".into()],
                revoked: false,
            },
        )?;
        Ok(Self {
            root,
            device,
            authorization,
        })
    }
    /// # Errors
    /// Returns an error for invalid identity material, authorization, signatures, or state continuity.
    pub fn sign_device<T: Statement>(&self, value: T) -> Result<Signed<T>, InvalidObject> {
        sign(&self.device, value)
    }
    /// # Errors
    /// Returns an error for invalid identity material, authorization, signatures, or state continuity.
    pub fn contact(&self, mls_public_key: &[u8]) -> Result<Contact, InvalidObject> {
        Ok(Contact {
            authorization: self.authorization.clone(),
            mls_binding: self.sign_device(MlsBinding {
                version: 1,
                account_id: self.authorization.value.account_id.clone(),
                device_id: self.authorization.value.device_id.clone(),
                authorization_hash: hash(canonical(&self.authorization)?),
                mls_public_key: encode(mls_public_key),
            })?,
        })
    }
    /// # Errors
    /// Returns an error for invalid identity material, authorization, signatures, or state continuity.
    pub fn export_secret(&self) -> Result<Zeroizing<Vec<u8>>, InvalidObject> {
        Ok(Zeroizing::new(
            serde_json::to_vec(&StoredIdentity {
                root: self.root.to_bytes(),
                device: self.device.to_bytes(),
                authorization: self.authorization.clone(),
            })
            .map_err(|_| InvalidObject)?,
        ))
    }
    /// # Errors
    /// Returns an error for invalid identity material, authorization, signatures, or state continuity.
    pub fn import_secret(bytes: &[u8], now: u64) -> Result<Self, InvalidObject> {
        let stored: StoredIdentity = serde_json::from_slice(bytes).map_err(|_| InvalidObject)?;
        validate_authorization(&stored.authorization, now)?;
        let root = SigningKey::from_bytes(&stored.root);
        let device = SigningKey::from_bytes(&stored.device);
        if encode(root.verifying_key().to_bytes()) != stored.authorization.value.root_public_key
            || encode(device.verifying_key().to_bytes())
                != stored.authorization.value.device_public_key
        {
            return Err(InvalidObject);
        }
        Ok(Self {
            root,
            device,
            authorization: stored.authorization.clone(),
        })
    }
}

/// # Errors
/// Returns an error for invalid identity material, authorization, signatures, or state continuity.
pub fn validate_authorization(
    record: &Signed<DeviceAuthorization>,
    now: u64,
) -> Result<(), InvalidObject> {
    let v = &record.value;
    if v.version != 1
        || v.generation == 0
        || v.revoked
        || v.created_at > now.saturating_add(STATEMENT_CLOCK_SKEW_SECONDS)
        || v.expires_at.is_some_and(|expiry| expiry <= now)
        || !v.capabilities.iter().any(|v| v == "device.authenticate")
        || uuid::Uuid::parse_str(&v.device_id).is_err()
    {
        return Err(InvalidObject);
    }
    let key: [u8; 32] = decode(&v.root_public_key)?
        .try_into()
        .map_err(|_| InvalidObject)?;
    if account_id(&key) != v.account_id || v.root_public_key == v.device_public_key {
        return Err(InvalidObject);
    }
    let device: [u8; 32] = decode(&v.device_public_key)?
        .try_into()
        .map_err(|_| InvalidObject)?;
    ed25519_dalek::VerifyingKey::from_bytes(&device).map_err(|_| InvalidObject)?;
    record.verify(&v.root_public_key)
}

/// # Errors
/// Returns an error for invalid identity material, authorization, signatures, or state continuity.
pub fn validate_contact(contact: &Contact, now: u64) -> Result<(), InvalidObject> {
    validate_authorization(&contact.authorization, now)?;
    let a = &contact.authorization.value;
    let b = &contact.mls_binding.value;
    if b.version != 1
        || a.account_id != b.account_id
        || a.device_id != b.device_id
        || b.authorization_hash != hash(canonical(&contact.authorization)?)
        || b.mls_public_key == a.device_public_key
        || b.mls_public_key == a.root_public_key
        || !a.capabilities.iter().any(|v| v == "mls.bind")
        || decode(&b.mls_public_key)?.len() != 32
    {
        return Err(InvalidObject);
    }
    contact.mls_binding.verify(&a.device_public_key)
}

/// Validate continuity against the newest persisted account authorization.
/// # Errors
/// Returns an error for invalid identity material, authorization, signatures, or state continuity.
pub fn validate_transition(
    previous: &Signed<DeviceAuthorization>,
    next: &Signed<DeviceAuthorization>,
) -> Result<(), InvalidObject> {
    let a = &previous.value;
    let b = &next.value;
    if a.account_id != b.account_id || a.root_public_key != b.root_public_key {
        return Err(InvalidObject);
    }
    next.verify(&a.root_public_key)?;
    if b.generation == a.generation {
        return if canonical(previous)? == canonical(next)? {
            Ok(())
        } else {
            Err(InvalidObject)
        };
    }
    if b.generation != a.generation.checked_add(1).ok_or(InvalidObject)?
        || b.previous_record_hash.as_deref() != Some(hash(canonical(previous)?).as_str())
    {
        return Err(InvalidObject);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independent_roles_persist_and_reject_substitution() -> Result<(), InvalidObject> {
        let a = Identity::generate(10)?;
        let b = Identity::generate(10)?;
        assert_ne!(
            a.authorization.value.account_id,
            b.authorization.value.account_id
        );
        assert_ne!(
            a.authorization.value.device_id,
            b.authorization.value.device_id
        );
        let mls = SigningKey::generate(&mut OsRng);
        let mut contact = a.contact(&mls.verifying_key().to_bytes())?;
        validate_contact(&contact, 10)?;
        let restored = Identity::import_secret(&a.export_secret()?, 11)?;
        assert_eq!(restored.authorization.signature, a.authorization.signature);
        contact.authorization = b.authorization;
        assert!(validate_contact(&contact, 11).is_err());
        assert!(validate_contact(&a.contact(&a.device.verifying_key().to_bytes())?, 11).is_err());
        Ok(())
    }
    #[test]
    fn generation_tamper_and_expiry_are_rejected() -> Result<(), InvalidObject> {
        let a = Identity::generate(10)?;
        let mut changed = a.authorization.clone();
        changed.value.generation = 2;
        assert!(validate_authorization(&changed, 11).is_err());
        assert!(validate_transition(&a.authorization, &changed).is_err());
        changed.value.previous_record_hash = Some(hash(canonical(&a.authorization)?));
        changed = sign(&a.root, changed.value)?;
        validate_transition(&a.authorization, &changed)?;
        assert!(validate_transition(&changed, &a.authorization).is_err());
        changed.value.expires_at = Some(11);
        changed = sign(&a.root, changed.value)?;
        assert!(validate_authorization(&changed, 11).is_err());
        Ok(())
    }
    #[test]
    fn creation_clock_skew_is_bounded_without_expiry_grace() -> Result<(), InvalidObject> {
        let a = Identity::generate(100)?;
        validate_authorization(&a.authorization, 95)?;
        assert!(validate_authorization(&a.authorization, 94).is_err());
        let mut expiring = a.authorization.value.clone();
        expiring.expires_at = Some(101);
        let expiring = sign(&a.root, expiring)?;
        validate_authorization(&expiring, 100)?;
        assert!(validate_authorization(&expiring, 101).is_err());
        Ok(())
    }
}
