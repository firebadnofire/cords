use super::*;
use crate::messaging::{encode, signing_bytes};
use ed25519_dalek::{Signer as _, SigningKey};

fn signed<T: Statement>(key: &SigningKey, value: T) -> Signed<T> {
    let signature = encode(
        key.sign(
            &signing_bytes(&value).unwrap_or_else(|error| panic!("test encoding failed: {error}")),
        )
        .to_bytes(),
    );
    Signed { value, signature }
}

fn record(
    parents: Vec<String>,
    generation: u64,
    name: &str,
    avatar: Option<ProfileAvatar>,
    changed: Vec<ProfileField>,
    device: u8,
) -> ProfileRecord {
    let root = SigningKey::from_bytes(&[1; 32]);
    let key = SigningKey::from_bytes(&[device; 32]);
    let account = account_id(&root.verifying_key().to_bytes());
    let authorization = signed(
        &root,
        DeviceAuthorization {
            version: 1,
            account_id: account.clone(),
            root_public_key: encode(root.verifying_key().as_bytes()),
            device_id: format!("00000000-0000-4000-8000-0000000000{device:02}"),
            device_public_key: encode(key.verifying_key().as_bytes()),
            generation: 1,
            created_at: 1,
            expires_at: None,
            previous_record_hash: None,
            capabilities: vec!["device.authenticate".into()],
            revoked: false,
        },
    );
    let mut parents = parents;
    parents.sort();
    let revision = signed(
        &key,
        ProfileRevision {
            version: 1,
            account_id: account,
            device_id: authorization.value.device_id.clone(),
            authorization_hash: hash(
                canonical(&authorization)
                    .unwrap_or_else(|error| panic!("test authorization encoding failed: {error}")),
            ),
            parents,
            generation,
            changed,
            profile: AccountProfile {
                display_name: name.into(),
                avatar,
            },
        },
    );
    ProfileRecord {
        authorization,
        revision,
    }
}
fn all() -> Vec<ProfileField> {
    vec![ProfileField::DisplayName, ProfileField::Avatar]
}
fn avatar() -> ProfileAvatar {
    ProfileAvatar {
        sha256: hash(b"image"),
        bytes: 100,
        width: 64,
        height: 64,
        shape: "circle".into(),
    }
}

#[test]
fn independent_fields_require_a_signed_merge_and_never_choose_a_branch() -> Result<(), InvalidObject>
{
    let mut graph = ProfileGraph::default();
    let base = graph.insert(record(vec![], 0, "Base", None, all(), 2))?;
    let left = graph.insert(record(
        vec![base.clone()],
        1,
        "New name",
        None,
        vec![ProfileField::DisplayName],
        2,
    ))?;
    let right = graph.insert(record(
        vec![base],
        1,
        "Base",
        Some(avatar()),
        vec![ProfileField::Avatar],
        3,
    ))?;
    let state = graph.state()?;
    assert_eq!(
        state.agreed.as_ref().ok_or(InvalidObject)?.display_name,
        "Base"
    );
    assert!(state.conflicts.is_empty());
    let candidate = state.merge_candidate.ok_or(InvalidObject)?;
    assert_eq!(candidate.display_name, "New name");
    assert_eq!(candidate.avatar, Some(avatar()));
    assert!(graph.changed_fields(&candidate)?.is_empty());
    graph.insert(record(
        vec![left, right],
        2,
        &candidate.display_name,
        candidate.avatar,
        vec![],
        3,
    ))?;
    let state = graph.state()?;
    assert_eq!(state.heads.len(), 1);
    assert_eq!(state.agreed.ok_or(InvalidObject)?.display_name, "New name");
    Ok(())
}

