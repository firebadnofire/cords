//! Deterministic role policy. Callers must derive `Subject` from authenticated durable state.
//! Not activated on legacy messaging routes until the versioned policy integration gate passes.
use cords_protocol::roles::{
    ADMIN_ROLE, DEFAULT_ROLE, DeviceAdmission, MAX_ROLES, MODERATOR_ROLE, Override, Overrides,
    Permission, Role,
};
use std::collections::{BTreeMap, BTreeSet};

pub mod migration;
pub mod persistence;
pub mod runtime;

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PolicyError {
    #[error("role policy is invalid")]
    Invalid,
    #[error("role is unknown")]
    UnknownRole,
    #[error("role operation is not authorized")]
    Denied,
}

/// Identity, account and device admission are independent gates; ownership cannot bypass them.
#[derive(Clone, Debug)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "Independent authority gates; none implies another"
)]
pub struct Subject {
    pub account_id: String,
    pub accepted: bool,
    pub device_authorized: bool,
    pub device_approved: bool,
    pub revoked_or_burned: bool,
    pub owner: bool,
    pub roles: BTreeSet<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Access {
    pub permissions: BTreeSet<Permission>,
    /// Server eligibility only. Encrypted delivery ALSO requires a committed MLS roster entry.
    pub eligible: bool,
}

#[derive(Clone, Debug)]
pub struct Policy {
    roles: BTreeMap<String, Role>,
    pub device_admission: DeviceAdmission,
}

impl Policy {
    /// Validate before making policy available to any authorization decision.
    /// # Errors
    /// Rejects malformed, ambiguous, duplicate or overprivileged built-in role definitions.
    pub fn new(roles: Vec<Role>, device_admission: DeviceAdmission) -> Result<Self, PolicyError> {
        if roles.len() > MAX_ROLES {
            return Err(PolicyError::Invalid);
        }
        let count = roles.len();
        let roles: BTreeMap<_, _> = roles
            .into_iter()
            .map(|role| (role.id.clone(), role))
            .collect();
        if roles.len() != count {
            return Err(PolicyError::Invalid);
        }
        let mut names = BTreeSet::new();
        for role in roles.values() {
            validate_role(role)?;
            if !names.insert(role.name.to_lowercase()) {
                return Err(PolicyError::Invalid);
            }
        }
        let default = roles.get(DEFAULT_ROLE).ok_or(PolicyError::Invalid)?;
        let moderator = roles.get(MODERATOR_ROLE).ok_or(PolicyError::Invalid)?;
        let admin = roles.get(ADMIN_ROLE).ok_or(PolicyError::Invalid)?;
        if default.priority != 0 || moderator.priority == 0 || admin.priority <= moderator.priority
        {
            return Err(PolicyError::Invalid);
        }
        Ok(Self {
            roles,
            device_admission,
        })
    }

    #[must_use]
    pub fn roles(&self) -> &BTreeMap<String, Role> {
        &self.roles
    }

    fn admitted(&self, subject: &Subject) -> bool {
        subject.accepted
            && subject.device_authorized
            && !subject.revoked_or_burned
            && (self.device_admission == DeviceAdmission::AccountAuthorized
                || subject.device_approved)
    }

    fn effective_roles(&self, subject: &Subject) -> Result<BTreeSet<String>, PolicyError> {
        if subject.roles.len() > MAX_ROLES
            || subject.roles.iter().any(|id| !self.roles.contains_key(id))
        {
            return Err(PolicyError::UnknownRole);
        }
        let mut roles = subject.roles.clone();
        roles.insert(DEFAULT_ROLE.into());
        if roles.contains(ADMIN_ROLE) {
            roles.insert(MODERATOR_ROLE.into());
        }
        Ok(roles)
    }

