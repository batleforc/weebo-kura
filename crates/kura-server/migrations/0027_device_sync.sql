-- RFC 0006: where each device stands on each vault it syncs. One row per
-- (device, vault), overwritten, never a timeline (§3, §9). Written by the
-- server from the device's own calls; read through kura_vault_sync() only,
-- since `device` rows are visible to their owner alone.
CREATE TABLE device_sync (
    vault_id      uuid NOT NULL,
    tenant_id     uuid NOT NULL,
    device_id     uuid NOT NULL REFERENCES device (id) ON DELETE CASCADE,
    epoch         uuid,
    caught_up_seq bigint CHECK (caught_up_seq >= 0),
    caught_up_at  timestamptz,
    -- The LIKE patterns the device read with, so "behind" counts only what
    -- it may read (T5) without re-deriving another person's claims.
    scope         text[],
    pushed_at     timestamptz,
    PRIMARY KEY (vault_id, device_id),
    FOREIGN KEY (vault_id, tenant_id) REFERENCES vault (id, tenant_id) ON DELETE CASCADE
);
CREATE INDEX device_sync_tenant ON device_sync (tenant_id);
CREATE INDEX device_sync_device ON device_sync (device_id);
ALTER TABLE device_sync ENABLE ROW LEVEL SECURITY;
ALTER TABLE device_sync FORCE ROW LEVEL SECURITY;
DO $$
BEGIN
    EXECUTE format('CREATE POLICY owner_all ON device_sync TO %I USING (true) WITH CHECK (true)', current_user);
END
$$;
CREATE POLICY app_tenant ON device_sync TO kura_app
    USING (tenant_id = kura_current_tenant()) WITH CHECK (tenant_id = kura_current_tenant());
GRANT SELECT, INSERT, UPDATE ON device_sync TO kura_app;

-- The rows of one vault of the current tenant, with each device's name and
-- owner; p_all false keeps the current subject's own devices (§7.2). Behind
-- is counted under the device's own scope, capped at 1,001.
CREATE FUNCTION kura_vault_sync(p_vault uuid, p_all boolean)
    RETURNS TABLE (device_id uuid, name text, profile text, user_sub text, revoked boolean,
                   epoch uuid, caught_up_seq bigint, caught_up_at timestamptz,
                   pushed_at timestamptz, behind bigint)
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
               END
        FROM device_sync s JOIN device d ON d.id = s.device_id
        WHERE s.vault_id = p_vault AND s.tenant_id = kura_current_tenant()
          AND d.signed_out_at IS NULL
          AND (p_all OR d.user_sub = kura_current_subject())
        ORDER BY d.name, d.id
        LIMIT 1000
    $$;

-- The vaults each of the current subject's devices has caught up on.
CREATE FUNCTION kura_my_device_vaults()
    RETURNS TABLE (device_id uuid, vault_id uuid, caught_up_at timestamptz)
    LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        SELECT s.device_id, s.vault_id, s.caught_up_at
        FROM device_sync s JOIN device d ON d.id = s.device_id
        WHERE d.user_sub = kura_current_subject() AND s.caught_up_at IS NOT NULL
        ORDER BY s.device_id, s.vault_id
        LIMIT 5000
    $$;

-- A person unbound from a vault leaves its device list (question 3).
CREATE FUNCTION kura_device_sync_forget(p_vault uuid, p_subject text) RETURNS void
    LANGUAGE sql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        DELETE FROM device_sync s USING device d
        WHERE s.device_id = d.id AND s.vault_id = p_vault
          AND s.tenant_id = kura_current_tenant() AND d.user_sub = p_subject
    $$;

REVOKE ALL ON FUNCTION kura_vault_sync(uuid, boolean), kura_my_device_vaults(), kura_device_sync_forget(uuid, text) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION kura_vault_sync(uuid, boolean), kura_my_device_vaults(), kura_device_sync_forget(uuid, text) TO kura_app;
