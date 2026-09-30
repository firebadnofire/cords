CREATE TABLE cords_schema_metadata (
    singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (singleton),
    version BIGINT NOT NULL CHECK (version >= 1)
);

INSERT INTO cords_schema_metadata (singleton, version) VALUES (TRUE, 1);
