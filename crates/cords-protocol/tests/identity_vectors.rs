use cords_protocol::messaging::{
    Challenge, DeviceAuthorization, DeviceRevocation, GroupBinding, Membership, MlsBinding, Signed,
    Statement, account_id, decode, signing_bytes,
};
use serde::de::DeserializeOwned;
use serde_json::Value;

fn verify<T: Statement + DeserializeOwned>(
    vector: &Value,
) -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(vector["domain"], T::DOMAIN);
    let signed: Signed<T> = serde_json::from_value(vector["signed"].clone())?;
    assert_eq!(
        hex::encode(signing_bytes(&signed.value)?),
        vector["signing_bytes_hex"]
    );
    signed.verify(vector["public_key"].as_str().ok_or("missing key")?)?;
    let mut tampered = vector["signed"].clone();
    tampered["value"]["version"] = Value::from(2);
    let changed: Signed<T> = serde_json::from_value(tampered)?;
    assert!(
        changed
            .verify(vector["public_key"].as_str().ok_or("missing key")?)
            .is_err()
    );
    Ok(())
}

#[test]
fn signed_identity_protocol_vectors() -> Result<(), Box<dyn std::error::Error>> {
    let fixture: Value =
        serde_json::from_str(include_str!("../../../test-vectors/identity-v1.json"))?;
    let vectors = fixture["vectors"].as_array().ok_or("missing vectors")?;
    assert_eq!(vectors.len(), 6);
    verify::<DeviceAuthorization>(&vectors[0])?;
    verify::<MlsBinding>(&vectors[1])?;
    verify::<Challenge>(&vectors[2])?;
    verify::<Membership>(&vectors[3])?;
    verify::<GroupBinding>(&vectors[4])?;
    verify::<DeviceRevocation>(&vectors[5])?;
    let root: [u8; 32] = decode(vectors[0]["public_key"].as_str().ok_or("missing root")?)?
        .try_into()
        .map_err(|_| "invalid root length")?;
    assert_eq!(account_id(&root), fixture["account_id"]);
    Ok(())
}
