CREATE TABLE pending_policy_removals (
    channel_id TEXT NOT NULL REFERENCES channels,
    device_id TEXT NOT NULL REFERENCES device_revocations,
    PRIMARY KEY(channel_id, device_id)
);
INSERT INTO pending_policy_removals(channel_id, device_id)
SELECT m.channel_id, m.device_id FROM channel_members m
JOIN device_revocations r ON r.device_id=m.device_id WHERE m.active;
UPDATE channel_members SET delivery_active=FALSE
WHERE device_id IN (SELECT device_id FROM device_revocations);
UPDATE cords_schema_metadata SET version = 5 WHERE singleton = TRUE;
