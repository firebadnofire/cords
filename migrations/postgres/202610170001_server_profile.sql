CREATE TABLE server_profile (
    singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK(singleton),
    revision BIGINT NOT NULL CHECK(revision >= 0),
    name TEXT NOT NULL CHECK(char_length(name) <= 100),
    description TEXT NOT NULL CHECK(char_length(description) <= 2000),
    background TEXT NOT NULL DEFAULT '#314452' CHECK(background ~ '^#[0-9A-Fa-f]{6}$'),
    icon TEXT,
    icon_image BYTEA CHECK(octet_length(icon_image) <= 5000000),
    CHECK ((icon IS NULL) = (icon_image IS NULL))
);
INSERT INTO server_profile(singleton,revision,name,description) VALUES(TRUE,0,'','');
CREATE TABLE server_profile_audit (
    revision BIGINT PRIMARY KEY,
    actor_device_id TEXT NOT NULL,
    request_id TEXT NOT NULL,
    committed_at BIGINT NOT NULL
);
-- 16 belongs to the staged enforced-RBAC schema and remains unsupported.
UPDATE cords_schema_metadata SET version=17 WHERE singleton=TRUE;
