-- PR 7.3: opt-in search (RFC 0001 §6.7, decision 39). The index holds the
-- words of each Markdown head in plaintext, in this database and its dumps:
-- a tenant has none unless `search.enabled`. Both extensions are trusted, so
-- the database owner creates them.
CREATE EXTENSION IF NOT EXISTS unaccent;
CREATE EXTENSION IF NOT EXISTS pg_trgm;

-- French and English with accents folded (§6.7).
CREATE TEXT SEARCH CONFIGURATION kura_french (COPY = pg_catalog.french);
ALTER TEXT SEARCH CONFIGURATION kura_french
    ALTER MAPPING FOR hword, hword_part, word WITH unaccent, french_stem;
CREATE TEXT SEARCH CONFIGURATION kura_english (COPY = pg_catalog.english);
ALTER TEXT SEARCH CONFIGURATION kura_english
    ALTER MAPPING FOR hword, hword_part, word WITH unaccent, english_stem;

-- The head revision's lexemes only, one row per entry (§6.1).
CREATE TABLE search_doc (
    vault_id    uuid NOT NULL,
    tenant_id   uuid NOT NULL,
    entry_id    uuid NOT NULL,
    revision_id uuid NOT NULL,
    lex_fr      tsvector NOT NULL,
    lex_en      tsvector NOT NULL,
    PRIMARY KEY (vault_id, entry_id),
    FOREIGN KEY (vault_id, tenant_id) REFERENCES vault (id, tenant_id) ON DELETE CASCADE
);
CREATE INDEX search_doc_fr ON search_doc USING gin (lex_fr);
CREATE INDEX search_doc_en ON search_doc USING gin (lex_en);
CREATE INDEX search_doc_tenant ON search_doc (tenant_id);
-- Fuzzy file names (§6.7).
CREATE INDEX entry_path_trgm ON entry USING gin (path gin_trgm_ops);

-- One pending job per entry, inserted by the commit that changed it; a
-- newer commit replaces it. Claimed for 5 minutes at a time, failed after
-- 5 attempts and kept (counted in kura_index_queue_depth, §6.1).
CREATE TABLE index_job (
    vault_id    uuid NOT NULL,
    tenant_id   uuid NOT NULL,
    entry_id    uuid NOT NULL,
    revision_id uuid NOT NULL,
    attempts    integer NOT NULL DEFAULT 0,
    claimed_at  timestamptz,
    failed_at   timestamptz,
    created_at  timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (vault_id, entry_id),
    FOREIGN KEY (vault_id, tenant_id) REFERENCES vault (id, tenant_id) ON DELETE CASCADE
);
CREATE INDEX index_job_pending ON index_job (created_at) WHERE failed_at IS NULL;
CREATE INDEX index_job_tenant ON index_job (tenant_id);

-- Whether a tenant's index matches its setting: backfilled once enabled,
-- dropped once disabled.
ALTER TABLE tenant ADD COLUMN search_indexed boolean NOT NULL DEFAULT false;

DO $$
DECLARE
    t text;
BEGIN
    FOREACH t IN ARRAY ARRAY['search_doc', 'index_job'] LOOP
        EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY', t);
        EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY', t);
        EXECUTE format('CREATE POLICY owner_all ON %I TO %I USING (true) WITH CHECK (true)', t, current_user);
        EXECUTE format('CREATE POLICY app_tenant ON %I TO kura_app USING (tenant_id = kura_current_tenant()) WITH CHECK (tenant_id = kura_current_tenant())', t);
    END LOOP;
END
$$;
GRANT SELECT ON search_doc TO kura_app;
GRANT SELECT, INSERT, UPDATE ON index_job TO kura_app;

-- The worker's view of the tenants: backfill the newly enabled, drop the
-- newly disabled. Returns how many jobs were queued and docs dropped.
CREATE FUNCTION kura_search_sync() RETURNS TABLE (queued bigint, dropped bigint)
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
            WHERE e.tenant_id = t.id AND NOT e.deleted AND v.disabled_at IS NULL AND lower(e.path) LIKE '%.md'
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

