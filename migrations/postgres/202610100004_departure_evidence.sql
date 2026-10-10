-- Retain verifiable root departure evidence for authorized MLS members.
-- A departure is not a device revocation: retain the known-device integrity constraint
-- without requiring an unrelated root-signed revocation record to exist.
ALTER TABLE pending_policy_removals DROP CONSTRAINT pending_policy_removals_device_id_fkey;
ALTER TABLE pending_policy_removals ADD CONSTRAINT pending_policy_removals_device_id_fkey
    FOREIGN KEY(device_id) REFERENCES device_contacts(device_id);
CREATE TABLE account_departure_notices (
    account_id TEXT NOT NULL,
    kind TEXT NOT NULL CHECK(kind IN ('drop','burn')),
    record TEXT NOT NULL,
    accepted_at BIGINT NOT NULL,
    PRIMARY KEY(account_id,kind)
);
INSERT INTO account_departure_notices(account_id,kind,record,accepted_at)
    SELECT account_id,'burn',record,accepted_at FROM identity_burns;
-- Successor history remains available to the operator after recovery or transfer.
ALTER TABLE server_successor_designations ADD COLUMN retired BOOLEAN NOT NULL DEFAULT FALSE;
UPDATE cords_schema_metadata SET version=10 WHERE singleton=TRUE;
