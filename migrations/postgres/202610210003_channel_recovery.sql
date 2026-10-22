-- Additive transport/storage support only; this does not activate role policy.
ALTER TABLE roster_operations ADD COLUMN cancelled BOOLEAN NOT NULL DEFAULT FALSE;
DROP INDEX one_pending_roster_change;
CREATE UNIQUE INDEX one_pending_roster_change ON roster_operations(channel_id)
    WHERE NOT complete AND NOT cancelled;
CREATE TABLE channel_recovery_audit (
    predecessor_id TEXT PRIMARY KEY REFERENCES channels(id),
    successor_id TEXT NOT NULL UNIQUE REFERENCES channels(id),
    actor_device_id TEXT NOT NULL REFERENCES device_contacts(device_id),
    policy_revision BIGINT NOT NULL CHECK(policy_revision >= 0),
    channel_epoch BIGINT NOT NULL CHECK(channel_epoch >= 0),
    pending_digest TEXT NOT NULL,
    recovery TEXT NOT NULL,
    succession TEXT NOT NULL,
    cancelled_operations TEXT NOT NULL,
    committed_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);
UPDATE cords_schema_metadata SET version=20 WHERE singleton=TRUE;
