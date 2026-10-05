//! `OpenMLS` objects stay in this module. Snapshots must only enter authenticated storage.
use ::tls_codec::{Deserialize as _, Serialize as _};
use openmls::prelude::*;
use openmls_basic_credential::SignatureKeyPair;
use openmls_rust_crypto::OpenMlsRustCrypto;
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

const SUITE: Ciphersuite = Ciphersuite::MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519;

#[derive(Debug, thiserror::Error)]
#[error("MLS operation rejected or persistent state is invalid")]
pub struct CryptoError;
fn checked<T, E>(value: Result<T, E>) -> Result<T, CryptoError> {
    value.map_err(|_| CryptoError)
}

pub struct ConversationCrypto {
    provider: OpenMlsRustCrypto,
    signer: SignatureKeyPair,
    device_id: String,
}
impl std::fmt::Debug for ConversationCrypto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConversationCrypto")
            .field("device_id", &self.device_id)
            .finish_non_exhaustive()
    }
}

#[derive(Serialize, Deserialize)]
struct Snapshot {
    version: u16,
    device_id: String,
    signer: SignatureKeyPair,
    records: Vec<(Vec<u8>, Vec<u8>)>,
}

#[derive(Debug)]
pub struct Commit {
    pub commit: Vec<u8>,
    pub welcome: Vec<u8>,
}
#[derive(Debug)]
pub enum Processed {
    Application {
        device_id: String,
        plaintext: Zeroizing<Vec<u8>>,
    },
    Commit,
}

