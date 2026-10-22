//! Role-policy values. These are not additions to legacy signed membership objects.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const DEFAULT_ROLE: &str = "builtin:default";
pub const MODERATOR_ROLE: &str = "builtin:moderator";
pub const ADMIN_ROLE: &str = "builtin:admin";
pub const MAX_ROLES: usize = 256;
pub const ROLE_AUDIT_PAGE_SIZE: usize = 50;

/// Negotiated separately from legacy signed membership capabilities.
pub const ENFORCEMENT_FEATURE: &str = "enforced-roles-v1";

/// Subtractive server policy evidence, never an account/device revocation or an
/// authorization to add a participant. The pinned server owns channel access.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChannelAccessRemoval {
    pub version: u16,
    pub server_id: String,
    pub channel_id: String,
    pub account_id: String,
    pub device_id: String,
    pub policy_revision: u64,
    pub channel_epoch: u64,
    pub issued_at: u64,
}
impl crate::messaging::Statement for ChannelAccessRemoval {
    const DOMAIN: &'static str = "CORDS-CHANNEL-ACCESS-REMOVAL-V1";
}

/// Current server actions are presentation hints, not reusable credentials.
/// Channel operations and role targets need their own current scoped decision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectiveActions {
    pub version: u16,
    pub active: bool,
    pub channel_id: Option<String>,
    pub server_id: String,
    pub account_id: String,
    pub device_id: String,
    pub policy_revision: u64,
    pub permissions: BTreeSet<Permission>,
    pub owner: bool,
}
impl crate::messaging::Statement for EffectiveActions {
    const DOMAIN: &'static str = "CORDS-EFFECTIVE-ACTIONS-V1";
}

/// Owner-only metadata for a fenced encrypted route. It contains no roster,
/// messages or secrets and cannot authorize a commit in the predecessor group.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChannelRecoveryCandidate {
    pub version: u16,
    pub server_id: String,
    pub account_id: String,
    pub device_id: String,
    pub predecessor: crate::messaging::Signed<crate::messaging::ChannelIdentity>,
    pub policy_revision: u64,
    pub channel_epoch: u64,
    pub pending_digest: String,
    pub committer_available: bool,
}
impl crate::messaging::Statement for ChannelRecoveryCandidate {
    const DOMAIN: &'static str = "CORDS-CHANNEL-RECOVERY-CANDIDATE-V1";
}

/// Explicit device intent to abandon a fenced group for the exact signed
/// succession proposal. Separate domain; no legacy V1 object is reinterpreted.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChannelRemovalRecovery {
    pub version: u16,
    pub server_id: String,
    pub account_id: String,
    pub device_id: String,
    pub channel_id: String,
    pub policy_revision: u64,
    pub channel_epoch: u64,
    pub pending_digest: String,
    pub succession_digest: String,
}
impl crate::messaging::Statement for ChannelRemovalRecovery {
    const DOMAIN: &'static str = "CORDS-CHANNEL-REMOVAL-RECOVERY-V1";
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoleAuditEntry {
    pub revision: u64,
    pub actor_device_id: String,
    pub operation: String,
    pub target: String,
    /// Database transaction timestamp; revision, not wall clock, defines ordering.
    pub committed_at: u64,
}

/// Bounded administrative metadata, not a claim of tamper-proof server history.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoleAuditPage {
    pub version: u16,
    pub server_id: String,
    pub after_revision: u64,
    pub head_revision: u64,
    pub entries: Vec<RoleAuditEntry>,
}
impl crate::messaging::Statement for RoleAuditPage {
    const DOMAIN: &'static str = "CORDS-ROLE-AUDIT-PAGE-V1";
}

/// Separate endpoint objects; never decoded as legacy membership or message events.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoleCatalog {
    pub version: u16,
    pub server_id: String,
    pub revision: u64,
    pub active: bool,
    pub roles: Vec<Role>,
}
impl crate::messaging::Statement for RoleCatalog {
    const DOMAIN: &'static str = "CORDS-ROLE-CATALOG-V1";
}

