-- Request-scoped signed presentation, accessible only to admission reviewers.
ALTER TABLE membership_requests ADD COLUMN admission_profile TEXT
    CHECK (admission_profile IS NULL OR octet_length(admission_profile) <= 8388608);
UPDATE cords_schema_metadata SET version=18 WHERE singleton=TRUE;
