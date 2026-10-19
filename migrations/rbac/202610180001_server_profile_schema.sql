-- Preserve the unsupported staged-policy gate after server presentation schema 17.
UPDATE cords_schema_metadata SET version=18 WHERE singleton=TRUE;
