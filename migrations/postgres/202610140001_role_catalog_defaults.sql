-- Built-in roles are server defaults, not a user-triggered setup operation.
-- Seed only the untouched schema-13 catalog; never replace edited definitions.
-- Revision zero is the installation baseline, not a fabricated user audit event.
INSERT INTO server_roles(id,name,definition)
SELECT role->>'id',role->>'name',role::text
FROM jsonb_array_elements('[
 {"id":"builtin:default","name":"default","color":"#99aab5","description":"Every accepted member","priority":0,"permissions":["server.read","channel.view","channel.read","channel.write","channel.mls.commit","keypackage.publish"],"mentionable":false},
 {"id":"builtin:moderator","name":"moderator","color":"#4c9bd4","description":"Moderation and admission","priority":100,"permissions":["server.member.approve","server.device.approve","server.audit.read","server.member.kick","server.member.ban","server.member.timeout","server.moderation.read","mention_roles"],"mentionable":true},
 {"id":"builtin:admin","name":"admin","color":"#c58ae8","description":"Server administration; never ownership","priority":200,"permissions":["server.manage","server.roles.manage","server.roles.assign","server.invite.create","server.invite.manage","channel.create","channel.manage","channel.members.manage","channel.permissions.manage","server.roles.mentionability"],"mentionable":true}
]'::jsonb) AS role
WHERE EXISTS(SELECT 1 FROM role_policy WHERE singleton AND revision=0 AND NOT active)
  AND NOT EXISTS(SELECT 1 FROM server_roles)
  AND NOT EXISTS(SELECT 1 FROM role_policy_changes)
  AND NOT EXISTS(SELECT 1 FROM account_roles);

DO $$ BEGIN
    IF (SELECT count(*) FROM server_roles WHERE id IN
        ('builtin:default','builtin:moderator','builtin:admin')) <> 3 THEN
        RAISE EXCEPTION 'Role catalog is inconsistent; restore or repair verified policy before upgrading';
    END IF;
END $$;

-- Keep the schema-13 NOT active constraint. This upgrade does not enable RBAC.
UPDATE cords_schema_metadata SET version=14 WHERE singleton=TRUE;
