-- PR 7.1: scoped tokens for non-interactive readers (RFC 0001 §6.2, T5).
-- Only the secret's SHA-256 is stored. A token is revoked with the grant
-- that let its creator issue it: a binding (the foreign key empties when
-- the binding goes), or a tenant role through a claim value (checked
-- against the reconciled configuration at each use).
CREATE TABLE scoped_token (
    id                 uuid PRIMARY KEY,
    tenant_id          uuid NOT NULL,
    vault_id           uuid NOT NULL,
    path_prefix        text NOT NULL DEFAULT '' CHECK (octet_length(path_prefix) <= 4096),
    mode               text NOT NULL DEFAULT 'read' CHECK (mode = 'read'),
    allowed_cidrs      cidr[] NOT NULL CHECK (cardinality(allowed_cidrs) BETWEEN 1 AND 32),
    expires_at         timestamptz NOT NULL,
    token_hash         bytea NOT NULL CHECK (octet_length(token_hash) = 32),
    created_by         text NOT NULL,
    granted_by_binding uuid REFERENCES role_binding (id) ON DELETE SET NULL,
    granted_by_claim   text,
    created_at         timestamptz NOT NULL DEFAULT now(),
    last_used_at       timestamptz,
    revoked_at         timestamptz,
    CHECK (expires_at <= created_at + interval '90 days'),
    FOREIGN KEY (vault_id, tenant_id) REFERENCES vault (id, tenant_id) ON DELETE CASCADE
);
CREATE INDEX scoped_token_vault ON scoped_token (vault_id);
CREATE INDEX scoped_token_tenant ON scoped_token (tenant_id);
CREATE INDEX scoped_token_expires ON scoped_token (expires_at);
ALTER TABLE scoped_token ENABLE ROW LEVEL SECURITY;
ALTER TABLE scoped_token FORCE ROW LEVEL SECURITY;
DO $$
BEGIN
    EXECUTE format('CREATE POLICY owner_all ON scoped_token TO %I USING (true) WITH CHECK (true)', current_user);
END
$$;
CREATE POLICY app_tenant ON scoped_token TO kura_app
    USING (tenant_id = kura_current_tenant()) WITH CHECK (tenant_id = kura_current_tenant());
GRANT SELECT, INSERT, UPDATE ON scoped_token TO kura_app;

-- The presented token names its id before any tenant is known (T4): one
-- named lookup, which returns the hash to compare and the grant to check.
-- `binding_manager` is whether the granting binding still exists with the
-- manager role.
CREATE FUNCTION kura_scoped_token_lookup(p_id uuid)
    RETURNS TABLE (tenant_id uuid, vault_id uuid, path_prefix text, allowed_cidrs text[],
                   expires_at timestamptz, revoked_at timestamptz, token_hash bytea,
                   granted_by_claim text, binding_manager boolean)
    LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        SELECT t.tenant_id, t.vault_id, t.path_prefix, t.allowed_cidrs::text[], t.expires_at, t.revoked_at,
               t.token_hash, t.granted_by_claim,
               EXISTS (SELECT 1 FROM role_binding b WHERE b.id = t.granted_by_binding AND b.role = 'manager')
        FROM scoped_token t WHERE t.id = p_id
    $$;

-- last_used_at, at most once a minute (§6.2).
CREATE FUNCTION kura_scoped_token_touch(p_id uuid) RETURNS void
    LANGUAGE sql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        UPDATE scoped_token SET last_used_at = now()
        WHERE id = p_id AND (last_used_at IS NULL OR last_used_at < now() - interval '1 minute')
    $$;

-- Kept 30 days past expiry, for the audit trail (§9), then purged daily.
CREATE FUNCTION kura_scoped_token_purge() RETURNS bigint
    LANGUAGE sql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        WITH gone AS (DELETE FROM scoped_token WHERE expires_at < now() - interval '30 days' RETURNING 1)
        SELECT count(*) FROM gone
    $$;

REVOKE ALL ON FUNCTION kura_scoped_token_lookup(uuid), kura_scoped_token_touch(uuid), kura_scoped_token_purge() FROM PUBLIC;
GRANT EXECUTE ON FUNCTION kura_scoped_token_lookup(uuid), kura_scoped_token_touch(uuid), kura_scoped_token_purge() TO kura_app;
