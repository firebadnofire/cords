use super::*;

fn subject(roles: &[&str]) -> Subject {
    Subject {
        account_id: "account".into(),
        accepted: true,
        device_authorized: true,
        device_approved: true,
        revoked_or_burned: false,
        owner: false,
        roles: roles.iter().map(|id| (*id).into()).collect(),
    }
}

fn policy() -> Result<Policy, PolicyError> {
    Policy::new(default_roles(), DeviceAdmission::AccountAuthorized)
}

#[test]
fn complete_reorder_checks_old_and_new_authority_without_intermediate_catalogs()
-> Result<(), PolicyError> {
    use cords_protocol::roles::RolePosition;
    let mut roles = default_roles();
    let id = uuid::Uuid::now_v7().to_string();
    roles.push(Role {
        id: id.clone(),
        name: "Helpers".into(),
        color: "#123456".into(),
        description: String::new(),
        priority: 20,
        permissions: BTreeSet::new(),
        mentionable: false,
    });
    let policy = Policy::new(roles, DeviceAdmission::AccountAuthorized)?;
    let owner = Subject {
        owner: true,
        ..subject(&[])
    };
    let mut positions: Vec<RolePosition> = policy
        .roles()
        .values()
        .map(|role| RolePosition {
            role_id: role.id.clone(),
            priority: role.priority,
        })
        .collect();
    // Final admin/moderator order is valid even though editing admin first would
    // temporarily put it below the previous moderator rank.
    for position in &mut positions {
        if position.role_id == ADMIN_ROLE {
            position.priority = 60;
        }
        if position.role_id == MODERATOR_ROLE {
            position.priority = 40;
        }
    }
    policy.reorder(&owner, &positions)?;
    assert!(policy.reorder(&subject(&[]), &positions).is_err());
    assert!(policy.reorder(&subject(&[ADMIN_ROLE]), &positions).is_err());
    let mut duplicate = positions.clone();
    duplicate[0] = duplicate[1].clone();
    assert!(policy.reorder(&owner, &duplicate).is_err());
    assert!(policy.reorder(&owner, &positions[1..]).is_err());
    let mut substituted = positions.clone();
    substituted[0].role_id = "unknown".into();
    assert!(policy.reorder(&owner, &substituted).is_err());
    let mut default = positions.clone();
    default
        .iter_mut()
        .find(|position| position.role_id == DEFAULT_ROLE)
        .unwrap()
        .priority = 1;
    assert!(policy.reorder(&owner, &default).is_err());
    let mut lower: Vec<RolePosition> = policy
        .roles()
        .values()
        .map(|role| RolePosition {
            role_id: role.id.clone(),
            priority: role.priority,
        })
        .collect();
    lower
        .iter_mut()
        .find(|position| position.role_id == id)
        .unwrap()
        .priority = 21;
    policy.reorder(&subject(&[ADMIN_ROLE]), &lower)?;
    lower
        .iter_mut()
        .find(|position| position.role_id == id)
        .unwrap()
        .priority = 200;
    assert!(policy.reorder(&subject(&[ADMIN_ROLE]), &lower).is_err());
    Ok(())
}

fn rule(allow: &[Permission], deny: &[Permission]) -> Override {
    Override {
        allow: allow.iter().copied().collect(),
        deny: deny.iter().copied().collect(),
    }
}

#[test]
fn baseline_has_no_administration_or_role_ping_authority() -> Result<(), PolicyError> {
    let policy = policy()?;
    let ordinary = subject(&[]);
    let access = policy.access(&ordinary, &Overrides::new(), &Overrides::new())?;
    assert!(access.eligible);
    assert!(access.permissions.contains(&Permission::SendMessages));
    assert!(!access.permissions.contains(&Permission::ManageRoles));
    assert!(!policy.may_mention(
        &ordinary,
        MODERATOR_ROLE,
        &Overrides::new(),
        &Overrides::new()
    )?);
    assert!(!policy.roles()[DEFAULT_ROLE].mentionable);
    Ok(())
}