#[test]
fn conflicting_names_preserve_last_agreement_until_explicit_resolution() -> Result<(), InvalidObject>
{
    let mut graph = ProfileGraph::default();
    let base = graph.insert(record(vec![], 0, "Agreed", None, all(), 2))?;
    let a = record(
        vec![base.clone()],
        1,
        "A",
        None,
        vec![ProfileField::DisplayName],
        2,
    );
    let b = record(vec![base], 1, "B", None, vec![ProfileField::DisplayName], 3);
    let mut reverse = graph.clone();
    graph.insert(a.clone())?;
    graph.insert(b.clone())?;
    reverse.insert(b.clone())?;
    reverse.insert(a.clone())?;
    assert_eq!(canonical(&graph.state()?)?, canonical(&reverse.state()?)?);
    let state = graph.state()?;
    assert_eq!(state.agreed.ok_or(InvalidObject)?.display_name, "Agreed");
    assert_eq!(state.conflicts, vec![ProfileField::DisplayName]);
    assert!(state.merge_candidate.is_none());
    let invalid = record(state.heads.clone(), 2, "A", None, vec![], 2);
    let mut bad = graph.clone();
    bad.insert(invalid)?;
    assert!(bad.state().is_err());
    graph.insert(record(
        state.heads,
        2,
        "B",
        None,
        vec![ProfileField::DisplayName],
        3,
    ))?;
    assert_eq!(
        graph.state()?.agreed.ok_or(InvalidObject)?.display_name,
        "B"
    );
    // Replaying old accepted evidence cannot replace the resolved profile.
    graph.insert(a)?;
    graph.insert(b)?;
    assert_eq!(
        graph.state()?.agreed.ok_or(InvalidObject)?.display_name,
        "B"
    );
    Ok(())
}

#[test]
fn identical_concurrent_values_agree_and_missing_ancestry_stays_pending()
-> Result<(), InvalidObject> {
    let mut graph = ProfileGraph::default();
    let genesis = record(vec![], 0, "Base", None, all(), 2);
    let base = genesis.id()?;
    let left = record(
        vec![base.clone()],
        1,
        "Same",
        None,
        vec![ProfileField::DisplayName],
        2,
    );
    let right = record(
        vec![base],
        1,
        "Same",
        None,
        vec![ProfileField::DisplayName],
        3,
    );
    graph.insert(left.clone())?;
    assert_eq!(graph.state()?.pending.len(), 1);
    assert!(graph.state()?.agreed.is_none());
    graph.insert(genesis)?;
    graph.insert(right)?;
    let state = graph.state()?;
    assert!(state.conflicts.is_empty());
    assert_eq!(
        state.merge_candidate.ok_or(InvalidObject)?.display_name,
        "Same"
    );
    graph.insert(record(state.heads, 2, "Same", None, vec![], 3))?;
    assert_eq!(
        graph.state()?.agreed.ok_or(InvalidObject)?.display_name,
        "Same"
    );
    Ok(())
}

#[test]
fn unsigned_tampering_wrong_generations_and_unlisted_field_changes_fail()
-> Result<(), InvalidObject> {
    let genesis = record(vec![], 0, "Base", None, all(), 2);
    let mut tampered = genesis.clone();
    tampered.revision.value.profile.display_name = "Forgery".into();
    assert!(tampered.verify().is_err());
    let mut graph = ProfileGraph::default();
    let base = graph.insert(genesis)?;
    let wrong = record(
        vec![base.clone()],
        9,
        "Wrong",
        None,
        vec![ProfileField::DisplayName],
        2,
    );
    let mut invalid = graph.clone();
    invalid.insert(wrong)?;
    assert!(invalid.state().is_err());
    graph.insert(record(vec![base], 1, "Unlisted", None, vec![], 2))?;
    assert!(graph.state().is_err());
    Ok(())
}

#[test]
fn revision_domain_vector() -> Result<(), InvalidObject> {
    let genesis = record(vec![], 0, "Base", None, all(), 2);
    genesis.verify()?;
    assert_eq!(genesis.id()?, "ICzyputMRtzmNdZ_ONJzTNH64_6eY5LhroDh2HRvCuI");
    assert_eq!(
        hex::encode(signing_bytes(&genesis.revision.value)?),
        "434f5244532d4143434f554e542d50524f46494c452d5245564953494f4e2d5631a8676368616e676564826c646973706c61795f6e616d656661766174617267706172656e7473806770726f66696c65a266617661746172f66c646973706c61795f6e616d6564426173656776657273696f6e01696465766963655f6964782430303030303030302d303030302d343030302d383030302d3030303030303030303030326a6163636f756e745f6964782b39677777425a584441615964534167617a47467a59333761683635465f6d426b78706d626745494a7036516a67656e65726174696f6e0072617574686f72697a6174696f6e5f68617368782b41385f773261455f5132393737424d764f776247716d3479336d5474415f415531626532327563787a5649"
    );
    assert_eq!(genesis.revision.value.generation, 0);
    Ok(())
}
