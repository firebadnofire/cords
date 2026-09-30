CREATE TABLE cords_schema_metadata (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    version INTEGER NOT NULL CHECK (version >= 1)
);

INSERT INTO cords_schema_metadata (singleton, version) VALUES (1, 1);
