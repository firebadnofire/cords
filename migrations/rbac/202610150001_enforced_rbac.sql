-- Checkpoint candidate: not in the automatically applied migration directory.
-- Install only with the server that enforces this policy on every request path.
-- Native identity validation must finish before this schema can serve traffic.
CREATE TABLE archived_role_assignments (
    account_id TEXT NOT NULL REFERENCES account_heads(account_id),
    role_id TEXT NOT NULL,
    definition TEXT NOT NULL,
    source_revision BIGINT NOT NULL,
    PRIMARY KEY(account_id, role_id)
);
INSERT INTO archived_role_assignments
SELECT a.account_id,a.role_id,r.definition,p.revision
FROM account_roles a JOIN server_roles r ON r.id=a.role_id CROSS JOIN role_policy p;
DELETE FROM account_roles;

-- Certification, account admission and device approval are independent gates.
-- Native verification populates these from genuine admitted device records.
CREATE TABLE server_account_admission (
    account_id TEXT PRIMARY KEY REFERENCES account_heads(account_id),
    accepted BOOLEAN NOT NULL
);
CREATE TABLE server_device_admission (
    device_id TEXT PRIMARY KEY REFERENCES device_contacts(device_id),
    approved BOOLEAN NOT NULL
);

CREATE TABLE channel_categories (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    overrides TEXT NOT NULL DEFAULT '{}'
);
CREATE TABLE channel_access_policy (
    channel_id TEXT PRIMARY KEY REFERENCES channels(id),
    category_id TEXT REFERENCES channel_categories(id),
    account_restricted BOOLEAN NOT NULL,
    overrides TEXT NOT NULL DEFAULT '{}'
);
INSERT INTO channel_access_policy(channel_id,account_restricted)
SELECT id,confidentiality_mode='encrypted' FROM channels;

CREATE TABLE channel_account_access (
    channel_id TEXT NOT NULL REFERENCES channel_access_policy(channel_id),
    account_id TEXT NOT NULL REFERENCES account_heads(account_id),
    PRIMARY KEY(channel_id,account_id)
);
-- Account eligibility cannot reactivate an excluded device. Explicit enrollment
-- may clear a channel-local exclusion, but can never clear identity revocation.
CREATE TABLE channel_device_exclusions (
    channel_id TEXT NOT NULL REFERENCES channels(id),
    device_id TEXT NOT NULL REFERENCES device_contacts(device_id),
    PRIMARY KEY(channel_id,device_id)
);
INSERT INTO channel_device_exclusions
SELECT m.channel_id,m.device_id FROM channel_members m
JOIN device_contacts d ON d.device_id=m.device_id
WHERE NOT m.active OR NOT m.delivery_active
  OR EXISTS(SELECT 1 FROM device_revocations r WHERE r.device_id=m.device_id)
  OR EXISTS(SELECT 1 FROM identity_burns b WHERE b.account_id=d.account_id)
  OR EXISTS(SELECT 1 FROM pending_policy_removals p
            WHERE p.channel_id=m.channel_id AND p.device_id=m.device_id);

CREATE TABLE rbac_migration (
    singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK(singleton),
    completed BOOLEAN NOT NULL DEFAULT FALSE,
    checked_at BIGINT,
    rejected_devices BIGINT NOT NULL DEFAULT 0,
    CHECK(completed = (checked_at IS NOT NULL))
);
INSERT INTO rbac_migration(singleton) VALUES(TRUE);
-- Freeze the old boundary before native validation; never infer it from roles.
CREATE TABLE rbac_migration_candidates (
    channel_id TEXT NOT NULL REFERENCES channels(id),
    device_id TEXT NOT NULL REFERENCES device_contacts(device_id),
    account_id TEXT NOT NULL REFERENCES account_heads(account_id),
    joined_sequence BIGINT NOT NULL,
    PRIMARY KEY(channel_id,device_id)
);
INSERT INTO rbac_migration_candidates
SELECT c.id,d.device_id,d.account_id,m.joined_sequence
FROM channels c JOIN channel_members m ON m.channel_id=c.id
JOIN device_contacts d ON d.device_id=m.device_id
JOIN memberships s ON s.device_id=d.device_id
WHERE c.confidentiality_mode='encrypted' AND m.active AND m.delivery_active AND s.active
  AND NOT EXISTS(SELECT 1 FROM device_revocations r WHERE r.device_id=d.device_id)
  AND NOT EXISTS(SELECT 1 FROM identity_burns b WHERE b.account_id=d.account_id)
  AND NOT EXISTS(SELECT 1 FROM pending_policy_removals p
                 WHERE p.channel_id=c.id AND p.device_id=d.device_id);

-- Keep policy inactive until native verification. Version 15 binaries must
-- refuse this schema; completing migration is not an authorization bypass.
ALTER TABLE role_policy DROP CONSTRAINT role_policy_active_check;
UPDATE cords_schema_metadata SET version=16 WHERE singleton=TRUE;
