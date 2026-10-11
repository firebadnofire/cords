-- Separate a channel's immutable incoming relationship from its later outgoing one.
ALTER TABLE channels ADD COLUMN predecessor_succession TEXT;
DROP TRIGGER immutable_channel_identity ON channels;
UPDATE channels SET predecessor_succession=succession,succession=NULL
    WHERE replaces_channel_id IS NOT NULL;
CREATE OR REPLACE FUNCTION preserve_channel_identity() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'channel identities must be retained as tombstones';
    END IF;
    IF NEW.id IS DISTINCT FROM OLD.id OR NEW.confidentiality_mode IS DISTINCT FROM OLD.confidentiality_mode
       OR NEW.creator IS DISTINCT FROM OLD.creator OR NEW.created_at IS DISTINCT FROM OLD.created_at
       OR NEW.name IS DISTINCT FROM OLD.name OR (OLD.retired AND NOT NEW.retired)
       OR (OLD.succession IS NOT NULL AND NEW.succession IS DISTINCT FROM OLD.succession)
       OR NEW.predecessor_succession IS DISTINCT FROM OLD.predecessor_succession
       OR NEW.replaces_channel_id IS DISTINCT FROM OLD.replaces_channel_id THEN
        RAISE EXCEPTION 'immutable channel identity or transition';
    END IF;
    IF NEW.confidentiality_mode = 'public' AND (NEW.binding IS NOT NULL OR NEW.epoch <> 0) THEN
        RAISE EXCEPTION 'public channels cannot use MLS';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER immutable_channel_identity BEFORE UPDATE OR DELETE ON channels
    FOR EACH ROW EXECUTE FUNCTION preserve_channel_identity();
UPDATE cords_schema_metadata SET version=12 WHERE singleton=TRUE;
