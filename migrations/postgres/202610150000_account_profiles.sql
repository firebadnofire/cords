-- Separate public account presentation from device authority and message storage.
-- Signed history is append-only; image bytes are deduplicated without compaction.
-- Additive profile storage precedes the separately staged RBAC candidate.
CREATE TABLE profile_images (
    sha256 TEXT PRIMARY KEY CHECK (sha256 ~ '^[A-Za-z0-9_-]{43}$'),
    encoded BYTEA NOT NULL CHECK (octet_length(encoded) BETWEEN 1 AND 5000000),
    width INTEGER NOT NULL CHECK (width BETWEEN 1 AND 1024),
    height INTEGER NOT NULL CHECK (height BETWEEN 1 AND 1024)
);
CREATE TABLE profile_revisions (
    id TEXT PRIMARY KEY CHECK (id ~ '^[A-Za-z0-9_-]{43}$'),
    account_id TEXT NOT NULL,
    device_id TEXT NOT NULL,
    generation BIGINT NOT NULL CHECK (generation >= 0),
    record TEXT NOT NULL CHECK (octet_length(record) <= 32768),
    avatar_hash TEXT REFERENCES profile_images(sha256),
    parents TEXT[] NOT NULL CHECK (cardinality(parents) <= 64)
);
CREATE INDEX profile_history ON profile_revisions(account_id,id COLLATE "C");
CREATE INDEX profile_avatar_references ON profile_revisions(account_id,avatar_hash);
UPDATE cords_schema_metadata SET version=15 WHERE singleton=TRUE;
