//! Execute existing public vectors; this is not a model of future root/report state.

use cords_protocol::messaging::{
    Challenge, DeviceAuthorization, DeviceRevocation, GroupBinding, Membership, MlsBinding, Signed,
    Statement, account_id, canonical, decode, hash, signing_bytes,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixture {
    account_id: String,
    description: String,
    vectors: Vec<Vector>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Vector {
    domain: String,
    public_key: String,
    signed: serde_json::Value,
    signing_bytes_hex: String,
}

// Deliberately different existing domain. Retagging the identical payload must fail.
#[derive(Serialize)]
#[serde(transparent)]
struct Retagged<T>(T);

impl<T: Serialize> Statement for Retagged<T> {
    const DOMAIN: &'static str = "CORDS-SERVER-DEPARTURE-V1";
}

fn fixture() -> Result<Fixture, serde_json::Error> {
    serde_json::from_str(include_str!("../../../test-vectors/identity-v1.json"))
}

fn verify_vector<T: Statement + DeserializeOwned>(
    vector: &Vector,
    other_key: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let signed: Signed<T> = serde_json::from_value(vector.signed.clone())?;
    assert_eq!(vector.domain, T::DOMAIN);
    assert_eq!(
        hex::encode(signing_bytes(&signed.value)?),
        vector.signing_bytes_hex
    );
    signed.verify(&vector.public_key)?;
    assert!(signed.verify(other_key).is_err(), "substituted signing key");

    let mut changed = vector.signed.clone();
    changed["value"]["version"] = serde_json::json!(2);
    let changed: Signed<T> = serde_json::from_value(changed)?;
    assert!(
        changed.verify(&vector.public_key).is_err(),
        "old signature accepted for changed version"
    );

    let retagged = Signed {
        value: Retagged(signed.value),
        signature: signed.signature,
    };
    assert!(
        retagged.verify(&vector.public_key).is_err(),
        "old signature accepted in a different protocol domain"
    );
    Ok(())
}

#[test]
fn existing_identity_wire_bytes_signatures_and_domains_remain_compatible()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = fixture()?;
    assert!(fixture.description.contains("test-only"));
    assert_eq!(fixture.vectors.len(), 6);
    for vector in &fixture.vectors {
        let other_key = &fixture
            .vectors
            .iter()
            .find(|other| other.public_key != vector.public_key)
            .ok_or("fixture needs an independent public key")?
            .public_key;
        match vector.domain.as_str() {
            DeviceAuthorization::DOMAIN => verify_vector::<DeviceAuthorization>(vector, other_key)?,
            MlsBinding::DOMAIN => verify_vector::<MlsBinding>(vector, other_key)?,
            Challenge::DOMAIN => verify_vector::<Challenge>(vector, other_key)?,
            Membership::DOMAIN => verify_vector::<Membership>(vector, other_key)?,
            GroupBinding::DOMAIN => verify_vector::<GroupBinding>(vector, other_key)?,
            DeviceRevocation::DOMAIN => verify_vector::<DeviceRevocation>(vector, other_key)?,
            domain => return Err(format!("unhandled baseline vector domain {domain}").into()),
        }
    }
    Ok(())
}

#[test]
fn legacy_authorization_binding_and_revocation_preserve_exact_record_hashes()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = fixture()?;
    let find = |domain: &str| -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        Ok(fixture
            .vectors
            .iter()
            .find(|vector| vector.domain == domain)
            .ok_or("missing baseline domain")?
            .signed
            .clone())
    };
    let authorization: Signed<DeviceAuthorization> =
        serde_json::from_value(find(DeviceAuthorization::DOMAIN)?)?;
    let binding: Signed<MlsBinding> = serde_json::from_value(find(MlsBinding::DOMAIN)?)?;
    let revocation: Signed<DeviceRevocation> =
        serde_json::from_value(find(DeviceRevocation::DOMAIN)?)?;
    let root: [u8; 32] = decode(&authorization.value.root_public_key)?
        .try_into()
        .map_err(|_| "invalid root length")?;
    assert_eq!(account_id(&root), fixture.account_id);
    let record_hash = hash(canonical(&authorization)?);
    assert_eq!(binding.value.authorization_hash, record_hash);
    assert_eq!(revocation.value.previous_state_hash, record_hash);
    assert_eq!(binding.value.account_id, fixture.account_id);
    assert_eq!(revocation.value.account_id, fixture.account_id);
    assert_eq!(binding.value.device_id, authorization.value.device_id);
    assert_eq!(revocation.value.revoked_device_id, binding.value.device_id);
    assert!(revocation.value.generation > authorization.value.generation);
    Ok(())
}
