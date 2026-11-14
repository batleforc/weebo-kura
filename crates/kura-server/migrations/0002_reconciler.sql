-- PR 1.5: the reconciler's writes (RFC 0001 §6.10). The server reconciles
-- every tenant at once, so each write is a named SECURITY DEFINER function
-- (T4 residual), tested in tests/reconcile.rs. Additive only.

-- The removal policy an object was last declared with: the annotation and the
-- policy leave with the document, so the row remembers them.
ALTER TABLE tenant ADD COLUMN force_orphan boolean NOT NULL DEFAULT false;
ALTER TABLE vault ADD COLUMN force_orphan boolean NOT NULL DEFAULT false;

-- "1.10.2" -> {1,10,2}: versions compare numerically, never as text.
CREATE FUNCTION kura_version(p text) RETURNS int[]
    LANGUAGE sql IMMUTABLE
    AS $$ SELECT string_to_array(split_part(split_part(p, '-', 1), '+', 1), '.')::int[] $$;

-- Takes the transaction-scoped lock and says whether this replica may write
-- (§6.10): never when the stored writer is newer or the generation is the
-- same; always when the stored writer is older or nothing is stored; for an
-- equal version, only when this replica's own files changed (p_local_change),
-- so a replica whose mount lags behind never reverts a fresher one.
CREATE FUNCTION kura_config_begin(p_generation bytea, p_version text, p_local_change boolean) RETURNS boolean
    LANGUAGE plpgsql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
DECLARE
    stored record;
BEGIN
    PERFORM pg_advisory_xact_lock(hashtext('kura.config_state'));
    SELECT generation, writer_version INTO stored FROM config_state ORDER BY id DESC LIMIT 1;
    IF NOT FOUND THEN
        RETURN true;
    END IF;
    IF stored.generation = p_generation OR kura_version(stored.writer_version) > kura_version(p_version) THEN
        RETURN false;
    END IF;
    IF kura_version(stored.writer_version) < kura_version(p_version) THEN
        RETURN true;
    END IF;
    RETURN p_local_change;
END
$$;

-- What validation needs from the stored state (kura_api::PreviousState).
CREATE FUNCTION kura_config_previous()
    RETURNS TABLE (kind text, namespace text, name text, chunking jsonb, managed_by text, disabled boolean)
    LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        SELECT 'tenant', NULL, slug, NULL, 'config', disabled_at IS NOT NULL FROM tenant
        UNION ALL
        SELECT 'vault', namespace, name, chunking_params, managed_by, disabled_at IS NOT NULL FROM vault
    $$;

-- Enabled tenants, so the authorization cache can map a slug to its id.
CREATE FUNCTION kura_tenant_ids() RETURNS TABLE (slug text, id uuid)
    LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$ SELECT slug, id FROM tenant WHERE disabled_at IS NULL $$;

-- Declares a tenant. A returning document cancels a scheduled deletion.
CREATE FUNCTION kura_tenant_upsert(
    p_slug text, p_claim_mappings jsonb, p_vault_namespaces text[], p_retention jsonb,
    p_quota_bytes bigint, p_max_file_size bigint, p_search boolean, p_deletion_policy text, p_force_orphan boolean
) RETURNS uuid
    LANGUAGE sql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        INSERT INTO tenant (slug, claim_mappings, vault_namespaces, retention_policy, quota_bytes, max_file_size,
                            search_enabled, deletion_policy, force_orphan)
        VALUES (p_slug, p_claim_mappings, p_vault_namespaces, p_retention, p_quota_bytes, p_max_file_size,
                p_search, p_deletion_policy, p_force_orphan)
        ON CONFLICT (slug) DO UPDATE SET
            claim_mappings = EXCLUDED.claim_mappings, vault_namespaces = EXCLUDED.vault_namespaces,
            retention_policy = EXCLUDED.retention_policy, quota_bytes = EXCLUDED.quota_bytes,
            max_file_size = EXCLUDED.max_file_size, search_enabled = EXCLUDED.search_enabled,
            deletion_policy = EXCLUDED.deletion_policy, force_orphan = EXCLUDED.force_orphan,
            disabled_at = NULL, delete_after = NULL
        RETURNING id
    $$;

