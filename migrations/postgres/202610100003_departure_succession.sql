-- Bootstrap remains permanently CLAIMED after the first claim. Lockdown and operator
-- recovery are separate state, so neither can recreate the original claim path.
CREATE TABLE server_owner_control (
    singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (singleton),
    locked_down BOOLEAN NOT NULL DEFAULT FALSE,
    recovery_code_hash TEXT,
    recovery_generation BIGINT NOT NULL DEFAULT 0 CHECK (recovery_generation >= 0),
    recovery_issued_at BIGINT,
    CHECK ((recovery_code_hash IS NULL) = (recovery_issued_at IS NULL))
);
INSERT INTO server_owner_control(singleton) VALUES(TRUE);

CREATE TABLE server_successor_designations (
    id TEXT PRIMARY KEY,
    owner_account_id TEXT NOT NULL,
    successor_account_id TEXT NOT NULL,
    designation TEXT NOT NULL,
    accepted_at BIGINT NOT NULL,
    acceptance TEXT,
    successor_accepted_at BIGINT,
    CHECK ((acceptance IS NULL) = (successor_accepted_at IS NULL))
);
CREATE INDEX server_successor_eligible ON server_successor_designations(accepted_at, id)
    WHERE acceptance IS NOT NULL;

-- Burn tombstones survive membership deactivation and deny automatic readmission.
CREATE TABLE identity_burns (
    account_id TEXT PRIMARY KEY,
    record TEXT NOT NULL,
    accepted_at BIGINT NOT NULL
);
CREATE TABLE server_departure_receipts (
    account_id TEXT NOT NULL,
    kind TEXT NOT NULL CHECK(kind IN ('drop', 'burn')),
    request_hash TEXT NOT NULL,
    receipt TEXT NOT NULL,
    PRIMARY KEY(account_id, kind, request_hash)
);
CREATE TABLE server_owner_audit (
    id TEXT PRIMARY KEY,
    action TEXT NOT NULL,
    actor_account_id TEXT,
    subject_account_id TEXT,
    occurred_at BIGINT NOT NULL,
    details_hash TEXT NOT NULL
);

UPDATE cords_schema_metadata SET version = 9 WHERE singleton = TRUE;
