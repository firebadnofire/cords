CREATE TABLE installation (singleton INTEGER PRIMARY KEY CHECK(singleton=1), id TEXT NOT NULL, protection TEXT NOT NULL, salt BLOB NOT NULL, wrapped_key BLOB NOT NULL);
CREATE TABLE client_state (singleton INTEGER PRIMARY KEY CHECK(singleton=1), sealed BLOB NOT NULL);
CREATE TABLE message_cache (route_id TEXT NOT NULL, message_id TEXT NOT NULL, sequence INTEGER NOT NULL, sealed BLOB NOT NULL, PRIMARY KEY(route_id,message_id));
CREATE TABLE ciphertext_cache (route_id TEXT NOT NULL, sequence INTEGER NOT NULL, envelope BLOB NOT NULL, PRIMARY KEY(route_id,sequence));
UPDATE cords_schema_metadata SET version=2 WHERE singleton=1;
