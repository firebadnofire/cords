-- Each account vault has its own database and authenticated encryption key.
CREATE TABLE profile_images (
    hash TEXT PRIMARY KEY,
    sealed BLOB NOT NULL CHECK(length(sealed) <= 5000100),
    accessed INTEGER NOT NULL
);
UPDATE cords_schema_metadata SET version=4 WHERE singleton=1;
