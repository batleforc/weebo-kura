-- RFC 0004: a scoped token carries 1 to 16 folder rules, each `read` or
-- `write`, and a name. `path_prefix` stays for a rollback: a token with one
-- read rule keeps its folder there; any other stores '/', which no vault
-- path starts with, so a binary from before this migration sees nothing
-- through it (T8).
ALTER TABLE scoped_token
    ADD COLUMN name text CHECK (octet_length(name) BETWEEN 1 AND 64),
    ADD COLUMN scopes jsonb;
UPDATE scoped_token
    SET scopes = jsonb_build_array(jsonb_build_object('path', path_prefix, 'mode', 'read')),
        name = 'key-' || left(id::text, 8);
ALTER TABLE scoped_token
    ALTER COLUMN name SET NOT NULL,
    ALTER COLUMN scopes SET NOT NULL,
    ADD CONSTRAINT scoped_token_scopes_check
        CHECK (jsonb_typeof(scopes) = 'array' AND jsonb_array_length(scopes) BETWEEN 1 AND 16),
    DROP CONSTRAINT scoped_token_mode_check,
    ADD CONSTRAINT scoped_token_mode_check CHECK (mode IN ('read', 'write'));

-- A binary from before this migration inserts neither column (§13): its
-- token gets the one read rule its prefix means.
CREATE FUNCTION kura_scoped_token_defaults() RETURNS trigger
    LANGUAGE plpgsql SET search_path = pg_catalog, public
    AS $$
    BEGIN
        IF NEW.scopes IS NULL THEN
            NEW.scopes := jsonb_build_array(jsonb_build_object('path', NEW.path_prefix, 'mode', 'read'));
        END IF;
        IF NEW.name IS NULL THEN
            NEW.name := 'key-' || left(NEW.id::text, 8);
        END IF;
        RETURN NEW;
    END
    $$;
CREATE TRIGGER scoped_token_defaults BEFORE INSERT ON scoped_token
    FOR EACH ROW EXECUTE FUNCTION kura_scoped_token_defaults();

-- The lookup also returns the rules and the name; the return type changes,
-- so the function is replaced whole.
DROP FUNCTION kura_scoped_token_lookup(uuid);
CREATE FUNCTION kura_scoped_token_lookup(p_id uuid)
    RETURNS TABLE (tenant_id uuid, vault_id uuid, path_prefix text, allowed_cidrs text[],
                   expires_at timestamptz, revoked_at timestamptz, token_hash bytea,
                   granted_by_claim text, binding_manager boolean, scopes jsonb, name text)
    LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        SELECT t.tenant_id, t.vault_id, t.path_prefix, t.allowed_cidrs::text[], t.expires_at, t.revoked_at,
               t.token_hash, t.granted_by_claim,
               EXISTS (SELECT 1 FROM role_binding b WHERE b.id = t.granted_by_binding AND b.role = 'manager'),
               t.scopes, t.name
        FROM scoped_token t WHERE t.id = p_id
    $$;
REVOKE ALL ON FUNCTION kura_scoped_token_lookup(uuid) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION kura_scoped_token_lookup(uuid) TO kura_app;

-- Schema floor (decision 11): the oldest binary, by its newest migration,
-- that may serve this schema. `check_schema` refuses to start below it. A
-- migration that is not safe to roll back raises it; this one does not,
-- because '/' above already fails closed.
CREATE TABLE schema_floor (
    one                  boolean PRIMARY KEY DEFAULT true CHECK (one),
    min_binary_migration bigint NOT NULL
);
INSERT INTO schema_floor (min_binary_migration) VALUES (0);
ALTER TABLE schema_floor ENABLE ROW LEVEL SECURITY;
ALTER TABLE schema_floor FORCE ROW LEVEL SECURITY;
DO $$
BEGIN
    EXECUTE format('CREATE POLICY owner_all ON schema_floor TO %I USING (true) WITH CHECK (true)', current_user);
END
$$;
CREATE POLICY app_read ON schema_floor FOR SELECT TO kura_app USING (true);
GRANT SELECT ON schema_floor TO kura_app;

-- What a token proved it holds (RFC 0004 §6.4): content it uploaded itself.
-- With what it can already read, this is all a write token may name in a
-- manifest or a commit, so a hash alone is not a capability (T2, T3).
CREATE TABLE token_upload (
    token_id   uuid NOT NULL REFERENCES scoped_token (id) ON DELETE CASCADE,
    vault_id   uuid NOT NULL,
    tenant_id  uuid NOT NULL,
    kind       text NOT NULL CHECK (kind IN ('chunk', 'blob')),
    hash       bytea NOT NULL CHECK (octet_length(hash) = 32),
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (token_id, kind, hash)
);
CREATE INDEX token_upload_created ON token_upload (created_at);
ALTER TABLE token_upload ENABLE ROW LEVEL SECURITY;
ALTER TABLE token_upload FORCE ROW LEVEL SECURITY;
DO $$
BEGIN
    EXECUTE format('CREATE POLICY owner_all ON token_upload TO %I USING (true) WITH CHECK (true)', current_user);
END
$$;
CREATE POLICY app_tenant ON token_upload TO kura_app
    USING (tenant_id = kura_current_tenant()) WITH CHECK (tenant_id = kura_current_tenant());
GRANT SELECT, INSERT, UPDATE ON token_upload TO kura_app;

-- Kept 24 hours: long enough for an upload to reach its commit (T6).
CREATE FUNCTION kura_token_upload_purge() RETURNS bigint
    LANGUAGE sql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        WITH gone AS (DELETE FROM token_upload WHERE created_at < now() - interval '24 hours' RETURNING 1)
        SELECT count(*) FROM gone
    $$;
REVOKE ALL ON FUNCTION kura_token_upload_purge() FROM PUBLIC;
GRANT EXECUTE ON FUNCTION kura_token_upload_purge() TO kura_app;

-- The tenants holding live write tokens: `GET /v1/admin/status` warns when
-- one of them has `scopedTokens.write` off, where those writes are refused
-- (RFC 0004 §4.3).
CREATE FUNCTION kura_write_token_tenants() RETURNS TABLE (slug text, tokens bigint)
    LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        SELECT t.slug, count(*)
        FROM scoped_token s JOIN tenant t ON t.id = s.tenant_id
        WHERE s.mode = 'write' AND s.revoked_at IS NULL AND s.expires_at > now() AND t.disabled_at IS NULL
        GROUP BY t.slug
    $$;
REVOKE ALL ON FUNCTION kura_write_token_tenants() FROM PUBLIC;
GRANT EXECUTE ON FUNCTION kura_write_token_tenants() TO kura_app;