-- Up to p_limit pending jobs (§6.7: 100 a batch), claimed for 5 minutes so
-- two replicas do not do the same work, with what indexing needs.
CREATE FUNCTION kura_index_claim(p_limit integer)
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
        -- The entry's current head, whatever revision the job named.
        SELECT c.vault_id, c.tenant_id, t.slug, c.entry_id, e.head_revision, e.path, e.deleted, r.blob_hash, r.size
        FROM claimed c
        JOIN tenant t ON t.id = c.tenant_id
        JOIN entry e ON e.vault_id = c.vault_id AND e.id = c.entry_id
        JOIN revision r ON r.id = e.head_revision
    $$;

-- The document of one head, or none (deleted, not Markdown): the job goes
-- unless a newer commit replaced it meanwhile. Only for a claimed job and
-- the entry's current head, so the serving role cannot write words or drop
-- documents of an arbitrary vault through it (review 7 L4).
CREATE FUNCTION kura_index_set(p_vault uuid, p_entry uuid, p_revision uuid, p_text text) RETURNS void
    LANGUAGE plpgsql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
    BEGIN
        IF NOT EXISTS (SELECT 1 FROM index_job j WHERE j.vault_id = p_vault AND j.entry_id = p_entry AND j.claimed_at IS NOT NULL)
           OR NOT EXISTS (SELECT 1 FROM entry e WHERE e.vault_id = p_vault AND e.id = p_entry AND e.head_revision = p_revision) THEN
            RETURN;
        END IF;
        IF p_text IS NULL THEN
            DELETE FROM search_doc WHERE vault_id = p_vault AND entry_id = p_entry;
        ELSE
            INSERT INTO search_doc (vault_id, tenant_id, entry_id, revision_id, lex_fr, lex_en)
            SELECT p_vault, v.tenant_id, p_entry, p_revision,
                   to_tsvector('kura_french', p_text), to_tsvector('kura_english', p_text)
            FROM vault v WHERE v.id = p_vault
            ON CONFLICT (vault_id, entry_id) DO UPDATE
                SET revision_id = EXCLUDED.revision_id, lex_fr = EXCLUDED.lex_fr, lex_en = EXCLUDED.lex_en;
        END IF;
        DELETE FROM index_job j
        WHERE j.vault_id = p_vault AND j.entry_id = p_entry
          AND (j.revision_id = p_revision OR NOT EXISTS (
                SELECT 1 FROM entry e WHERE e.vault_id = p_vault AND e.id = p_entry AND e.head_revision <> p_revision));
    END
    $$;

-- A job that failed: retried, at most 5 attempts, then kept as failed.
CREATE FUNCTION kura_index_failed(p_vault uuid, p_entry uuid) RETURNS void
    LANGUAGE sql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        UPDATE index_job SET attempts = attempts + 1, claimed_at = NULL,
               failed_at = CASE WHEN attempts + 1 >= 5 THEN now() END
        WHERE vault_id = p_vault AND entry_id = p_entry AND claimed_at IS NOT NULL
    $$;

CREATE FUNCTION kura_index_depth() RETURNS TABLE (pending bigint, failed bigint)
    LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        SELECT count(*) FILTER (WHERE failed_at IS NULL), count(*) FILTER (WHERE failed_at IS NOT NULL)
        FROM index_job
    $$;

REVOKE ALL ON FUNCTION kura_search_sync(), kura_index_claim(integer), kura_index_set(uuid, uuid, uuid, text),
    kura_index_failed(uuid, uuid), kura_index_depth() FROM PUBLIC;
GRANT EXECUTE ON FUNCTION kura_search_sync(), kura_index_claim(integer), kura_index_set(uuid, uuid, uuid, text),
    kura_index_failed(uuid, uuid), kura_index_depth() TO kura_app;
