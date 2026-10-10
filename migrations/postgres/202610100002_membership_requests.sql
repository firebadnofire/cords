CREATE TABLE membership_requests (
    device_id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL,
    contact TEXT NOT NULL,
    requested_at BIGINT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending', 'approved', 'rejected')),
    decided_by TEXT,
    decided_at BIGINT
);
CREATE INDEX membership_requests_status_requested ON membership_requests(status, requested_at);
ALTER TABLE auth_challenges ADD COLUMN pending_request_hash TEXT;
ALTER TABLE auth_challenges ADD COLUMN pending_idempotency_key TEXT;
ALTER TABLE auth_challenges ADD COLUMN pending_result_code TEXT;
UPDATE cords_schema_metadata SET version = 8 WHERE singleton = TRUE;
