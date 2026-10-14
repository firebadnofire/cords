-- Additive preparation only: existing servers retain legacy authorization until
-- the coordinated endpoint/client activation gate is passed. No inferred roles,
-- device approvals, signatures, or MLS memberships are manufactured here.
CREATE TABLE role_policy (
    singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (singleton),
    revision BIGINT NOT NULL DEFAULT 0 CHECK (revision >= 0),
    -- Schema 13 cannot activate RBAC. The validated activation migration must
    -- remove this fence AND advance the schema so preparatory binaries refuse it.
    active BOOLEAN NOT NULL DEFAULT FALSE CHECK (NOT active),
    preset TEXT CHECK (preset IN ('community', 'private_group', 'lockdown')),
    device_admission TEXT NOT NULL DEFAULT 'account_authorized'
        CHECK (device_admission IN ('account_authorized', 'administrator_approval'))
);
INSERT INTO role_policy(singleton) VALUES(TRUE);

CREATE TABLE server_roles (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 64),
    definition TEXT NOT NULL,
    CHECK (id IN ('builtin:default', 'builtin:moderator', 'builtin:admin')
        OR id ~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$')
);
CREATE UNIQUE INDEX server_roles_name_unique ON server_roles(lower(name));
CREATE TABLE account_roles (
    account_id TEXT NOT NULL REFERENCES account_heads(account_id),
    role_id TEXT NOT NULL REFERENCES server_roles(id),
    PRIMARY KEY(account_id, role_id),
    CHECK (role_id <> 'builtin:default')
);

-- Idempotent receipts and audit commit in the same transaction as the mutation.
-- Contains administrative metadata only, never keys, tokens or message bodies.
CREATE TABLE role_policy_changes (
    revision BIGINT PRIMARY KEY CHECK (revision > 0),
    actor_device_id TEXT NOT NULL REFERENCES device_contacts(device_id),
    request_id TEXT NOT NULL CHECK (length(request_id) BETWEEN 1 AND 128),
    request_digest BYTEA NOT NULL CHECK (octet_length(request_digest) = 32),
    operation TEXT NOT NULL CHECK (operation IN ('initialize', 'put_role', 'delete_role', 'assign_role', 'revoke_role')),
    target TEXT NOT NULL,
    committed_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(actor_device_id, request_id)
);
UPDATE cords_schema_metadata SET version = 13 WHERE singleton = TRUE;