    /// The caller must still enforce actual MLS membership for encrypted routes.
    /// # Errors
    /// Unknown roles and malformed overrides fail closed.
    pub fn access(
        &self,
        subject: &Subject,
        category: &Overrides,
        channel: &Overrides,
    ) -> Result<Access, PolicyError> {
        self.validate_overrides(category)?;
        self.validate_overrides(channel)?;
        let roles = self.effective_roles(subject)?;
        if !self.admitted(subject) {
            return Ok(Access {
                permissions: BTreeSet::new(),
                eligible: false,
            });
        }
        let mut permissions: BTreeSet<_> = roles
            .iter()
            .flat_map(|id| self.roles[id].permissions.iter().copied())
            .collect();
        // Owner-only operations are not represented by this set. Their root-proof checks remain.
        if subject.owner {
            permissions.extend(owner_management());
        } else {
            apply_overrides(&mut permissions, &roles, category);
            if !permissions.contains(&Permission::ViewChannel) {
                permissions.retain(|permission| !permission.channel_scoped());
                return Ok(Access {
                    permissions,
                    eligible: false,
                });
            }
            apply_overrides(&mut permissions, &roles, channel);
        }
        let eligible = permissions.contains(&Permission::ViewServer)
            && permissions.contains(&Permission::ViewChannel)
            && permissions.contains(&Permission::ReadMessages);
        if !permissions.contains(&Permission::ViewChannel) {
            permissions.retain(|permission| !permission.channel_scoped());
        }
        Ok(Access {
            permissions,
            eligible,
        })
    }

    /// # Errors
    /// Rejects global privilege overrides, unknown role targets and contradictory rules.
    pub fn validate_overrides(&self, overrides: &Overrides) -> Result<(), PolicyError> {
        for (id, rule) in overrides {
            if !self.roles.contains_key(id) {
                return Err(PolicyError::UnknownRole);
            }
            if !rule.allow.is_disjoint(&rule.deny)
                || rule.allow.union(&rule.deny).any(|p| !p.channel_scoped())
            {
                return Err(PolicyError::Invalid);
            }
        }
        Ok(())
    }

    fn server_permissions(&self, subject: &Subject) -> Result<BTreeSet<Permission>, PolicyError> {
        let roles = self.effective_roles(subject)?;
        if !self.admitted(subject) {
            return Ok(BTreeSet::new());
        }
        let mut permissions = roles
            .iter()
            .flat_map(|id| self.roles[id].permissions.iter().copied())
            .collect::<BTreeSet<_>>();
        if subject.owner {
            permissions.extend(owner_management());
        }
        Ok(permissions)
    }

    fn rank(&self, subject: &Subject) -> Result<u32, PolicyError> {
        Ok(self
            .effective_roles(subject)?
            .iter()
            .map(|id| self.roles[id].priority)
            .max()
            .unwrap_or(0))
    }

    /// Validates both the previous and proposed role to prevent raising its rank/capabilities.
    /// # Errors
    /// Rejects built-in deletion, self-elevation, equal/higher-rank changes and invalid policy.
    pub fn authorize_edit(
        &self,
        actor: &Subject,
        id: &str,
        proposed: Option<&Role>,
    ) -> Result<(), PolicyError> {
        let grants = self.server_permissions(actor)?;
        if !grants.contains(&Permission::ManageRoles) {
            return Err(PolicyError::Denied);
        }
        let old = self.roles.get(id);
        if proposed.is_none() && old.is_none() {
            return Err(PolicyError::UnknownRole);
        }
        if proposed.is_none() && is_builtin(id) {
            return Err(PolicyError::Denied);
        }
        if let Some(role) = proposed {
            validate_role(role)?;
            if role.id != id {
                return Err(PolicyError::Invalid);
            }
            if old.is_some_and(|old| old.mentionable != role.mentionable)
                && !grants.contains(&Permission::ManageMentionability)
            {
                return Err(PolicyError::Denied);
            }
            if old.is_none()
                && role.mentionable
                && !grants.contains(&Permission::ManageMentionability)
            {
                return Err(PolicyError::Denied);
            }
        }
        if !actor.owner {
            let assigned = self.effective_roles(actor)?;
            let rank = self.rank(actor)?;
            if assigned.contains(id)
                || old.is_some_and(|r| r.priority >= rank)
                || proposed.is_some_and(|r| r.priority >= rank || !r.permissions.is_subset(&grants))
            {
                return Err(PolicyError::Denied);
            }
        }
        let mut roles = self.roles.clone();
        if let Some(role) = proposed {
            roles.insert(id.into(), role.clone());
        } else {
            roles.remove(id);
        }
        Self::new(roles.into_values().collect(), self.device_admission)?;
        Ok(())
    }

