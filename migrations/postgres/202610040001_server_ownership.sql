CREATE TABLE server_ownership (
    singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (singleton),
    account_id TEXT NOT NULL,
    claimed_by_device_id TEXT NOT NULL REFERENCES memberships(device_id),
    claimed_at BIGINT NOT NULL
);
UPDATE cords_schema_metadata SET version = 6 WHERE singleton = TRUE;