impl ConversationCrypto {
    /// # Errors
    /// Returns an error if MLS rejects the operation or the stored group state is invalid.
    pub fn new(device_id: String) -> Result<Self, CryptoError> {
        Ok(Self {
            provider: OpenMlsRustCrypto::default(),
            signer: checked(SignatureKeyPair::new(SUITE.signature_algorithm()))?,
            device_id,
        })
    }
    pub fn public_key(&self) -> Vec<u8> {
        self.signer.to_public_vec()
    }
    fn credential(&self) -> CredentialWithKey {
        CredentialWithKey {
            credential: BasicCredential::new(self.device_id.as_bytes().to_vec()).into(),
            signature_key: self.signer.to_public_vec().into(),
        }
    }
    /// # Errors
    /// Returns an error if MLS rejects the operation or the stored group state is invalid.
    pub fn snapshot(&self) -> Result<Zeroizing<Vec<u8>>, CryptoError> {
        #[derive(Serialize)]
        struct Borrowed<'a> {
            version: u16,
            device_id: &'a str,
            signer: &'a SignatureKeyPair,
            records: &'a [(Vec<u8>, Vec<u8>)],
        }
        // Serialize a borrowed signer; do not require clonable secret-key features.
        let mut records: Vec<_> = checked(self.provider.storage().values.read())?
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        records.sort_by(|a, b| a.0.cmp(&b.0));
        let records = Zeroizing::new(records);
        let output = checked(serde_json::to_vec(&Borrowed {
            version: 1,
            device_id: &self.device_id,
            signer: &self.signer,
            records: &records,
        }))?;
        Ok(Zeroizing::new(output))
    }
    /// # Errors
    /// Returns an error if MLS rejects the operation or the stored group state is invalid.
    pub fn restore(bytes: &[u8]) -> Result<Self, CryptoError> {
        let snapshot: Snapshot = checked(serde_json::from_slice(bytes))?;
        if snapshot.version != 1 {
            return Err(CryptoError);
        }
        let provider = OpenMlsRustCrypto::default();
        checked(provider.storage().values.write())?.extend(snapshot.records);
        Ok(Self {
            provider,
            signer: snapshot.signer,
            device_id: snapshot.device_id,
        })
    }
    /// # Errors
    /// Returns an error if MLS rejects the operation or the stored group state is invalid.
    pub fn key_package(&self) -> Result<Vec<u8>, CryptoError> {
        let bundle = checked(KeyPackage::builder().build(
            SUITE,
            &self.provider,
            &self.signer,
            self.credential(),
        ))?;
        checked(bundle.key_package().tls_serialize_detached())
    }
    /// # Errors
    /// Returns an error if MLS rejects the operation or the stored group state is invalid.
    pub fn validate_key_package(bytes: &[u8], device: &str, key: &[u8]) -> Result<(), CryptoError> {
        let provider = OpenMlsRustCrypto::default();
        let incoming = checked(KeyPackageIn::tls_deserialize_exact(bytes))?;
        let package = checked(incoming.validate(provider.crypto(), ProtocolVersion::Mls10))?;
        if package.ciphersuite() != SUITE
            || package.leaf_node().credential().serialized_content() != device.as_bytes()
            || package.leaf_node().signature_key().as_slice() != key
        {
            return Err(CryptoError);
        }
        Ok(())
    }
    /// # Errors
    /// Returns an error if MLS rejects the operation or the stored group state is invalid.
    pub fn create(&self, route: &str) -> Result<(), CryptoError> {
        if checked(MlsGroup::load(
            self.provider.storage(),
            &GroupId::from_slice(route.as_bytes()),
        ))?
        .is_some()
        {
            return Err(CryptoError);
        }
        let config = MlsGroupCreateConfig::builder()
            .ciphersuite(SUITE)
            .use_ratchet_tree_extension(true)
            .max_past_epochs(0)
            .build();
        checked(MlsGroup::new_with_group_id(
            &self.provider,
            &self.signer,
            &config,
            GroupId::from_slice(route.as_bytes()),
            self.credential(),
        ))?;
        Ok(())
    }
    fn group(&self, route: &str) -> Result<MlsGroup, CryptoError> {
        checked(MlsGroup::load(
            self.provider.storage(),
            &GroupId::from_slice(route.as_bytes()),
        ))?
        .ok_or(CryptoError)
    }
    /// # Errors
    /// Returns an error if MLS rejects the operation or the stored group state is invalid.
    pub fn epoch(&self, route: &str) -> Result<u64, CryptoError> {
        Ok(self.group(route)?.epoch().as_u64())
    }
    /// # Errors
    /// Returns an error if MLS rejects the operation or the stored group state is invalid.
    pub fn add(&self, route: &str, package: &[u8]) -> Result<Commit, CryptoError> {
        let incoming = checked(KeyPackageIn::tls_deserialize_exact(package))?;
        let package = checked(incoming.validate(self.provider.crypto(), ProtocolVersion::Mls10))?;
        if package.ciphersuite() != SUITE {
            return Err(CryptoError);
        }
        let mut group = self.group(route)?;
        let (commit, welcome, _) =
            checked(group.add_members(&self.provider, &self.signer, &[package]))?;
        Ok(Commit {
            commit: checked(commit.tls_serialize_detached())?,
            welcome: checked(welcome.tls_serialize_detached())?,
        })
    }
    /// # Errors
    /// Returns an error if MLS rejects the operation or the stored group state is invalid.
    pub fn merge_pending(&self, route: &str) -> Result<(), CryptoError> {
        checked(self.group(route)?.merge_pending_commit(&self.provider))
    }
    /// Stage cryptographic removal; persist and obtain relay acceptance before merging.
    /// # Errors
    /// Returns an error if the device is absent or MLS rejects the removal.
    pub fn remove(&self, route: &str, device: &str) -> Result<Vec<u8>, CryptoError> {
        let mut group = self.group(route)?;
        let member = group
            .members()
            .find(|member| member.credential.serialized_content() == device.as_bytes())
            .ok_or(CryptoError)?;
        let (commit, _, _) =
            checked(group.remove_members(&self.provider, &self.signer, &[member.index]))?;
        checked(commit.tls_serialize_detached())
    }
    /// # Errors
    /// Returns an error if MLS rejects the operation or the stored group state is invalid.
    pub fn join(&self, route: &str, welcome: &[u8]) -> Result<(), CryptoError> {
        if checked(MlsGroup::load(
            self.provider.storage(),
            &GroupId::from_slice(route.as_bytes()),
        ))?
        .is_some()
        {
            return Err(CryptoError);
        }
        let message = checked(MlsMessageIn::tls_deserialize_exact(welcome))?;
        let MlsMessageBodyIn::Welcome(welcome) = message.extract() else {
            return Err(CryptoError);
        };
        let config = MlsGroupJoinConfig::builder()
            .use_ratchet_tree_extension(true)
            .max_past_epochs(0)
            .build();
        let staged = checked(StagedWelcome::new_from_welcome(
            &self.provider,
            &config,
            welcome,
            None,
        ))?;
        let group = checked(staged.into_group(&self.provider))?;
        if group.group_id().as_slice() != route.as_bytes() {
            return Err(CryptoError);
        }
        Ok(())
    }
    /// # Errors
    /// Returns an error if MLS rejects the operation or the stored group state is invalid.
    pub fn encrypt(&self, route: &str, plaintext: &[u8]) -> Result<Vec<u8>, CryptoError> {
        let message = checked(self.group(route)?.create_message(
            &self.provider,
            &self.signer,
            plaintext,
        ))?;
        checked(message.tls_serialize_detached())
    }
    /// # Errors
    /// Returns an error if MLS rejects the operation or the stored group state is invalid.
    pub fn process(&self, route: &str, bytes: &[u8]) -> Result<Processed, CryptoError> {
        let incoming = checked(MlsMessageIn::tls_deserialize_exact(bytes))?;
        let protocol = checked(incoming.try_into_protocol_message())?;
        let mut group = self.group(route)?;
        let processed = checked(group.process_message(&self.provider, protocol))?;
        let device_id = checked(String::from_utf8(
            processed.credential().serialized_content().to_vec(),
        ))?;
        match processed.into_content() {
            ProcessedMessageContent::ApplicationMessage(message) => Ok(Processed::Application {
                device_id,
                plaintext: Zeroizing::new(message.into_bytes()),
            }),
            ProcessedMessageContent::StagedCommitMessage(commit) => {
                checked(group.merge_staged_commit(&self.provider, *commit))?;
                Ok(Processed::Commit)
            }
            _ => Err(CryptoError),
        }
    }
    /// # Errors
    /// Returns an error if MLS rejects the operation or the stored group state is invalid.
    pub fn roster(&self, route: &str) -> Result<Vec<(String, Vec<u8>)>, CryptoError> {
        self.group(route)?
            .members()
            .map(|member| {
                Ok((
                    checked(String::from_utf8(
                        member.credential.serialized_content().to_vec(),
                    ))?,
                    member.signature_key,
                ))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independent_persistent_members_exchange() -> Result<(), CryptoError> {
        let a = ConversationCrypto::new("a".into())?;
        let b = ConversationCrypto::new("b".into())?;
        assert_ne!(a.public_key(), b.public_key());
        let package = b.key_package()?;
        ConversationCrypto::validate_key_package(&package, "b", &b.public_key())?;
        assert!(ConversationCrypto::validate_key_package(&package, "a", &a.public_key()).is_err());
        a.create("channel")?;
        let commit = a.add("channel", &package)?;
        a.merge_pending("channel")?;
        b.join("channel", &commit.welcome)?;
        let bytes = a.encrypt("channel", b"hello")?;
        let Processed::Application {
            device_id,
            plaintext,
        } = b.process("channel", &bytes)?
        else {
            return Err(CryptoError);
        };
        assert_eq!(device_id, "a");
        assert_eq!(&**plaintext, b"hello");
        let a = ConversationCrypto::restore(&a.snapshot()?)?;
        let b = ConversationCrypto::restore(&b.snapshot()?)?;
        assert!(b.process("channel", &bytes).is_err());
        let bytes = b.encrypt("channel", b"after restart")?;
        assert!(matches!(
            a.process("channel", &bytes)?,
            Processed::Application { .. }
        ));
        assert_eq!(a.epoch("channel")?, 1);
        let removal = a.remove("channel", "b")?;
        a.merge_pending("channel")?;
        assert!(matches!(b.process("channel", &removal)?, Processed::Commit));
        assert!(
            b.process("channel", &a.encrypt("channel", b"excluded")?)
                .is_err()
        );
        Ok(())
    }
}
