-- Phase 4 review L1: a socket re-checks its device on every ping.
-- Is a socket's device still allowed: not revoked, and its session not
-- revoked on another device (D34, D48)? Checked on every ping.
CREATE FUNCTION kura_device_active(p_device uuid) RETURNS boolean
    LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        SELECT d.revoked_at IS NULL AND NOT EXISTS (
            SELECT 1 FROM device r WHERE r.revoked_at IS NOT NULL
              AND (r.session_sid = d.session_sid
                   OR (r.user_sub = d.user_sub AND r.session_auth_time = d.session_auth_time)))
        FROM device d WHERE d.id = p_device
    $$;

REVOKE ALL ON FUNCTION kura_device_active(uuid) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION kura_device_active(uuid) TO kura_app;
