-- Capability negotiation is per session, separate from signed V1 credentials.
-- NULL denotes an older client; it cannot use an enforced-policy deployment.
ALTER TABLE sessions ADD COLUMN role_policy_version SMALLINT
    CHECK (role_policy_version = 1);
UPDATE cords_schema_metadata SET version = 19 WHERE singleton = TRUE;
