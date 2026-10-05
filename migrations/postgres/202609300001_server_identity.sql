CREATE TABLE cords_server_identity (
    singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (singleton),
    server_id TEXT NOT NULL
);
UPDATE cords_schema_metadata SET version = 2 WHERE singleton = TRUE;