    /// # Errors
    /// Rejects assigning/revoking authority on oneself, peers, the owner, or a higher role.
    pub fn authorize_assignment(
        &self,
        actor: &Subject,
        target: &Subject,
        role_id: &str,
    ) -> Result<(), PolicyError> {
        if !target.accepted || target.revoked_or_burned {
            return Err(PolicyError::Denied);
        }
        self.authorize_revocation(actor, target, role_id)
    }

    /// Removing stored authority must remain possible after target departure or burn.
    /// The actor still needs current admission and full assignment authority.
    /// # Errors
    /// Rejects unauthorized actors, implicit default removal and hierarchy violations.
    pub fn authorize_revocation(
        &self,
        actor: &Subject,
        target: &Subject,
        role_id: &str,
    ) -> Result<(), PolicyError> {
        let grants = self.server_permissions(actor)?;
        let role = self.roles.get(role_id).ok_or(PolicyError::UnknownRole)?;
        if !grants.contains(&Permission::AssignRoles) || role_id == DEFAULT_ROLE {
            return Err(PolicyError::Denied);
        }
        if !actor.owner
            && (actor.account_id == target.account_id
                || target.owner
                || role.priority >= self.rank(actor)?
                || self.rank(target)? >= self.rank(actor)?
                || !role.permissions.is_subset(&grants))
        {
            return Err(PolicyError::Denied);
        }
        Ok(())
    }

    /// Authorization only; recipient notification preferences/rate limits are enforced by delivery.
    /// # Errors
    /// Unknown policy references fail closed; unauthorized text remains ordinary text.
    pub fn may_mention(
        &self,
        actor: &Subject,
        target_role: &str,
        category: &Overrides,
        channel: &Overrides,
    ) -> Result<bool, PolicyError> {
        let Some(role) = self.roles.get(target_role) else {
            return Ok(false);
        };
        let access = self.access(actor, category, channel)?;
        Ok(role.mentionable
            && access.eligible
            && access.permissions.contains(&Permission::SendMessages)
            && access.permissions.contains(&Permission::MentionRoles))
    }
}

fn is_builtin(id: &str) -> bool {
    matches!(id, DEFAULT_ROLE | MODERATOR_ROLE | ADMIN_ROLE)
}

fn validate_role(role: &Role) -> Result<(), PolicyError> {
    let valid_id = is_builtin(&role.id)
        || uuid::Uuid::parse_str(&role.id).is_ok_and(|id| id.to_string() == role.id);
    if !valid_id
        || role.name.trim() != role.name
        || role.name.is_empty()
        || role.name.chars().count() > 64
        || role
            .name
            .chars()
            .any(|c| c.is_control() || matches!(c, '$' | '`' | '\\'))
        || role.description.chars().count() > 1000
        || role.color.len() != 7
        || !role.color.starts_with('#')
        || !role.color[1..].bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(PolicyError::Invalid);
    }
    let builtin_name = match role.id.as_str() {
        DEFAULT_ROLE => Some("default"),
        MODERATOR_ROLE => Some("moderator"),
        ADMIN_ROLE => Some("admin"),
        _ => None,
    };
    if builtin_name.is_some_and(|name| role.name != name)
        || (role.id == DEFAULT_ROLE && role.permissions.iter().any(|p| !p.safe_for_default()))
    {
        return Err(PolicyError::Invalid);
    }
    Ok(())
}

fn apply_rule(permissions: &mut BTreeSet<Permission>, rule: &Override) {
    permissions.extend(&rule.allow);
    permissions.retain(|permission| !rule.deny.contains(permission));
}

