//! Role-policy values. These are not additions to legacy signed membership objects.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const DEFAULT_ROLE: &str = "builtin:default";
pub const MODERATOR_ROLE: &str = "builtin:moderator";
pub const ADMIN_ROLE: &str = "builtin:admin";
pub const MAX_ROLES: usize = 256;
pub const ROLE_AUDIT_PAGE_SIZE: usize = 50;

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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum RoleChange {
    Initialize,
    PutRole { role: Role },
    DeleteRole { role_id: String },
    AssignRole { account_id: String, role_id: String },
    RevokeRole { account_id: String, role_id: String },
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
}
