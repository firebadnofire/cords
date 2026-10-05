CREATE TABLE device_revocations (
    device_id TEXT PRIMARY KEY REFERENCES device_contacts,
    account_id TEXT NOT NULL REFERENCES account_heads,
    generation BIGINT NOT NULL,
    record TEXT NOT NULL
);
ALTER TABLE roster_operations ALTER COLUMN package_id DROP NOT NULL;
ALTER TABLE roster_operations ADD COLUMN action TEXT NOT NULL DEFAULT 'add' CHECK (action IN ('add', 'remove'));
CREATE TABLE channel_epochs (
    channel_id TEXT NOT NULL REFERENCES channels,
    epoch BIGINT NOT NULL,
    view TEXT NOT NULL,
    PRIMARY KEY(channel_id, epoch)
);
ALTER TABLE channel_members ADD COLUMN delivery_active BOOLEAN NOT NULL DEFAULT TRUE;
UPDATE cords_schema_metadata SET version = 4 WHERE singleton = TRUE;
