//! Account presentation is a signed causal graph, separate from device identity.
use crate::messaging::{
    DeviceAuthorization, InvalidObject, Signed, Statement, account_id, canonical, decode, hash,
    signing_bytes,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const FEATURE: &str = "account-profile-history-v1";
pub const MAX_IMAGE: u32 = 5_000_000;
pub const MAX_HEADS: usize = 64;
type FieldWrites = BTreeMap<String, [BTreeSet<String>; 2]>;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileAvatar {
    pub sha256: String,
    pub bytes: u32,
    pub width: u16,
    pub height: u16,
    pub shape: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccountProfile {
    pub display_name: String,
    pub avatar: Option<ProfileAvatar>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileField {
    DisplayName,
    Avatar,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileRevision {
    pub version: u16,
    pub account_id: String,
    pub device_id: String,
    pub authorization_hash: String,
    pub parents: Vec<String>,
    pub generation: u64,
    pub changed: Vec<ProfileField>,
    pub profile: AccountProfile,
}
impl Statement for ProfileRevision {
    const DOMAIN: &'static str = "CORDS-ACCOUNT-PROFILE-REVISION-V1";
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileRecord {
    pub authorization: Signed<DeviceAuthorization>,
    pub revision: Signed<ProfileRevision>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileHistoryPage {
    pub version: u16,
    pub server_id: String,
    pub account_id: String,
    pub records: Vec<ProfileRecord>,
    pub heads: Vec<String>,
    pub next_after: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilePublish {
    pub record: ProfileRecord,
    /// Canonical padded Base64 of JPEG XL, only when this server lacks the hash.
    pub image: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileReceipt {
    pub id: String,
    pub heads: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ProfileGraph {
    pub records: BTreeMap<String, ProfileRecord>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProfileState {
    pub heads: Vec<String>,
    pub pending: Vec<String>,
    pub agreed: Option<AccountProfile>,
    pub merge_candidate: Option<AccountProfile>,
    pub conflicts: Vec<ProfileField>,
}

impl ProfileRecord {
    /// Content identifier is independent of signature spelling and arrival order.
    /// # Errors
    /// Returns an error for unsupported deterministic statement content.
    pub fn id(&self) -> Result<String, InvalidObject> {
        Ok(hash(signing_bytes(&self.revision.value)?))
    }

    /// Verify immutable cryptographic evidence. Current admission, expiration,
    /// revocation and burn checks are performed by the native client/server before
    /// accepting a new publication; old accepted evidence remains verifiable.
    /// # Errors
    /// Rejects substitutions, invalid signatures, unbounded fields or old formats.
    pub fn verify(&self) -> Result<(), InvalidObject> {
        let a = &self.authorization.value;
        let r = &self.revision.value;
        let root: [u8; 32] = decode(&a.root_public_key)?
            .try_into()
            .map_err(|_| InvalidObject)?;
        if a.version != 1
            || a.revoked
            || a.generation == 0
            || account_id(&root) != a.account_id
            || a.root_public_key == a.device_public_key
            || !a
                .capabilities
                .iter()
                .any(|value| value == "device.authenticate")
            || uuid::Uuid::parse_str(&a.device_id).is_err()
            || r.version != 1
            || r.account_id != a.account_id
            || r.device_id != a.device_id
            || r.authorization_hash != hash(canonical(&self.authorization)?)
            || r.parents.len() > MAX_HEADS
            || r.parents.windows(2).any(|pair| pair[0] >= pair[1])
            || r.parents.iter().any(|parent| !valid_hash(parent))
            || r.changed.len() > 2
            || r.changed.windows(2).any(|pair| pair[0] >= pair[1])
            || r.profile.display_name.trim().is_empty()
            || r.profile.display_name.chars().count() > 100
            || r.profile.display_name.chars().any(char::is_control)
        {
            return Err(InvalidObject);
        }
        if let Some(avatar) = &r.profile.avatar
            && (!valid_hash(&avatar.sha256)
                || avatar.bytes == 0
                || avatar.bytes > MAX_IMAGE
                || avatar.width == 0
                || avatar.width > 1024
                || avatar.height == 0
                || avatar.height > 1024
                || !matches!(avatar.shape.as_str(), "circle" | "square"))
        {
            return Err(InvalidObject);
        }
        self.authorization.verify(&a.root_public_key)?;
        self.revision.verify(&a.device_public_key)
    }
}

fn valid_hash(value: &str) -> bool {
    value.len() == 43
        && decode(value)
            .is_ok_and(|bytes| bytes.len() == 32 && crate::messaging::encode(bytes) == value)
}

impl ProfileGraph {
    /// Declare only writes differing from causally maximal inherited values.
    /// # Errors
    /// Missing or invalid ancestry cannot be signed over.
    pub fn changed_fields(
        &self,
        profile: &AccountProfile,
    ) -> Result<Vec<ProfileField>, InvalidObject> {
        let (writes, pending) = self.validated_writes()?;
        if !pending.is_empty() {
            return Err(InvalidObject);
        }
        let state = self.state()?;
        let mut changed = Vec::new();
        for (index, field) in [ProfileField::DisplayName, ProfileField::Avatar]
            .iter()
            .enumerate()
        {
            let inherited = self.maximal(
                &state
                    .heads
                    .iter()
                    .flat_map(|head| writes[head][index].iter().cloned())
                    .collect(),
            );
            if inherited.is_empty()
                || inherited.iter().any(|id| {
                    !equal_field(&self.records[id].revision.value.profile, profile, *field)
                })
            {
                changed.push(*field);
            }
        }
        Ok(changed)
    }
    /// Insert authenticated evidence without discarding competing branches.
    /// # Errors
    /// Rejects invalid signatures, mixed accounts and content substitution.
    pub fn insert(&mut self, record: ProfileRecord) -> Result<String, InvalidObject> {
        record.verify()?;
        if self.records.values().next().is_some_and(|known| {
            known.revision.value.account_id != record.revision.value.account_id
        }) {
            return Err(InvalidObject);
        }
        let id = record.id()?;
        if let Some(known) = self.records.get(&id) {
            if canonical(&known.revision.value)? != canonical(&record.revision.value)? {
                return Err(InvalidObject);
            }
        } else {
            self.records.insert(id.clone(), record);
        }
        Ok(id)
    }

    fn ancestor(&self, ancestor: &str, descendant: &str) -> bool {
        let mut stack = vec![descendant];
        let mut seen = BTreeSet::new();
        while let Some(id) = stack.pop() {
            if id == ancestor {
                return true;
            }
            if !seen.insert(id) {
                continue;
            }
            if let Some(record) = self.records.get(id) {
                stack.extend(record.revision.value.parents.iter().map(String::as_str));
            }
        }
        false
    }

    fn maximal(&self, ids: &BTreeSet<String>) -> BTreeSet<String> {
        ids.iter()
            .filter(|id| {
                !ids.iter()
                    .any(|other| other != *id && self.ancestor(id, other))
            })
            .cloned()
            .collect()
    }

    /// Validate ancestry and causal field writes; missing ancestry stays pending.
    /// Generation orders validation only and never chooses a profile winner.
    /// # Errors
    /// Rejects invalid causal generation, fabricated unchanged fields or overwide graphs.
    fn validated_writes(&self) -> Result<(FieldWrites, Vec<String>), InvalidObject> {
        let mut ordered: Vec<_> = self.records.iter().collect();
        ordered.sort_by_key(|(_, record)| record.revision.value.generation);
        let mut writes: BTreeMap<String, [BTreeSet<String>; 2]> = BTreeMap::new();
        let mut pending = Vec::new();
        for (id, record) in &ordered {
            let r = &record.revision.value;
            if r.parents.contains(id)
                || r.parents.iter().any(|parent| {
                    self.records
                        .get(parent)
                        .is_some_and(|parent| parent.revision.value.generation >= r.generation)
                })
            {
                return Err(InvalidObject);
            }
            if r.parents.iter().any(|parent| !writes.contains_key(parent)) {
                pending.push((*id).clone());
                continue;
            }
            let generation = if r.parents.is_empty() {
                0
            } else {
                r.parents
                    .iter()
                    .map(|parent| self.records[parent].revision.value.generation)
                    .max()
                    .ok_or(InvalidObject)?
                    .checked_add(1)
                    .ok_or(InvalidObject)?
            };
            if generation != r.generation {
                return Err(InvalidObject);
            }
            let mut current: [BTreeSet<String>; 2] = Default::default();
            for (index, field) in [ProfileField::DisplayName, ProfileField::Avatar]
                .iter()
                .enumerate()
            {
                let inherited = self.maximal(
                    &r.parents
                        .iter()
                        .flat_map(|parent| writes[parent][index].iter().cloned())
                        .collect(),
                );
                if r.changed.contains(field) {
                    if !inherited.is_empty()
                        && inherited.iter().all(|parent| {
                            equal_field(
                                &self.records[parent].revision.value.profile,
                                &r.profile,
                                *field,
                            )
                        })
                    {
                        return Err(InvalidObject);
                    }
                    current[index].insert((*id).clone());
                } else {
                    if inherited.is_empty()
                        || inherited.iter().any(|parent| {
                            !equal_field(
                                &self.records[parent].revision.value.profile,
                                &r.profile,
                                *field,
                            )
                        })
                    {
                        return Err(InvalidObject);
                    }
                    current[index] = inherited;
                }
            }
            writes.insert((*id).clone(), current);
        }
        Ok((writes, pending))
    }

    /// Compute presentation only from complete, causally validated records.
    /// # Errors
    /// Returns an error for invalid ancestry or field declarations.
    pub fn state(&self) -> Result<ProfileState, InvalidObject> {
        let (writes, pending) = self.validated_writes()?;
        let mut ordered: Vec<_> = self.records.iter().collect();
        ordered.sort_by_key(|(_, record)| record.revision.value.generation);
        // Incomplete descendants must not hide their complete ancestors.
        let parents: BTreeSet<_> = writes
            .keys()
            .flat_map(|id| self.records[id].revision.value.parents.iter())
            .collect();
        let heads: Vec<_> = writes
            .keys()
            .filter(|id| !parents.contains(id))
            .cloned()
            .collect();
        if heads.len() > MAX_HEADS {
            return Err(InvalidObject);
        }
        let mut agreed = None;
        for (_, candidate) in ordered.iter().rev() {
            let id = candidate.id()?;
            if writes.contains_key(&id) && heads.iter().all(|head| self.ancestor(&id, head)) {
                // An arbitrary tie is harmless only when every common candidate
                // at this generation has identical content. Otherwise go older.
                let peers: Vec<_> = ordered
                    .iter()
                    .filter(|(other_id, other)| {
                        writes.contains_key(*other_id)
                            && other.revision.value.generation
                                == candidate.revision.value.generation
                            && heads.iter().all(|head| self.ancestor(other_id, head))
                    })
                    .collect();
                if peers.iter().all(|(_, other)| {
                    other.revision.value.profile == candidate.revision.value.profile
                }) {
                    agreed = Some(candidate.revision.value.profile.clone());
                    break;
                }
            }
        }
        let mut conflicts = Vec::new();
        let mut merged = agreed.clone().or_else(|| {
            heads
                .first()
                .map(|id| self.records[id].revision.value.profile.clone())
        });
        for (index, field) in [ProfileField::DisplayName, ProfileField::Avatar]
            .iter()
            .enumerate()
        {
            let maximal = self.maximal(
                &heads
                    .iter()
                    .flat_map(|head| writes[head][index].iter().cloned())
                    .collect(),
            );
            if let Some(first) = maximal.first() {
                let value = &self.records[first].revision.value.profile;
                if maximal
                    .iter()
                    .any(|id| !equal_field(value, &self.records[id].revision.value.profile, *field))
                {
                    conflicts.push(*field);
                } else if let Some(merged) = &mut merged {
                    match field {
                        ProfileField::DisplayName => {
                            merged.display_name.clone_from(&value.display_name);
                        }
                        ProfileField::Avatar => merged.avatar.clone_from(&value.avatar),
                    }
                }
            }
        }
        if !conflicts.is_empty() || !pending.is_empty() {
            merged = None;
        }
        Ok(ProfileState {
            heads,
            pending,
            agreed,
            merge_candidate: merged,
            conflicts,
        })
    }
}

fn equal_field(left: &AccountProfile, right: &AccountProfile, field: ProfileField) -> bool {
    match field {
        ProfileField::DisplayName => left.display_name == right.display_name,
        ProfileField::Avatar => left.avatar == right.avatar,
    }
}

#[cfg(test)]
mod tests;