/// Account assignments and catalog from one committed policy revision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccountRoles {
    pub account_id: String,
    pub catalog: RoleCatalog,
    pub role_ids: BTreeSet<String>,
}
impl crate::messaging::Statement for AccountRoles {
    const DOMAIN: &'static str = "CORDS-ACCOUNT-ROLES-V1";
}

/// Current catalog presentation and definition actions for this exact actor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoleWorkspace {
    pub version: u16,
    pub server_id: String,
    pub account_id: String,
    pub device_id: String,
    pub catalog: RoleCatalog,
    pub counts: BTreeMap<String, u64>,
    pub editable: BTreeSet<String>,
    pub deletable: BTreeSet<String>,
    pub can_create: bool,
}
impl crate::messaging::Statement for RoleWorkspace {
    const DOMAIN: &'static str = "CORDS-ROLE-WORKSPACE-V1";
}

/// Target-specific presentation hints. The server rechecks all mutation authority.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoleTargetActions {
    pub version: u16,
    pub server_id: String,
    pub account_id: String,
    pub device_id: String,
    pub target_account_id: String,
    pub catalog: RoleCatalog,
    pub role_ids: BTreeSet<String>,
    pub assignable: BTreeSet<String>,
    pub removable: BTreeSet<String>,
    pub eligible: bool,
}
impl crate::messaging::Statement for RoleTargetActions {
    const DOMAIN: &'static str = "CORDS-ROLE-TARGET-ACTIONS-V1";
}

pub const ROLE_MEMBERS_PAGE_SIZE: usize = 16;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoleMember {
    pub account_id: String,
    pub assigned: bool,
    pub can_assign: bool,
    pub can_remove: bool,
}

/// Bounded account presentation; pagination is not a frozen membership snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoleMembersPage {
    pub version: u16,
    pub server_id: String,
    pub account_id: String,
    pub device_id: String,
    pub catalog: RoleCatalog,
    pub role_id: String,
    pub assigned: bool,
    pub after: String,
    pub next_after: Option<String>,
    pub member_count: u64,
    pub members: Vec<RoleMember>,
}
impl crate::messaging::Statement for RoleMembersPage {
    const DOMAIN: &'static str = "CORDS-ROLE-MEMBERS-PAGE-V1";
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum RoleChange {
    Initialize,
    PutRole { role: Role },
    DeleteRole { role_id: String },
    AssignRole { account_id: String, role_id: String },
    RevokeRole { account_id: String, role_id: String },
    ReorderRoles { positions: Vec<RolePosition> },
}

/// Complete catalog positions, highest priority first in presentation. Request
/// order grants no authority; each previous/proposed rank is checked separately.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RolePosition {
    pub role_id: String,
    pub priority: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoleChangeRequest {
    pub expected_revision: i64,
    pub request_id: String,
    pub change: RoleChange,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoleChangeReceipt {
    pub version: u16,
    pub server_id: String,
    pub revision: u64,
    pub request_id: String,
    pub request_digest: String,
    pub actor_device_id: String,
}
impl crate::messaging::Statement for RoleChangeReceipt {
    const DOMAIN: &'static str = "CORDS-ROLE-CHANGE-RECEIPT-V1";
}

/// Stable capability strings; labels and `$role` display names never grant authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Permission {
    #[serde(rename = "server.read")]
    ViewServer,
    #[serde(rename = "server.manage")]
    ManageServer,
    #[serde(rename = "server.roles.manage")]
    ManageRoles,
    #[serde(rename = "server.roles.assign")]
    AssignRoles,
    #[serde(rename = "server.invite.create")]
    CreateInvites,
    #[serde(rename = "server.invite.manage")]
    ManageInvites,
    #[serde(rename = "server.member.approve")]
    ApproveAccounts,
    #[serde(rename = "server.device.approve")]
    ApproveDevices,
    #[serde(rename = "server.audit.read")]
    ViewAudit,
    #[serde(rename = "channel.view")]
    ViewChannel,
    #[serde(rename = "channel.read")]
    ReadMessages,
    #[serde(rename = "channel.write")]
    SendMessages,
    #[serde(rename = "channel.create")]
    CreateChannels,
    #[serde(rename = "channel.manage")]
    ManageChannel,
    #[serde(rename = "channel.retire")]
    RetireChannel,
    #[serde(rename = "channel.permissions.manage")]
    ManageChannelPermissions,
    #[serde(rename = "channel.members.manage")]
    ManageChannelMembers,
    #[serde(rename = "channel.mls.commit")]
    CommitMls,
    #[serde(rename = "keypackage.publish")]
    PublishKeyPackage,
    #[serde(rename = "server.member.kick")]
    KickMembers,
    #[serde(rename = "server.member.ban")]
    BanMembers,
    #[serde(rename = "server.member.timeout")]
    TimeoutMembers,
    #[serde(rename = "server.moderation.read")]
    ViewModeration,
    #[serde(rename = "channel.messages.manage")]
    ManageMessages,
    #[serde(rename = "channel.emojis.manage")]
    ManageEmojis,
    #[serde(rename = "mention_roles")]
    MentionRoles,
    #[serde(rename = "server.roles.mentionability")]
    ManageMentionability,
}

impl Permission {
    /// Only these capabilities may be overridden on a resource.
    #[must_use]
    pub const fn channel_scoped(self) -> bool {
        matches!(
            self,
            Self::ViewChannel
                | Self::ReadMessages
                | Self::SendMessages
                | Self::ManageChannel
                | Self::RetireChannel
                | Self::ManageChannelPermissions
                | Self::ManageChannelMembers
                | Self::CommitMls
                | Self::ManageMessages
                | Self::ManageEmojis
        )
    }

