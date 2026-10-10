-- Existing rows receive their original pinned server ID and updated authenticated
-- encryption context at vault unlock, before another server can be selected.
ALTER TABLE message_cache RENAME TO message_cache_legacy;
CREATE TABLE message_cache (
    server_id TEXT NOT NULL,
    route_id TEXT NOT NULL,
    message_id TEXT NOT NULL,
    sequence INTEGER NOT NULL,
    sealed BLOB NOT NULL,
    PRIMARY KEY(server_id, route_id, message_id)
);
INSERT INTO message_cache(server_id,route_id,message_id,sequence,sealed)
    SELECT '',route_id,message_id,sequence,sealed FROM message_cache_legacy;
DROP TABLE message_cache_legacy;

ALTER TABLE ciphertext_cache RENAME TO ciphertext_cache_legacy;
CREATE TABLE ciphertext_cache (
    server_id TEXT NOT NULL,
    route_id TEXT NOT NULL,
    sequence INTEGER NOT NULL,
    envelope BLOB NOT NULL,
    PRIMARY KEY(server_id, route_id, sequence)
);
INSERT INTO ciphertext_cache(server_id,route_id,sequence,envelope)
    SELECT '',route_id,sequence,envelope FROM ciphertext_cache_legacy;
DROP TABLE ciphertext_cache_legacy;

UPDATE cords_schema_metadata SET version=3 WHERE singleton=1;
