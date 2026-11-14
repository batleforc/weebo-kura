-- PR 1.7: devices seen by a tenant admin (RFC 0001 §4.4: a tenant `admin` lists
-- and revokes the tenant's devices). A device belongs to a subject, not a
-- tenant (§6.1), so registration links it to the tenants its owner belongs to.
-- Additive only.

CREATE TABLE device_tenant (
    tenant_id uuid NOT NULL REFERENCES tenant (id),
    device_id uuid NOT NULL REFERENCES device (id) ON DELETE CASCADE,
    PRIMARY KEY (tenant_id, device_id)
);
CREATE INDEX device_tenant_device ON device_tenant (device_id);

ALTER TABLE device_tenant ENABLE ROW LEVEL SECURITY;
ALTER TABLE device_tenant FORCE ROW LEVEL SECURITY;
DO $$ BEGIN
    EXECUTE format('CREATE POLICY owner_all ON device_tenant TO %I USING (true) WITH CHECK (true)', current_user);
END $$;
CREATE POLICY app_tenant ON device_tenant FOR SELECT TO kura_app USING (tenant_id = kura_current_tenant());
GRANT SELECT ON device_tenant TO kura_app;

-- Sets the tenants of the caller's own device to those it belongs to now:
-- new links added, links to tenants it left removed.
CREATE FUNCTION kura_device_link(p_device uuid, p_tenants uuid[]) RETURNS void
    LANGUAGE sql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        DELETE FROM device_tenant l USING device d
            WHERE l.device_id = p_device AND d.id = p_device AND d.user_sub = kura_current_subject()
              AND l.tenant_id <> ALL (p_tenants);
        INSERT INTO device_tenant (tenant_id, device_id)
            SELECT t, d.id FROM device d, unnest(p_tenants) t
            WHERE d.id = p_device AND d.user_sub = kura_current_subject()
            ON CONFLICT DO NOTHING;
    $$;

-- The devices linked to the current tenant (a tenant admin's view). Links
-- are refreshed at most once a minute while a device is used (D27).
CREATE FUNCTION kura_tenant_devices()
    RETURNS TABLE (id uuid, user_sub text, name text, profile text, created_at timestamptz, last_seen timestamptz, revoked_at timestamptz)
    LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        SELECT d.id, d.user_sub, d.name, d.profile, d.created_at, d.last_seen, d.revoked_at
        FROM device d JOIN device_tenant l ON l.device_id = d.id
        WHERE l.tenant_id = kura_current_tenant()
        ORDER BY d.created_at, d.id
        LIMIT 1000
    $$;

-- Whether a subject is a known member of the current tenant: one of its
-- live devices is linked to it (check_binding, D29). Indexed, unbounded.
CREATE FUNCTION kura_tenant_has_member(p_sub text) RETURNS boolean
    LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        SELECT EXISTS (
            SELECT 1 FROM device_tenant l JOIN device d ON d.id = l.device_id
            WHERE l.tenant_id = kura_current_tenant() AND d.user_sub = p_sub AND d.revoked_at IS NULL
        )
    $$;

-- Revokes a device linked to the current tenant; returns it, or nothing.
CREATE FUNCTION kura_tenant_device_revoke(p_device uuid) RETURNS uuid
    LANGUAGE sql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        UPDATE device SET revoked_at = coalesce(revoked_at, now())
        WHERE id = p_device
          AND EXISTS (SELECT 1 FROM device_tenant l WHERE l.device_id = p_device AND l.tenant_id = kura_current_tenant())
        RETURNING id
    $$;

-- Daily purge (§6.1): devices idle 180 days go; revoked ones and their sid
-- stay, so a stolen refresh token still cannot register (decision 35).
CREATE FUNCTION kura_device_purge() RETURNS bigint
    LANGUAGE sql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        WITH gone AS (
            DELETE FROM device WHERE revoked_at IS NULL AND last_seen < now() - interval '180 days' RETURNING 1
        )
        SELECT count(*) FROM gone
    $$;

REVOKE ALL ON FUNCTION kura_device_link(uuid, uuid[]), kura_tenant_devices(), kura_tenant_has_member(text), kura_tenant_device_revoke(uuid), kura_device_purge() FROM PUBLIC;
GRANT EXECUTE ON FUNCTION kura_device_link(uuid, uuid[]), kura_tenant_devices(), kura_tenant_has_member(text), kura_tenant_device_revoke(uuid), kura_device_purge() TO kura_app;
