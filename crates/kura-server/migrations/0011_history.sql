-- Phase 5: epochs, retention, garbage collection and the Delete purge
-- (RFC 0001 §6.3, §6.9, decisions 24, 32, 33). Additive only. Every job
-- crosses tenants, so each runs through a named definer function, in batches.

-- PR 5.1: the cluster the vaults' epochs were issued under. PostgreSQL's
-- system identifier changes on a restore into a new cluster, its timeline on
-- every promotion of a standby: either way history may have been lost, so
-- every vault gets a new epoch and devices resync without deleting (§6.3).
CREATE TABLE cluster_identity (
    id                boolean PRIMARY KEY DEFAULT true CHECK (id),
    system_identifier bigint  NOT NULL,
    timeline          integer NOT NULL,
    checked_at        timestamptz NOT NULL DEFAULT now()
);
ALTER TABLE cluster_identity ENABLE ROW LEVEL SECURITY;
ALTER TABLE cluster_identity FORCE ROW LEVEL SECURITY;
DO $$
BEGIN
    EXECUTE format('CREATE POLICY owner_all ON cluster_identity TO %I USING (true) WITH CHECK (true)', current_user);
END
$$;

-- Records the identity; when it changed, gives every vault a new epoch and
-- returns how many. The first record changes nothing. Two replicas racing:
-- the second waits on the row, then finds it current.
CREATE FUNCTION kura_epoch_check() RETURNS bigint
    LANGUAGE plpgsql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
    DECLARE
        sysid bigint;
        -- The timeline of the WAL being written, current from the promotion
        -- on (the checkpoint's lags until the next checkpoint).
        tl integer := ('x' || substr(pg_walfile_name(pg_current_wal_lsn()), 1, 8))::bit(32)::integer;
        updated boolean;
        n bigint := 0;
    BEGIN
        SELECT system_identifier INTO sysid FROM pg_control_system();
        INSERT INTO cluster_identity (system_identifier, timeline) VALUES (sysid, tl)
        ON CONFLICT (id) DO UPDATE
            SET system_identifier = EXCLUDED.system_identifier, timeline = EXCLUDED.timeline, checked_at = now()
            WHERE cluster_identity.system_identifier <> EXCLUDED.system_identifier
               OR cluster_identity.timeline <> EXCLUDED.timeline
        RETURNING xmax::text <> '0' INTO updated;
        IF updated THEN
            UPDATE vault SET epoch = gen_random_uuid();
            GET DIAGNOSTICS n = ROW_COUNT;
        END IF;
        RETURN n;
    END
    $$;

-- The restore runbook's explicit step (decision 32), run as the owner.
CREATE FUNCTION kura_epoch_reset() RETURNS bigint
    LANGUAGE plpgsql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
    DECLARE
        n bigint;
    BEGIN
        UPDATE vault SET epoch = gen_random_uuid();
        GET DIAGNOSTICS n = ROW_COUNT;
        RETURN n;
    END
    $$;

-- PR 5.3: every vault the maintenance pass visits, with its tenant's
-- retention policy; `due` when a Delete grace period has passed (decision 33).
CREATE FUNCTION kura_maintenance_vaults()
    RETURNS TABLE (vault_id uuid, retention jsonb, active boolean, due boolean)
    LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        SELECT v.id, t.retention_policy,
               v.disabled_at IS NULL AND t.disabled_at IS NULL,
               coalesce(v.delete_after <= now(), false)
                 OR (coalesce(t.delete_after <= now(), false) AND NOT v.force_orphan)
        FROM vault v JOIN tenant t ON t.id = v.tenant_id
        ORDER BY v.id
    $$;

-- Revisions past the policy, at most p_limit: older than p_max_age, or beyond
-- the newest p_max_per_entry of their entry. An entry's head is never pruned,
-- a deletion's included: deleted entries leave only through the purge below,
-- so every device sees the deletion first (§6.9). A policy below one hour or
-- one revision prunes nothing: no caller can pass one that empties history.
-- shortcut: each batch ranks the vault's whole history; walk revision_history
-- per entry if a vault's batch nears the 5 s statement_timeout.
CREATE FUNCTION kura_prune_revisions(p_vault uuid, p_max_age interval, p_max_per_entry integer, p_limit integer)
    RETURNS bigint
    LANGUAGE sql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        WITH ranked AS (
            SELECT r.id, r.created_at, e.head_revision,
                   row_number() OVER (PARTITION BY r.entry_id ORDER BY r.seq DESC) AS rn
            FROM revision r JOIN entry e ON e.vault_id = r.vault_id AND e.id = r.entry_id
            WHERE r.vault_id = p_vault
        ), doomed AS (
            SELECT id FROM ranked
            WHERE id <> head_revision
              AND ((p_max_per_entry >= 1 AND rn > p_max_per_entry)
                OR (p_max_age >= interval '1 hour' AND created_at < now() - p_max_age))
            LIMIT p_limit
        ), gone AS (
            DELETE FROM revision WHERE id IN (SELECT id FROM doomed) RETURNING 1
        )
        SELECT count(*) FROM gone
    $$;

-- Deleted entries whose deletion is older than p_ttl, at most p_limit, purged
-- with their history under the vault lock (no commit interleaves); the
-- horizon moves to the last of them, so an older cursor gets 410 (§6.3).
CREATE FUNCTION kura_purge_deleted(p_vault uuid, p_ttl interval, p_limit integer) RETURNS bigint
    LANGUAGE plpgsql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
    DECLARE
        ids uuid[];
        horizon bigint;
    BEGIN
        IF p_ttl IS NULL OR p_ttl < interval '1 day' THEN
            RETURN 0;
        END IF;
        PERFORM 1 FROM vault WHERE id = p_vault FOR UPDATE;
        SELECT array_agg(d.id), max(d.last_seq) INTO ids, horizon FROM (
            SELECT e.id, e.last_seq
            FROM entry e JOIN revision h ON h.id = e.head_revision
            WHERE e.vault_id = p_vault AND e.deleted AND h.created_at < now() - p_ttl
            ORDER BY e.last_seq
            LIMIT p_limit
        ) d;
        IF ids IS NULL THEN
            RETURN 0;
        END IF;
        DELETE FROM revision WHERE vault_id = p_vault AND entry_id = ANY (ids);
        DELETE FROM entry WHERE vault_id = p_vault AND id = ANY (ids);
        UPDATE vault SET horizon_seq = greatest(horizon_seq, horizon) WHERE id = p_vault;
        RETURN cardinality(ids);
    END
    $$;

-- PR 5.4: garbage collection (§6.9, decision 24). A blob PUT refreshes its
-- blob's last_touched, as `chunks/missing` does a chunk's, so a manifest
-- about to be committed is never collected.
ALTER TABLE blob ADD COLUMN last_touched timestamptz NOT NULL DEFAULT now();
CREATE INDEX blob_last_touched ON blob (vault_id, last_touched);
-- "Is this blob still referenced?"
CREATE INDEX revision_blob ON revision (vault_id, blob_hash);

-- Blobs no revision references, untouched 24 hours, locked so a commit
-- taking them (FOR KEY SHARE) is skipped, then rechecked in a new snapshot.
CREATE FUNCTION kura_gc_blobs(p_vault uuid, p_limit integer) RETURNS bigint
    LANGUAGE plpgsql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
    DECLARE
        hashes bytea[];
        n bigint;
    BEGIN
        SELECT array_agg(hash) INTO hashes FROM (
            SELECT b.hash FROM blob b
            WHERE b.vault_id = p_vault AND b.last_touched < now() - interval '24 hours'
              AND NOT EXISTS (SELECT 1 FROM revision r WHERE r.vault_id = b.vault_id AND r.blob_hash = b.hash)
            ORDER BY b.last_touched
            LIMIT p_limit
            FOR UPDATE SKIP LOCKED
        ) c;
        IF hashes IS NULL THEN
            RETURN 0;
        END IF;
        DELETE FROM blob b
        WHERE b.vault_id = p_vault AND b.hash = ANY (hashes)
          AND NOT EXISTS (SELECT 1 FROM revision r WHERE r.vault_id = b.vault_id AND r.blob_hash = b.hash);
        GET DIAGNOSTICS n = ROW_COUNT;
        RETURN n;
    END
    $$;

-- Chunks no manifest references, untouched 24 hours: locked FOR UPDATE SKIP
-- LOCKED, so a blob PUT holding them FOR KEY SHARE is skipped and one
-- arriving later waits, then finds them gone and asks for a re-upload;
-- rechecked, deleted, and their object keys returned for the caller to
-- delete once this commits (decision 24).
CREATE FUNCTION kura_gc_chunks(p_vault uuid, p_limit integer) RETURNS SETOF text
    LANGUAGE plpgsql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
    DECLARE
        hashes bytea[];
    BEGIN
        SELECT array_agg(hash) INTO hashes FROM (
            SELECT c.hash FROM chunk c
            WHERE c.vault_id = p_vault AND c.last_touched < now() - interval '24 hours'
              AND NOT EXISTS (SELECT 1 FROM blob_chunk b WHERE b.vault_id = c.vault_id AND b.chunk_hash = c.hash)
            ORDER BY c.last_touched
            LIMIT p_limit
            FOR UPDATE SKIP LOCKED
        ) l;
        IF hashes IS NULL THEN
            RETURN;
        END IF;
        RETURN QUERY
            DELETE FROM chunk c
            WHERE c.vault_id = p_vault AND c.hash = ANY (hashes)
              AND NOT EXISTS (SELECT 1 FROM blob_chunk b WHERE b.vault_id = c.vault_id AND b.chunk_hash = c.hash)
            RETURNING c.object_key;
    END
    $$;

-- Which of these object keys a chunk row still names: the others, old
-- enough, are left by a crash between an object PUT and its row (§6.9).
CREATE FUNCTION kura_known_objects(p_vault uuid, p_keys text[]) RETURNS SETOF text
    LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        SELECT object_key FROM chunk WHERE vault_id = p_vault AND object_key = ANY (p_keys)
    $$;

-- PR 5.6: the Delete purge (decision 33). One batch of a vault's rows, at
-- most p_limit, table by table; 0 when only the vault row is left, -1 when
-- the vault is no longer due (its document came back): checked in every
-- batch, under a lock the reconciler's restore waits on.
ALTER TABLE tenant ADD COLUMN purged_at timestamptz;
-- Set by the first batch; every batch gives the vault a new epoch: if its
-- document comes back mid-purge, every device gets `history_rewound` and
-- pushes its copies back instead of trashing them as deleted elsewhere.
ALTER TABLE vault ADD COLUMN purge_started_at timestamptz;

CREATE FUNCTION kura_purge_vault(p_vault uuid, p_limit integer) RETURNS bigint
    LANGUAGE plpgsql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
    DECLARE
        n bigint;
        ids uuid[];
    BEGIN
        PERFORM 1 FROM vault v JOIN tenant t ON t.id = v.tenant_id
        WHERE v.id = p_vault
          AND (v.delete_after <= now() OR (t.delete_after <= now() AND NOT v.force_orphan))
        FOR UPDATE OF v FOR SHARE OF t;
        IF NOT FOUND THEN
            RETURN -1;
        END IF;
        -- Every batch, so a vault removed, restored and removed again is
        -- covered on each cycle; it is disabled, so the churn costs nothing.
        UPDATE vault SET purge_started_at = coalesce(purge_started_at, now()), epoch = gen_random_uuid()
        WHERE id = p_vault;
        DELETE FROM commit_record WHERE ctid = ANY (ARRAY(SELECT ctid FROM commit_record WHERE vault_id = p_vault LIMIT p_limit));
        GET DIAGNOSTICS n = ROW_COUNT;
        IF n > 0 THEN RETURN n; END IF;
        DELETE FROM ws_ticket WHERE vault_id = p_vault;
        -- An entry leaves with its whole history in one batch: a vault
        -- restored mid-purge never holds an entry without its head.
        SELECT array_agg(id) INTO ids FROM (SELECT id FROM entry WHERE vault_id = p_vault LIMIT p_limit) e;
        IF ids IS NOT NULL THEN
            DELETE FROM revision WHERE vault_id = p_vault AND entry_id = ANY (ids);
            DELETE FROM entry WHERE vault_id = p_vault AND id = ANY (ids);
            RETURN cardinality(ids);
        END IF;
        DELETE FROM blob WHERE (vault_id, hash) IN (SELECT vault_id, hash FROM blob WHERE vault_id = p_vault LIMIT p_limit);
        GET DIAGNOSTICS n = ROW_COUNT;
        IF n > 0 THEN RETURN n; END IF;
        DELETE FROM chunk WHERE (vault_id, hash) IN (SELECT vault_id, hash FROM chunk WHERE vault_id = p_vault LIMIT p_limit);
        GET DIAGNOSTICS n = ROW_COUNT;
        RETURN n;
    END
    $$;

-- Once the rows and the objects are gone: the vault row (its bindings
-- cascade), and the tenant marked purged when it was its last vault. The
-- tenant row stays until an operator destroys its Transit key (decision 33).
CREATE FUNCTION kura_purge_vault_finish(p_vault uuid) RETURNS boolean
    LANGUAGE plpgsql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
    DECLARE
        tenant uuid;
        tenant_due boolean;
    BEGIN
        SELECT v.tenant_id, coalesce(t.delete_after <= now(), false) INTO tenant, tenant_due
        FROM vault v JOIN tenant t ON t.id = v.tenant_id
        WHERE v.id = p_vault
          AND (v.delete_after <= now() OR (t.delete_after <= now() AND NOT v.force_orphan))
        FOR UPDATE OF v FOR SHARE OF t;
        IF NOT FOUND OR EXISTS (SELECT 1 FROM chunk WHERE vault_id = p_vault) THEN
            RETURN false;
        END IF;
        DELETE FROM vault WHERE id = p_vault;
        IF tenant_due AND NOT EXISTS (SELECT 1 FROM vault WHERE tenant_id = tenant) THEN
            UPDATE tenant SET purged_at = now() WHERE id = tenant AND purged_at IS NULL;
        END IF;
        RETURN true;
    END
    $$;

-- Tenants purged by Delete whose Transit key an operator must still destroy.
CREATE FUNCTION kura_tenants_awaiting_key_destruction() RETURNS bigint
    LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$ SELECT count(*) FROM tenant WHERE purged_at IS NOT NULL AND disabled_at IS NOT NULL $$;

REVOKE ALL ON FUNCTION kura_epoch_check(), kura_epoch_reset(), kura_maintenance_vaults(),
    kura_prune_revisions(uuid, interval, integer, integer), kura_purge_deleted(uuid, interval, integer),
    kura_gc_blobs(uuid, integer), kura_gc_chunks(uuid, integer), kura_known_objects(uuid, text[]),
    kura_purge_vault(uuid, integer), kura_purge_vault_finish(uuid),
    kura_tenants_awaiting_key_destruction() FROM PUBLIC;
GRANT EXECUTE ON FUNCTION kura_epoch_check(), kura_maintenance_vaults(),
    kura_prune_revisions(uuid, interval, integer, integer), kura_purge_deleted(uuid, interval, integer),
    kura_gc_blobs(uuid, integer), kura_gc_chunks(uuid, integer), kura_known_objects(uuid, text[]),
    kura_purge_vault(uuid, integer), kura_purge_vault_finish(uuid),
    kura_tenants_awaiting_key_destruction() TO kura_app;
