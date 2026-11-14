-- PR 6.1: storage per tenant for the metrics of §10.1 (chunk_bytes_stored,
-- dedup_ratio, quota_used_ratio). A cross-tenant read through a named
-- function, as kura_tenant_usage. Sizes only: no name, no path.
CREATE FUNCTION kura_tenant_storage()
    RETURNS TABLE (slug text, stored bigint, logical bigint, quota bigint)
    LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        SELECT t.slug,
               coalesce((SELECT sum(c.size) FROM chunk c WHERE c.tenant_id = t.id), 0)::bigint,
               coalesce((SELECT sum(r.size) FROM entry e JOIN revision r ON r.id = e.head_revision
                         WHERE e.tenant_id = t.id AND NOT e.deleted), 0)::bigint,
               t.quota_bytes
        FROM tenant t
        WHERE t.disabled_at IS NULL
    $$;
-- Tenants and vaults removed with Delete, waiting for their grace period
-- (decision 33): an alert, so an unnoticed removal is noticed.
CREATE FUNCTION kura_deletions_scheduled() RETURNS bigint
    LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        SELECT (SELECT count(*) FROM tenant WHERE delete_after IS NOT NULL AND purged_at IS NULL)
             + (SELECT count(*) FROM vault WHERE delete_after IS NOT NULL)
    $$;
REVOKE ALL ON FUNCTION kura_tenant_storage(), kura_deletions_scheduled() FROM PUBLIC;
GRANT EXECUTE ON FUNCTION kura_tenant_storage(), kura_deletions_scheduled() TO kura_app;

-- PR 6.4: the restore drill's last step decrypts a random sample of chunks
-- (§10.3); returns what opening one needs, never content.
CREATE FUNCTION kura_sample_chunks(p_count integer)
    RETURNS TABLE (vault_id uuid, tenant_id uuid, slug text, hash bytea, object_key text)
    LANGUAGE sql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        SELECT c.vault_id, c.tenant_id, t.slug, c.hash, c.object_key
        FROM chunk c JOIN tenant t ON t.id = c.tenant_id
        ORDER BY random()
        LIMIT least(greatest(p_count, 0), 10000)
    $$;
-- Owner only: content addresses and object keys of every tenant never reach
-- the serving role; `check-chunks` runs with the owner's URL (no timeout).
REVOKE ALL ON FUNCTION kura_sample_chunks(integer) FROM PUBLIC;

-- PR 6.4: each completed bucket copy, so a missed or failing backup alerts
-- (kura_backup_last_success_timestamp_seconds). Written by `kura-server
-- backup` through the function below; read by the metrics refresh.
CREATE TABLE backup_run (
    recover_to  timestamptz PRIMARY KEY,
    objects     bigint NOT NULL,
    finished_at timestamptz NOT NULL DEFAULT now()
);
ALTER TABLE backup_run ENABLE ROW LEVEL SECURITY;
ALTER TABLE backup_run FORCE ROW LEVEL SECURITY;
DO $$
BEGIN
    EXECUTE format('CREATE POLICY owner_all ON backup_run TO %I USING (true) WITH CHECK (true)', current_user);
END
$$;

CREATE FUNCTION kura_backup_done(p_recover_to timestamptz, p_objects bigint) RETURNS void
    LANGUAGE sql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        INSERT INTO backup_run (recover_to, objects) VALUES (p_recover_to, p_objects)
        ON CONFLICT (recover_to) DO NOTHING;
        DELETE FROM backup_run WHERE finished_at < now() - interval '90 days';
    $$;

CREATE FUNCTION kura_backup_last() RETURNS timestamptz
    LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$ SELECT max(finished_at) FROM backup_run $$;
REVOKE ALL ON FUNCTION kura_backup_done(timestamptz, bigint), kura_backup_last() FROM PUBLIC;
GRANT EXECUTE ON FUNCTION kura_backup_done(timestamptz, bigint), kura_backup_last() TO kura_app;
