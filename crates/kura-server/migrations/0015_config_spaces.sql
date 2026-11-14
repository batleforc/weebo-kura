-- Phase 8: Obsidian configuration per user and per device profile (RFC 0001
-- §6.8, decision 34, T14). An entry lives in the note namespace, or in a
-- configuration namespace owned by one user: `config:desktop` or
-- `config:mobile`. Row-level security hides another user's configuration
-- from every request, whatever query it runs (§13 says no new policy lands
-- on an existing write path; this one only narrows reads and writes to the
-- caller's own rows, plan D69).
ALTER TABLE entry DROP CONSTRAINT entry_namespace_check;
ALTER TABLE entry ADD COLUMN owner_sub text;
ALTER TABLE entry ADD CONSTRAINT entry_namespace_check CHECK (
    namespace IN ('notes', 'config:desktop', 'config:mobile')
    AND (namespace = 'notes') = (owner_sub IS NULL)
);

-- Unique per namespace and owner (§6.3).
DROP INDEX entry_path;
CREATE UNIQUE INDEX entry_path ON entry (vault_id, namespace, coalesce(owner_sub, ''), path_key) WHERE NOT deleted;

-- A configuration entry is visible to its owner only: the transaction's
-- subject (`kura.subject`), never set for a scoped token or a job.
DROP POLICY app_tenant ON entry;
CREATE POLICY app_tenant ON entry TO kura_app
    USING (tenant_id = kura_current_tenant() AND (owner_sub IS NULL OR owner_sub = kura_current_subject()))
    WITH CHECK (tenant_id = kura_current_tenant() AND (owner_sub IS NULL OR owner_sub = kura_current_subject()));

-- Where entry ids live, whoever owns them: a commit naming an id of another
-- namespace or owner is refused rather than written over (review 8).
CREATE FUNCTION kura_entry_home(p_vault uuid, p_ids uuid[])
    RETURNS TABLE (id uuid, namespace text, owner_sub text)
    LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$ SELECT e.id, e.namespace, e.owner_sub FROM entry e WHERE e.vault_id = p_vault AND e.id = ANY (p_ids) $$;
REVOKE ALL ON FUNCTION kura_entry_home(uuid, uuid[]) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION kura_entry_home(uuid, uuid[]) TO kura_app;

-- Search never indexes configuration (plugin READMEs are Markdown too).
CREATE OR REPLACE FUNCTION kura_search_sync() RETURNS TABLE (queued bigint, dropped bigint)
    LANGUAGE plpgsql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
    DECLARE
        q bigint := 0;
        d bigint := 0;
        n bigint;
        t record;
    BEGIN
        FOR t IN SELECT id FROM tenant WHERE search_enabled AND NOT search_indexed AND disabled_at IS NULL FOR UPDATE SKIP LOCKED LOOP
            INSERT INTO index_job (vault_id, tenant_id, entry_id, revision_id)
            SELECT e.vault_id, e.tenant_id, e.id, e.head_revision
            FROM entry e JOIN vault v ON v.id = e.vault_id
            WHERE e.tenant_id = t.id AND e.namespace = 'notes' AND NOT e.deleted
              AND v.disabled_at IS NULL AND lower(e.path) LIKE '%.md'
            ON CONFLICT (vault_id, entry_id) DO NOTHING;
            GET DIAGNOSTICS n = ROW_COUNT;
            q := q + n;
            UPDATE tenant SET search_indexed = true WHERE id = t.id;
        END LOOP;
        FOR t IN SELECT id FROM tenant WHERE NOT search_enabled AND search_indexed FOR UPDATE SKIP LOCKED LOOP
            DELETE FROM index_job WHERE tenant_id = t.id;
            DELETE FROM search_doc WHERE tenant_id = t.id;
            GET DIAGNOSTICS n = ROW_COUNT;
            d := d + n;
            UPDATE tenant SET search_indexed = false WHERE id = t.id;
        END LOOP;
        RETURN QUERY SELECT q, d;
    END
    $$;

CREATE OR REPLACE FUNCTION kura_index_claim(p_limit integer)
    RETURNS TABLE (vault_id uuid, tenant_id uuid, slug text, entry_id uuid, revision_id uuid,
                   path text, deleted boolean, blob_hash bytea, size bigint)
    LANGUAGE sql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        WITH c AS (
            SELECT j.vault_id, j.entry_id FROM index_job j
            WHERE j.failed_at IS NULL AND (j.claimed_at IS NULL OR j.claimed_at < now() - interval '5 minutes')
            ORDER BY j.created_at
            LIMIT least(p_limit, 100)
            FOR UPDATE SKIP LOCKED
        ), claimed AS (
            UPDATE index_job j SET claimed_at = now() FROM c
            WHERE j.vault_id = c.vault_id AND j.entry_id = c.entry_id
            RETURNING j.vault_id, j.tenant_id, j.entry_id
        )
        -- The entry's current head; a configuration entry indexes as nothing.
        SELECT c.vault_id, c.tenant_id, t.slug, c.entry_id, e.head_revision, e.path,
               e.deleted OR e.namespace <> 'notes', r.blob_hash, r.size
        FROM claimed c
        JOIN tenant t ON t.id = c.tenant_id
        JOIN entry e ON e.vault_id = c.vault_id AND e.id = c.entry_id
        JOIN revision r ON r.id = e.head_revision
    $$;