    /// Basic participation only; `$default` cannot turn every account into an administrator.
    #[must_use]
    pub const fn safe_for_default(self) -> bool {
        matches!(
            self,
            Self::ViewServer
                | Self::ViewChannel
                | Self::ReadMessages
                | Self::SendMessages
                | Self::CommitMls
                | Self::PublishKeyPackage
                | Self::MentionRoles
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Role {
    pub id: String,
    /// Stored without the `$` presentation prefix.
    pub name: String,
    pub color: String,
    pub description: String,
    pub priority: u32,
    pub permissions: BTreeSet<Permission>,
    pub mentionable: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Override {
    pub allow: BTreeSet<Permission>,
    pub deny: BTreeSet<Permission>,
}

pub type Overrides = BTreeMap<String, Override>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecurityPreset {
    Community,
    PrivateGroup,
    Lockdown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceAdmission {
    AccountAuthorized,
    AdministratorApproval,
}

impl SecurityPreset {
    #[must_use]
    pub const fn device_admission(self) -> DeviceAdmission {
        match self {
            Self::Community | Self::PrivateGroup => DeviceAdmission::AccountAuthorized,
            Self::Lockdown => DeviceAdmission::AdministratorApproval,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messaging::{Signed, Statement, canonical, encode, signing_bytes};
    use ed25519_dalek::{Signer as _, SigningKey};

    #[test]
    #[allow(clippy::too_many_lines)] // One complete fixed-vector and field-binding contract.
    fn recovery_domains_fixed_cbor_digests_and_all_field_binding()
    -> Result<(), Box<dyn std::error::Error>> {
        use crate::messaging::{ChannelIdentity, ConfidentialityMode, hash};
        let recovery = ChannelRemovalRecovery {
            version: 1,
            server_id: "server".into(),
            account_id: "account".into(),
            device_id: "device".into(),
            channel_id: "channel".into(),
            policy_revision: 3,
            channel_epoch: 2,
            pending_digest: "pending".into(),
            succession_digest: "succession".into(),
        };
        let candidate = ChannelRecoveryCandidate {
            version: 1,
            server_id: "server".into(),
            account_id: "account".into(),
            device_id: "device".into(),
            predecessor: Signed {
                signature: "predecessor-signature".into(),
                value: ChannelIdentity {
                    version: 1,
                    server_id: "server".into(),
                    channel_id: "channel".into(),
                    name: "original".into(),
                    creator_device_id: "creator".into(),
                    confidentiality_mode: ConfidentialityMode::Encrypted,
                    created_at: 7,
                },
            },
            policy_revision: 3,
            channel_epoch: 2,
            pending_digest: "pending".into(),
            committer_available: false,
        };
        // Independent minimal canonical-CBOR encoder + SHA-256 reference values.
        assert_eq!(
            hash(canonical(&recovery)?),
            "l7P2w9esmEaR7ZA6pCQ6-y_0w8hkjZ7ScAvKjDl-xF8"
        );
        assert_eq!(
            hash(canonical(&candidate)?),
            "wCo9pa93zsYIkylz08iASfEtDzEldtOBsU_Ay7eghCw"
        );
        assert_ne!(
            ChannelRemovalRecovery::DOMAIN,
            crate::messaging::ChannelSuccession::DOMAIN
        );
        assert_ne!(
            ChannelRemovalRecovery::DOMAIN,
            ChannelRecoveryCandidate::DOMAIN
        );
        let key = SigningKey::from_bytes(&[0x57; 32]);
        let public = encode(key.verifying_key().as_bytes());
        let signed = Signed {
            signature: encode(key.sign(&signing_bytes(&recovery)?).to_bytes()),
            value: recovery,
        };
        signed.verify(&public)?;
        for field in [
            "version",
            "server_id",
            "account_id",
            "device_id",
            "channel_id",
            "policy_revision",
            "channel_epoch",
            "pending_digest",
            "succession_digest",
        ] {
            let mut value = serde_json::to_value(&signed.value)?;
            value[field] = if value[field].is_string() {
                serde_json::json!("other")
            } else {
                serde_json::json!(99)
            };
            let altered = Signed::<ChannelRemovalRecovery> {
                value: serde_json::from_value(value)?,
                signature: signed.signature.clone(),
            };
            assert!(
                altered.verify(&public).is_err(),
                "unbound recovery field {field}"
            );
        }
        let signed = Signed {
            signature: encode(key.sign(&signing_bytes(&candidate)?).to_bytes()),
            value: candidate,
        };
        signed.verify(&public)?;
        for field in [
            "version",
            "server_id",
            "account_id",
            "device_id",
            "predecessor",
            "policy_revision",
            "channel_epoch",
            "pending_digest",
            "committer_available",
        ] {
            let mut value = serde_json::to_value(&signed.value)?;
            match field {
                "predecessor" => value[field]["value"]["channel_id"] = serde_json::json!("other"),
                "committer_available" => value[field] = serde_json::json!(true),
                _ => {
                    value[field] = if value[field].is_string() {
                        serde_json::json!("other")
                    } else {
                        serde_json::json!(99)
                    }
                }
            }
            let altered = Signed::<ChannelRecoveryCandidate> {
                value: serde_json::from_value(value)?,
                signature: signed.signature.clone(),
            };
            assert!(
                altered.verify(&public).is_err(),
                "unbound candidate field {field}"
            );
        }
        Ok(())
    }

    #[test]
    #[allow(clippy::too_many_lines)] // Fixed signing vectors and complete substitution coverage.
    fn enforced_actions_and_removal_have_separate_fixed_signing_contracts()
    -> Result<(), Box<dyn std::error::Error>> {
        let removal = ChannelAccessRemoval {
            version: 1,
            server_id: "server".into(),
            channel_id: "channel".into(),
            account_id: "account".into(),
            device_id: "device".into(),
            policy_revision: 3,
            channel_epoch: 2,
            issued_at: 9,
        };
        let actions = EffectiveActions {
            version: 1,
            active: true,
            channel_id: None,
            server_id: "server".into(),
            account_id: "account".into(),
            device_id: "device".into(),
            policy_revision: 3,
            permissions: [Permission::ViewServer].into(),
            owner: false,
        };
        assert_eq!(
            encode(canonical(&removal)?),
            "qGd2ZXJzaW9uAWlkZXZpY2VfaWRmZGV2aWNlaWlzc3VlZF9hdAlpc2VydmVyX2lkZnNlcnZlcmphY2NvdW50X2lkZ2FjY291bnRqY2hhbm5lbF9pZGdjaGFubmVsbWNoYW5uZWxfZXBvY2gCb3BvbGljeV9yZXZpc2lvbgM"
        );
        assert_eq!(
            encode(canonical(&actions)?),
            "qWVvd25lcvRmYWN0aXZl9Wd2ZXJzaW9uAWlkZXZpY2VfaWRmZGV2aWNlaXNlcnZlcl9pZGZzZXJ2ZXJqYWNjb3VudF9pZGdhY2NvdW50amNoYW5uZWxfaWT2a3Blcm1pc3Npb25zgWtzZXJ2ZXIucmVhZG9wb2xpY3lfcmV2aXNpb24D"
        );
        assert_ne!(ChannelAccessRemoval::DOMAIN, EffectiveActions::DOMAIN);
        assert_ne!(
            EffectiveActions::DOMAIN,
            crate::messaging::Membership::DOMAIN
        );
        let key = SigningKey::from_bytes(&[0x57; 32]);
        let public = encode(key.verifying_key().as_bytes());
        let signed = Signed {
            signature: encode(key.sign(&signing_bytes(&removal)?).to_bytes()),
            value: removal,
        };
        signed.verify(&public)?;
        for field in [
            "version",
            "server_id",
            "channel_id",
            "account_id",
            "device_id",
            "policy_revision",
            "channel_epoch",
            "issued_at",
        ] {
            let mut value = serde_json::to_value(&signed.value)?;
            value[field] = if value[field].is_string() {
                serde_json::json!("other")
            } else {
                serde_json::json!(99)
            };
            let substituted = Signed::<ChannelAccessRemoval> {
                value: serde_json::from_value(value)?,
                signature: signed.signature.clone(),
            };
            assert!(
                substituted.verify(&public).is_err(),
                "unbound removal field {field}"
            );
        }
        let signed = Signed {
            signature: encode(key.sign(&signing_bytes(&actions)?).to_bytes()),
            value: actions,
        };
        signed.verify(&public)?;
        for field in [
            "version",
            "server_id",
            "account_id",
            "device_id",
            "policy_revision",
            "permissions",
            "owner",
            "active",
            "channel_id",
        ] {
            let mut value = serde_json::to_value(&signed.value)?;
            value[field] = match field {
                "permissions" => serde_json::json!(["server.manage"]),
                "owner" => serde_json::json!(true),
                "active" => serde_json::json!(false),
                "channel_id" => serde_json::json!("other"),
                _ if value[field].is_string() => serde_json::json!("other"),
                _ => serde_json::json!(99),
            };
            let substituted = Signed::<EffectiveActions> {
                value: serde_json::from_value(value)?,
                signature: signed.signature.clone(),
            };
            assert!(
                substituted.verify(&public).is_err(),
                "unbound action field {field}"
            );
        }
        Ok(())
    }

    #[test]
    fn catalog_cbor_vector_and_signature_binding() -> Result<(), Box<dyn std::error::Error>> {
        let catalog = RoleCatalog {
            version: 1,
            server_id: "server".into(),
            revision: 0,
            active: false,
            roles: Vec::new(),
        };
        assert_eq!(canonical(&catalog)?, b"\xa5\x65roles\x80\x66active\xf4\x67version\x01\x68revision\x00\x69server_id\x66server");
        assert_eq!(RoleCatalog::DOMAIN, "CORDS-ROLE-CATALOG-V1");
        assert_ne!(RoleCatalog::DOMAIN, RoleChangeReceipt::DOMAIN);
        assert_ne!(RoleCatalog::DOMAIN, crate::messaging::Membership::DOMAIN);
        let key = SigningKey::from_bytes(&[0x57; 32]);
        let public = encode(key.verifying_key().as_bytes());
        let mut signed = Signed {
            signature: encode(key.sign(&signing_bytes(&catalog)?).to_bytes()),
            value: catalog,
        };
        signed.verify(&public)?;
        signed.value.active = true;
        assert!(signed.verify(&public).is_err());
        signed.value.active = false;
        signed.value.revision = 1;
        assert!(signed.verify(&public).is_err());
        signed.value.revision = 0;
        signed.value.server_id = "another-server".into();
        assert!(signed.verify(&public).is_err());
        Ok(())
    }

    #[test]
    fn account_roles_bind_account_catalog_and_assignments() -> Result<(), Box<dyn std::error::Error>>
    {
        let value = AccountRoles {
            account_id: "account".into(),
            catalog: RoleCatalog {
                version: 1,
                server_id: "server".into(),
                revision: 0,
                active: false,
                roles: vec![],
            },
            role_ids: BTreeSet::new(),
        };
        assert_eq!(canonical(&value)?, b"\xa3\x67catalog\xa5\x65roles\x80\x66active\xf4\x67version\x01\x68revision\x00\x69server_id\x66server\x68role_ids\x80\x6aaccount_id\x67account");
        assert_ne!(AccountRoles::DOMAIN, RoleCatalog::DOMAIN);
        let key = SigningKey::from_bytes(&[0x57; 32]);
        let public = encode(key.verifying_key().as_bytes());
        let signed = Signed {
            signature: encode(key.sign(&signing_bytes(&value)?).to_bytes()),
            value,
        };
        signed.verify(&public)?;
        let mut forged = signed.clone();
        forged.value.account_id = "other".into();
        assert!(forged.verify(&public).is_err());
        let mut forged = signed.clone();
        forged.value.role_ids.insert(ADMIN_ROLE.into());
        assert!(forged.verify(&public).is_err());
        let mut forged = signed;
        forged.value.catalog.revision = 1;
        assert!(forged.verify(&public).is_err());
        Ok(())
    }

    #[test]
    fn audit_page_binds_cursor_head_and_entries() -> Result<(), Box<dyn std::error::Error>> {
        let value = RoleAuditPage {
            version: 1,
            server_id: "server".into(),
            after_revision: 0,
            head_revision: 1,
            entries: vec![RoleAuditEntry {
                revision: 1,
                actor_device_id: "device".into(),
                operation: "initialize".into(),
                target: "server".into(),
                committed_at: 1,
            }],
        };
        let key = SigningKey::from_bytes(&[0x57; 32]);
        let public = encode(key.verifying_key().as_bytes());
        let signed = Signed {
            signature: encode(key.sign(&signing_bytes(&value)?).to_bytes()),
            value,
        };
        signed.verify(&public)?;
        for field in ["server_id", "after_revision", "head_revision", "entries"] {
            let mut value = serde_json::to_value(&signed.value)?;
            value[field] = match field {
                "server_id" => serde_json::json!("other"),
                "entries" => serde_json::json!([]),
                _ => serde_json::json!(2),
            };
            let forged = Signed::<RoleAuditPage> {
                value: serde_json::from_value(value)?,
                signature: signed.signature.clone(),
            };
            assert!(forged.verify(&public).is_err(), "unbound {field}");
        }
        Ok(())
    }

    #[test]
    fn receipt_binds_request_actor_and_commit() -> Result<(), Box<dyn std::error::Error>> {
        let receipt = RoleChangeReceipt {
            version: 1,
            server_id: "server".into(),
            revision: 1,
            request_id: "request".into(),
            request_digest: "digest".into(),
            actor_device_id: "device".into(),
        };
        let key = SigningKey::from_bytes(&[0x57; 32]);
        let public = encode(key.verifying_key().as_bytes());
        let signed = Signed {
            signature: encode(key.sign(&signing_bytes(&receipt)?).to_bytes()),
            value: receipt,
        };
        signed.verify(&public)?;
        for field in [
            "server_id",
            "revision",
            "request_id",
            "request_digest",
            "actor_device_id",
        ] {
            let mut value = serde_json::to_value(&signed.value)?;
            value[field] = if field == "revision" {
                serde_json::json!(2)
            } else {
                serde_json::json!("substituted")
            };
            let forged = Signed::<RoleChangeReceipt> {
                value: serde_json::from_value(value)?,
                signature: signed.signature.clone(),
            };
            assert!(forged.verify(&public).is_err(), "unbound {field}");
        }
        assert!(serde_json::from_value::<RoleChangeRequest>(serde_json::json!({
            "expected_revision":0,"request_id":"id","change":{"operation":"initialize"},"owner":true
        })).is_err());
        Ok(())
    }
    #[test]
    fn role_presentation_signatures_bind_every_scope_and_action_field()
    -> Result<(), Box<dyn std::error::Error>> {
        fn check<T: crate::messaging::Statement + Serialize + serde::de::DeserializeOwned>(
            value: T,
        ) -> Result<(), Box<dyn std::error::Error>> {
            let key = SigningKey::from_bytes(&[0x64; 32]);
            let public = encode(key.verifying_key().as_bytes());
            let signed = Signed {
                signature: encode(key.sign(&signing_bytes(&value)?).to_bytes()),
                value,
            };
            signed.verify(&public)?;
            let original = serde_json::to_value(&signed.value)?;
            for (field, item) in original.as_object().ok_or("object")? {
                let mut forged = original.clone();
                forged[field] = match item {
                    serde_json::Value::Bool(value) => serde_json::json!(!value),
                    serde_json::Value::Number(_) => serde_json::json!(99),
                    serde_json::Value::String(_) => serde_json::json!("substituted"),
                    serde_json::Value::Array(_) if field == "members" => serde_json::json!([{
                        "account_id":"substituted","assigned":true,"can_assign":false,"can_remove":false
                    }]),
                    serde_json::Value::Array(_) => serde_json::json!(["substituted"]),
                    _ if field == "catalog" => {
                        let mut catalog = item.clone();
                        catalog["revision"] = serde_json::json!(99);
                        catalog
                    }
                    _ => serde_json::json!({"substituted": 99}),
                };
                let forged = Signed::<T> {
                    value: serde_json::from_value(forged)?,
                    signature: signed.signature.clone(),
                };
                assert!(
                    forged.verify(&public).is_err(),
                    "unbound presentation field {field}"
                );
            }
            Ok(())
        }
        let catalog = RoleCatalog {
            version: 1,
            server_id: "server".into(),
            revision: 1,
            active: true,
            roles: vec![],
        };
        check(RoleWorkspace {
            version: 1,
            server_id: "server".into(),
            account_id: "actor".into(),
            device_id: "device".into(),
            catalog: catalog.clone(),
            counts: BTreeMap::new(),
            editable: BTreeSet::new(),
            deletable: BTreeSet::new(),
            can_create: true,
        })?;
        check(RoleTargetActions {
            version: 1,
            server_id: "server".into(),
            account_id: "actor".into(),
            device_id: "device".into(),
            target_account_id: "target".into(),
            catalog: catalog.clone(),
            role_ids: BTreeSet::new(),
            assignable: BTreeSet::new(),
            removable: BTreeSet::new(),
            eligible: true,
        })?;
        check(RoleMembersPage {
            version: 1,
            server_id: "server".into(),
            account_id: "actor".into(),
            device_id: "device".into(),
            catalog,
            role_id: DEFAULT_ROLE.into(),
            assigned: true,
            after: String::new(),
            next_after: Some("cursor".into()),
            member_count: 0,
            members: vec![],
        })?;
        assert_ne!(RoleMembersPage::DOMAIN, RoleTargetActions::DOMAIN);
        assert_ne!(RoleWorkspace::DOMAIN, RoleTargetActions::DOMAIN);
        assert_ne!(RoleWorkspace::DOMAIN, RoleCatalog::DOMAIN);
        Ok(())
    }
}
