CREATE TABLE server_ownership_bootstrap (
    singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (singleton),
    state TEXT NOT NULL CHECK (state IN ('UNCLAIMED', 'CLAIMED')),
    claim_code_hash TEXT,
    generation BIGINT NOT NULL CHECK (generation > 0),
    initialized_at BIGINT NOT NULL,
    claimed_at BIGINT,
    CHECK (
        (state = 'UNCLAIMED' AND claim_code_hash IS NOT NULL AND claimed_at IS NULL)
        OR
        (state = 'CLAIMED' AND claim_code_hash IS NULL AND claimed_at IS NOT NULL)
    )
);

UPDATE cords_schema_metadata SET version = 7 WHERE singleton = TRUE;
