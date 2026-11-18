-- RFC 0006 §18 D13: a device's owner by name, not by the identity provider's
-- opaque `sub`. The owner's own `name` (else `preferred_username`) claim,
-- written at registration and refreshed with `last_seen` (at most once a
-- minute), so a rename follows. Shown only where the owner already is.
ALTER TABLE device ADD COLUMN owner_name text CHECK (char_length(owner_name) BETWEEN 1 AND 128);

DROP FUNCTION kura_tenant_devices();
CREATE FUNCTION kura_tenant_devices()
    RETURNS TABLE (id uuid, user_sub text, owner_name text, name text, profile text, created_at timestamptz, last_seen timestamptz, revoked_at timestamptz)
    LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        SELECT d.id, d.user_sub, d.owner_name, d.name, d.profile, d.created_at, d.last_seen, d.revoked_at
        FROM device d JOIN device_tenant l ON l.device_id = d.id
        WHERE l.tenant_id = kura_current_tenant() AND d.signed_out_at IS NULL
        ORDER BY d.created_at, d.id
        LIMIT 1000
    $$;
REVOKE ALL ON FUNCTION kura_tenant_devices() FROM PUBLIC;
GRANT EXECUTE ON FUNCTION kura_tenant_devices() TO kura_app;

DROP FUNCTION kura_vault_sync(uuid, boolean);
CREATE FUNCTION kura_vault_sync(p_vault uuid, p_all boolean)
    RETURNS TABLE (device_id uuid, name text, profile text, user_sub text, owner_name text, revoked boolean,
                   epoch uuid, caught_up_seq bigint, caught_up_at timestamptz,
                   pushed_at timestamptz, behind bigint,
                   pending integer, conflicts integer, unsyncable integer, failed integer,
                   reported_at timestamptz)
    LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        SELECT d.id, d.name, d.profile, d.user_sub, d.owner_name, d.revoked_at IS NOT NULL,
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