fn apply_overrides(
    permissions: &mut BTreeSet<Permission>,
    roles: &BTreeSet<String>,
    overrides: &Overrides,
) {
    if let Some(rule) = overrides.get(DEFAULT_ROLE) {
        apply_rule(permissions, rule);
    }
    let mut combined = Override::default();
    for id in roles.iter().filter(|id| id.as_str() != DEFAULT_ROLE) {
        if let Some(rule) = overrides.get(id) {
            combined.allow.extend(&rule.allow);
            combined.deny.extend(&rule.deny);
        }
    }
    apply_rule(permissions, &combined);
}

fn owner_management() -> BTreeSet<Permission> {
    use Permission::{
        ApproveAccounts, ApproveDevices, AssignRoles, BanMembers, CommitMls, CreateChannels,
        CreateInvites, KickMembers, ManageChannel, ManageChannelMembers, ManageChannelPermissions,
        ManageEmojis, ManageInvites, ManageMentionability, ManageMessages, ManageRoles,
        ManageServer, MentionRoles, PublishKeyPackage, ReadMessages, RetireChannel, SendMessages,
        TimeoutMembers, ViewAudit, ViewChannel, ViewModeration, ViewServer,
    };
    [
        ViewServer,
        ManageServer,
        ManageRoles,
        AssignRoles,
        CreateInvites,
        ManageInvites,
        ApproveAccounts,
        ApproveDevices,
        ViewAudit,
        ViewChannel,
        ReadMessages,
        SendMessages,
        CreateChannels,
        ManageChannel,
        RetireChannel,
        ManageChannelPermissions,
        ManageChannelMembers,
        CommitMls,
        PublishKeyPackage,
        KickMembers,
        BanMembers,
        TimeoutMembers,
        ViewModeration,
        ManageMessages,
        ManageEmojis,
        MentionRoles,
        ManageMentionability,
    ]
    .into()
}

/// Initial least-privilege templates. Capability support is gated separately by endpoints.
#[must_use]
pub fn default_roles() -> Vec<Role> {
    use Permission::{
        ApproveAccounts, ApproveDevices, AssignRoles, BanMembers, CommitMls, CreateChannels,
        CreateInvites, KickMembers, ManageChannel, ManageChannelMembers, ManageChannelPermissions,
        ManageInvites, ManageMentionability, ManageRoles, ManageServer, MentionRoles,
        PublishKeyPackage, ReadMessages, SendMessages, TimeoutMembers, ViewAudit, ViewChannel,
        ViewModeration, ViewServer,
    };
    vec![
        Role {
            id: DEFAULT_ROLE.into(),
            name: "default".into(),
            color: "#99aab5".into(),
            description: "Every accepted member".into(),
            priority: 0,
            permissions: [
                ViewServer,
                ViewChannel,
                ReadMessages,
                SendMessages,
                CommitMls,
                PublishKeyPackage,
            ]
            .into(),
            mentionable: false,
        },
        Role {
            id: MODERATOR_ROLE.into(),
            name: "moderator".into(),
            color: "#4c9bd4".into(),
            description: "Moderation and admission".into(),
            priority: 100,
            permissions: [
                ApproveAccounts,
                ApproveDevices,
                ViewAudit,
                KickMembers,
                BanMembers,
                TimeoutMembers,
                ViewModeration,
                MentionRoles,
            ]
            .into(),
            mentionable: true,
        },
        Role {
            id: ADMIN_ROLE.into(),
            name: "admin".into(),
            color: "#c58ae8".into(),
            description: "Server administration; never ownership".into(),
            priority: 200,
            permissions: [
                ManageServer,
                ManageRoles,
                AssignRoles,
                CreateInvites,
                ManageInvites,
                CreateChannels,
                ManageChannel,
                ManageChannelMembers,
                ManageChannelPermissions,
                ManageMentionability,
            ]
            .into(),
            mentionable: true,
        },
    ]
}

#[cfg(test)]
mod tests;