#[test]
fn lockdown_never_bypasses_device_or_identity_gates_even_for_owner() -> Result<(), PolicyError> {
    let policy = Policy::new(default_roles(), DeviceAdmission::AdministratorApproval)?;
    for owner in [false, true] {
        for gate in 0..4 {
            let mut actor = subject(&[ADMIN_ROLE]);
            actor.owner = owner;
            match gate {
                0 => actor.accepted = false,
                1 => actor.device_authorized = false,
                2 => actor.device_approved = false,
                _ => actor.revoked_or_burned = true,
            }
            let access = policy.access(&actor, &Overrides::new(), &Overrides::new())?;
            assert!(!access.eligible);
            assert!(access.permissions.is_empty());
            assert_eq!(
                policy.authorize_edit(&actor, ADMIN_ROLE, Some(&policy.roles()[ADMIN_ROLE])),
                Err(PolicyError::Denied)
            );
        }
    }
    let mut device = subject(&[]);
    device.device_approved = false;
    let community = Policy::new(default_roles(), DeviceAdmission::AccountAuthorized)?;
    assert!(
        community
            .access(&device, &Overrides::new(), &Overrides::new())?
            .eligible
    );
    Ok(())
}

#[test]
fn admin_inherits_moderator_but_priority_does_not_inherit_custom_grants() -> Result<(), PolicyError>
{
    let mut roles = default_roles();
    roles.push(Role {
        id: "01a131cd-a0af-7631-b2c6-c4cef9306588".into(),
        name: "special".into(),
        color: "#ffffff".into(),
        description: String::new(),
        priority: 50,
        permissions: [Permission::ManageEmojis].into(),
        mentionable: false,
    });
    let policy = Policy::new(roles, DeviceAdmission::AccountAuthorized)?;
    let access = policy.access(
        &subject(&[ADMIN_ROLE]),
        &Overrides::new(),
        &Overrides::new(),
    )?;
    assert!(access.permissions.contains(&Permission::ApproveDevices));
    assert!(!access.permissions.contains(&Permission::ManageEmojis));
    Ok(())
}

#[test]
fn hidden_admin_category_cannot_be_revealed_by_channel_allow() -> Result<(), PolicyError> {
    let policy = policy()?;
    let category: Overrides = [
        (DEFAULT_ROLE.into(), rule(&[], &[Permission::ViewChannel])),
        (MODERATOR_ROLE.into(), rule(&[Permission::ViewChannel], &[])),
    ]
    .into();
    let channel = [(DEFAULT_ROLE.into(), rule(&[Permission::ViewChannel], &[]))].into();
    assert!(!policy.access(&subject(&[]), &category, &channel)?.eligible);
    for role in [MODERATOR_ROLE, ADMIN_ROLE] {
        assert!(
            policy
                .access(&subject(&[role]), &category, &channel)?
                .eligible
        );
    }
    Ok(())
}

#[test]
fn conflicting_assigned_roles_deny_independently_of_iteration_order() -> Result<(), PolicyError> {
    let policy = policy()?;
    let overrides: Overrides = [
        (
            MODERATOR_ROLE.into(),
            rule(&[Permission::SendMessages], &[]),
        ),
        (ADMIN_ROLE.into(), rule(&[], &[Permission::SendMessages])),
    ]
    .into();
    for roles in [[ADMIN_ROLE, MODERATOR_ROLE], [MODERATOR_ROLE, ADMIN_ROLE]] {
        let access = policy.access(&subject(&roles), &Overrides::new(), &overrides)?;
        assert!(!access.permissions.contains(&Permission::SendMessages));
        assert!(!policy.may_mention(
            &subject(&roles),
            MODERATOR_ROLE,
            &Overrides::new(),
            &overrides
        )?);
    }
    Ok(())
}

#[test]
fn all_paths_reject_invalid_policy_and_unknown_authority() -> Result<(), PolicyError> {
    let policy = policy()?;
    assert_eq!(
        policy.access(&subject(&["missing"]), &Overrides::new(), &Overrides::new()),
        Err(PolicyError::UnknownRole)
    );
    let privilege_override = [(DEFAULT_ROLE.into(), rule(&[Permission::ManageRoles], &[]))].into();
    assert_eq!(
        policy.validate_overrides(&privilege_override),
        Err(PolicyError::Invalid)
    );
    let contradictory = [(
        DEFAULT_ROLE.into(),
        rule(&[Permission::ReadMessages], &[Permission::ReadMessages]),
    )]
    .into();
    assert_eq!(
        policy.validate_overrides(&contradictory),
        Err(PolicyError::Invalid)
    );
    for bad in ["$admin", "with\nnewline", " admin", ""] {
        let mut roles = default_roles();
        roles[1].name = bad.into();
        assert!(Policy::new(roles, DeviceAdmission::AccountAuthorized).is_err());
    }
    let mut roles = default_roles();
    roles[0].permissions.insert(Permission::ManageRoles);
    assert!(Policy::new(roles, DeviceAdmission::AccountAuthorized).is_err());
    let mut roles = default_roles();
    roles.push(roles[0].clone());
    assert!(Policy::new(roles, DeviceAdmission::AccountAuthorized).is_err());
    Ok(())
}

