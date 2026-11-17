-- Code review of 2026-11-17 on RFC 0003 phase 1: a signed-out web device is
-- gone. Its open event sockets close (the socket loop asks this function),
-- and it leaves the tenant's device list. RFC 0003's notifications take 0024.
CREATE OR REPLACE FUNCTION kura_device_active(p_device uuid) RETURNS boolean
    LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        SELECT d.revoked_at IS NULL AND d.signed_out_at IS NULL AND NOT EXISTS (
            SELECT 1 FROM device r WHERE r.revoked_at IS NOT NULL
              AND (r.session_sid = d.session_sid
                   OR (r.user_sub = d.user_sub AND r.session_auth_time = d.session_auth_time)))
        FROM device d WHERE d.id = p_device
    $$;

CREATE OR REPLACE FUNCTION kura_tenant_devices()
    RETURNS TABLE (id uuid, user_sub text, name text, profile text, created_at timestamptz, last_seen timestamptz, revoked_at timestamptz)
    LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        SELECT d.id, d.user_sub, d.name, d.profile, d.created_at, d.last_seen, d.revoked_at
        FROM device d JOIN device_tenant l ON l.device_id = d.id
        WHERE l.tenant_id = kura_current_tenant() AND d.signed_out_at IS NULL
        ORDER BY d.created_at, d.id
        LIMIT 1000
    $$;