-- Declares a file-managed vault. The chunking is written at creation only
-- (validation refuses a change); an API-managed vault is never taken over.
CREATE FUNCTION kura_vault_upsert(
    p_tenant text, p_namespace text, p_name text, p_chunking jsonb, p_config_sync jsonb,
    p_deletion_policy text, p_force_orphan boolean
) RETURNS uuid
    LANGUAGE sql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        INSERT INTO vault (tenant_id, namespace, name, chunking_params, config_sync, managed_by, deletion_policy, force_orphan)
        SELECT id, p_namespace, p_name, p_chunking, p_config_sync, 'config', p_deletion_policy, p_force_orphan
        FROM tenant WHERE slug = p_tenant
        ON CONFLICT (namespace, name) DO UPDATE SET
            config_sync = EXCLUDED.config_sync, deletion_policy = EXCLUDED.deletion_policy,
            force_orphan = EXCLUDED.force_orphan, disabled_at = NULL, delete_after = NULL
            WHERE vault.managed_by = 'config' AND vault.tenant_id = EXCLUDED.tenant_id
        RETURNING id
    $$;

-- Replaces the file-managed bindings of a vault; API-managed ones stay.
-- p_bindings: [{"kind": "claim"|"user", "subject": "...", "role": "..."}].
CREATE FUNCTION kura_bindings_replace(p_vault uuid, p_bindings jsonb) RETURNS void
    LANGUAGE sql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        DELETE FROM role_binding WHERE vault_id = p_vault AND managed_by = 'config';
        INSERT INTO role_binding (tenant_id, vault_id, subject_kind, subject, role, managed_by)
            SELECT v.tenant_id, v.id, b->>'kind', b->>'subject', b->>'role', 'config'
            FROM vault v, jsonb_array_elements(p_bindings) b WHERE v.id = p_vault
            ON CONFLICT (vault_id, subject_kind, subject) DO NOTHING;
    $$;

-- Disables the file-managed objects no longer declared. `Delete` schedules
-- the purge 30 days out (phase 5 runs it); `Orphan`, or the force-orphan
-- annotation, keeps the data with no deletion date. Returns what changed.
CREATE FUNCTION kura_config_disable_missing(p_tenants text[], p_vaults jsonb)
    RETURNS TABLE (kind text, namespace text, name text, delete_after timestamptz)
    LANGUAGE sql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        WITH t AS (
            UPDATE tenant SET disabled_at = now(),
                delete_after = CASE WHEN deletion_policy = 'Delete' AND NOT force_orphan THEN now() + interval '30 days' END
            WHERE disabled_at IS NULL AND slug <> ALL (p_tenants)
            RETURNING 'tenant'::text, NULL::text, slug, delete_after
        ), v AS (
            UPDATE vault SET disabled_at = now(),
                delete_after = CASE WHEN deletion_policy = 'Delete' AND NOT force_orphan THEN now() + interval '30 days' END
            WHERE managed_by = 'config' AND disabled_at IS NULL
              AND NOT EXISTS (SELECT 1 FROM jsonb_array_elements(p_vaults) k
                              WHERE k->>'namespace' = vault.namespace AND k->>'name' = vault.name)
            RETURNING 'vault'::text, namespace, name, delete_after
        )
        SELECT * FROM t UNION ALL SELECT * FROM v
    $$;

-- Records the reconciled state, last of the transaction, and drops history
-- older than 30 days (§6.1), always keeping the current row.
CREATE FUNCTION kura_config_state_insert(p_generation bytea, p_version text, p_documents jsonb, p_status jsonb)
    RETURNS bigint
    LANGUAGE sql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        DELETE FROM config_state WHERE written_at < now() - interval '30 days'
            AND id <> (SELECT max(id) FROM config_state);
        INSERT INTO config_state (generation, writer_version, documents, status)
            VALUES (p_generation, p_version, p_documents, p_status)
            RETURNING id;
    $$;

DO $$
DECLARE
    f text;
BEGIN
    FOREACH f IN ARRAY ARRAY[
        'kura_config_begin(bytea, text, boolean)', 'kura_config_previous()', 'kura_tenant_ids()',
        'kura_tenant_upsert(text, jsonb, text[], jsonb, bigint, bigint, boolean, text, boolean)',
        'kura_vault_upsert(text, text, text, jsonb, jsonb, text, boolean)',
        'kura_bindings_replace(uuid, jsonb)', 'kura_config_disable_missing(text[], jsonb)',
        'kura_config_state_insert(bytea, text, jsonb, jsonb)'
    ] LOOP
        EXECUTE format('REVOKE ALL ON FUNCTION %s FROM PUBLIC', f);
        EXECUTE format('GRANT EXECUTE ON FUNCTION %s TO kura_app', f);
    END LOOP;
END
$$;
