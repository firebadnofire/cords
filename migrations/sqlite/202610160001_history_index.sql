-- Cursor pagination follows the existing server delivery order. No plaintext
-- or new key material is added to the account vault.
CREATE INDEX message_cache_history ON message_cache(server_id,route_id,sequence,message_id);
