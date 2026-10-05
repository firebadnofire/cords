//! Regenerate public compatibility vectors. Fixed seeds below are TEST ONLY.
use cords_protocol::messaging::{
    Challenge, DeviceAuthorization, DeviceRevocation, GroupBinding, Membership, MlsBinding, Signed,
    Statement, account_id, canonical, encode, hash, signing_bytes,
};
use ed25519_dalek::{Signer as _, SigningKey};
use serde_json::{Value, json};

fn vector<T: Statement>(key: &SigningKey, value: T) -> Result<Value, Box<dyn std::error::Error>> {
    let bytes = signing_bytes(&value)?;
    let signed = Signed {
        value,
        signature: encode(key.sign(&bytes).to_bytes()),
    };
    Ok(
        json!({"domain": T::DOMAIN, "public_key": encode(key.verifying_key().to_bytes()),
        "signing_bytes_hex": hex::encode(bytes), "signed": signed}),
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = SigningKey::from_bytes(&[1; 32]);
    let device = SigningKey::from_bytes(&[2; 32]);
    let mls = SigningKey::from_bytes(&[3; 32]);
    let server = SigningKey::from_bytes(&[4; 32]);
    let account = account_id(&root.verifying_key().to_bytes());
    let device_id = "00000000-0000-4000-8000-000000000001";
    let server_id = "vector-server";
    let authorization = vector(
        &root,
        DeviceAuthorization {
            version: 1,
            account_id: account.clone(),
            root_public_key: encode(root.verifying_key().to_bytes()),
            device_id: device_id.into(),
            device_public_key: encode(device.verifying_key().to_bytes()),
            generation: 1,
            created_at: 1_800_000_000,
            expires_at: None,
            previous_record_hash: None,
            capabilities: vec!["device.authenticate".into(), "mls.bind".into()],
            revoked: false,
        },
    )?;
    let authorization_hash = hash(canonical(&authorization["signed"])?);
    let binding = vector(
        &device,
        MlsBinding {
            version: 1,
            account_id: account.clone(),
            device_id: device_id.into(),
            authorization_hash: authorization_hash.clone(),
            mls_public_key: encode(mls.verifying_key().to_bytes()),
        },
    )?;
    let challenge = vector(
        &device,
        Challenge {
            version: 1,
            challenge_id: "00000000-0000-4000-8000-000000000002".into(),
            server_id: server_id.into(),
            account_id: account.clone(),
            device_id: device_id.into(),
            authorization_hash: authorization_hash.clone(),
            purpose: "join".into(),
            nonce: encode([5; 32]),
            expires_at: 1_800_000_060,
        },
    )?;
    let membership = vector(
        &server,
        Membership {
            version: 1,
            server_id: server_id.into(),
            member_id: "00000000-0000-4000-8000-000000000003".into(),
            account_id: account.clone(),
            device_id: device_id.into(),
            generation: 1,
            issued_at: 1_800_000_001,
            capabilities: vec!["channel.read".into(), "channel.write".into()],
            status: "active".into(),
        },
    )?;
    let group = vector(
        &device,
        GroupBinding {
            version: 1,
            server_id: server_id.into(),
            channel_id: "channel-vector".into(),
            creator_device_id: device_id.into(),
            group_id: "channel-vector".into(),
        },
    )?;
    let revocation = vector(
        &root,
        DeviceRevocation {
            version: 1,
            account_id: account.clone(),
            revoked_device_id: device_id.into(),
            generation: 2,
            issued_at: 1_800_000_100,
            previous_state_hash: authorization_hash,
        },
    )?;
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "description": "Public V1 vectors; seeds [1;32] through [4;32] are test-only, never deployment keys",
            "account_id": account,
            "vectors": [authorization, binding, challenge, membership, group, revocation],
        }))?
    );
    Ok(())
}
