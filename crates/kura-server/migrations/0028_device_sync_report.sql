-- RFC 0006 phase 3: what a device's last run left on its side, as counts
-- only (question 2). Overwritten by each report, like the rest of the row.
ALTER TABLE device_sync
    ADD COLUMN pending     integer CHECK (pending >= 0),
    ADD COLUMN conflicts   integer CHECK (conflicts >= 0),
    ADD COLUMN unsyncable  integer CHECK (unsyncable >= 0),
    ADD COLUMN failed      integer CHECK (failed >= 0),
    ADD COLUMN reported_at timestamptz;

-- The same function, returning the counts too (a new return type needs a drop).
DROP FUNCTION kura_vault_sync(uuid, boolean);
CREATE FUNCTION kura_vault_sync(p_vault uuid, p_all boolean)
    RETURNS TABLE (device_id uuid, name text, profile text, user_sub text, revoked boolean,
                   epoch uuid, caught_up_seq bigint, caught_up_at timestamptz,
                   pushed_at timestamptz, behind bigint,
                   pending integer, conflicts integer, unsyncable integer, failed integer,
                   reported_at timestamptz)
    LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        SELECT d.id, d.name, d.profile, d.user_sub, d.revoked_at IS NOT NULL,
               s.epoch, s.caught_up_seq, s.caught_up_at, s.pushed_at,
               CASE WHEN s.caught_up_seq IS NULL THEN NULL ELSE (
                   SELECT count(*) FROM (
                       SELECT 1 FROM entry e
                       WHERE e.vault_id = s.vault_id AND e.namespace = 'notes'
                         AND e.last_seq > s.caught_up_seq
                         AND e.path_key LIKE ANY (coalesce(s.scope, ARRAY['%']))
                       LIMIT 1001) b)
               END,
               s.pending, s.conflicts, s.unsyncable, s.failed, s.reported_at
        FROM device_sync s JOIN device d ON d.id = s.device_id
        WHERE s.vault_id = p_vault AND s.tenant_id = kura_current_tenant()
          AND d.signed_out_at IS NULL
          AND (p_all OR d.user_sub = kura_current_subject())
        ORDER BY d.name, d.id
        LIMIT 1000
    $$;
REVOKE ALL ON FUNCTION kura_vault_sync(uuid, boolean) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION kura_vault_sync(uuid, boolean) TO kura_app;