#[test]
fn role_mutations_and_assignments_cannot_escalate_authority() -> Result<(), PolicyError> {
    let policy = policy()?;
    let admin = subject(&[ADMIN_ROLE]);
    let mut custom = Role {
        id: "01a131cd-a0af-7631-b2c6-c4cef9306588".into(),
        name: "developers".into(),
        color: "#aabbcc".into(),
        description: String::new(),
        priority: 10,
        permissions: BTreeSet::new(),
        mentionable: false,
    };
    policy.authorize_edit(&admin, &custom.id, Some(&custom))?;
    custom.priority = 200;
    assert_eq!(
        policy.authorize_edit(&admin, &custom.id, Some(&custom)),
        Err(PolicyError::Denied)
    );
    custom.priority = 10;
    custom.permissions.insert(Permission::ManageEmojis);
    assert_eq!(
        policy.authorize_edit(&admin, &custom.id, Some(&custom)),
        Err(PolicyError::Denied)
    );
    assert_eq!(
        policy.authorize_edit(&admin, ADMIN_ROLE, Some(&policy.roles()[ADMIN_ROLE])),
        Err(PolicyError::Denied)
    );
    let mut target = subject(&[]);
    target.account_id = "other".into();
    policy.authorize_assignment(&admin, &target, MODERATOR_ROLE)?;
    assert_eq!(
        policy.authorize_assignment(&admin, &target, ADMIN_ROLE),
        Err(PolicyError::Denied)
    );
    assert_eq!(
        policy.authorize_assignment(&admin, &admin, MODERATOR_ROLE),
        Err(PolicyError::Denied)
    );
    target.owner = true;
    assert_eq!(
        policy.authorize_assignment(&admin, &target, MODERATOR_ROLE),
        Err(PolicyError::Denied)
    );
    let mut owner = admin.clone();
    owner.owner = true;
    assert_eq!(
        policy.authorize_edit(&owner, DEFAULT_ROLE, None),
        Err(PolicyError::Denied)
    );
    Ok(())
}

#[test]
fn departed_or_burned_targets_can_lose_roles_but_cannot_gain_them() -> Result<(), PolicyError> {
    let policy = policy()?;
    let admin = subject(&[ADMIN_ROLE]);
    let mut target = subject(&[MODERATOR_ROLE]);
    target.account_id = "departed".into();
    target.accepted = false;
    target.device_authorized = false;
    for burned in [false, true] {
        target.revoked_or_burned = burned;
        policy.authorize_revocation(&admin, &target, MODERATOR_ROLE)?;
        assert_eq!(
            policy.authorize_assignment(&admin, &target, MODERATOR_ROLE),
            Err(PolicyError::Denied)
        );
        assert_eq!(
            policy.authorize_revocation(&subject(&[]), &target, MODERATOR_ROLE),
            Err(PolicyError::Denied)
        );
        let mut revoked_actor = admin.clone();
        revoked_actor.revoked_or_burned = true;
        assert_eq!(
            policy.authorize_revocation(&revoked_actor, &target, MODERATOR_ROLE),
            Err(PolicyError::Denied)
        );
        target.owner = true;
        assert_eq!(
            policy.authorize_revocation(&admin, &target, MODERATOR_ROLE),
            Err(PolicyError::Denied)
        );
        target.owner = false;
        target.roles.insert(ADMIN_ROLE.into());
        assert_eq!(
            policy.authorize_revocation(&admin, &target, MODERATOR_ROLE),
            Err(PolicyError::Denied)
        );
        target.roles.remove(ADMIN_ROLE);
    }
    Ok(())
}

#[test]
fn mentions_require_target_opt_in_sender_grant_and_visible_writable_channel()
-> Result<(), PolicyError> {
    let policy = policy()?;
    let moderator = subject(&[MODERATOR_ROLE]);
    assert!(policy.may_mention(
        &moderator,
        MODERATOR_ROLE,
        &Overrides::new(),
        &Overrides::new()
    )?);
    assert!(!policy.may_mention(
        &moderator,
        DEFAULT_ROLE,
        &Overrides::new(),
        &Overrides::new()
    )?);
    assert!(!policy.may_mention(&moderator, "unknown", &Overrides::new(), &Overrides::new())?);
    let channel = [(DEFAULT_ROLE.into(), rule(&[], &[Permission::ReadMessages]))].into();
    assert!(!policy.may_mention(&moderator, MODERATOR_ROLE, &Overrides::new(), &channel)?);
    Ok(())
}
