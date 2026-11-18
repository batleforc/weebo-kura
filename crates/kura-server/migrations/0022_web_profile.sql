-- RFC 0003 §6.2: a browser is a device with profile `web`. Signing out marks
-- it signed out, which bans nothing: its id is refused as unknown, and the
-- same Authentik session may register a web device again. Web devices idle
-- 30 days are purged (others: 180). RFC 0005 takes 0021.
ALTER TABLE device
    DROP CONSTRAINT device_profile_check,
    ADD CONSTRAINT device_profile_check CHECK (profile IN ('desktop', 'mobile', 'web')),
    ADD COLUMN signed_out_at timestamptz;

CREATE OR REPLACE FUNCTION kura_device_purge() RETURNS bigint
    LANGUAGE sql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        WITH gone AS (
            DELETE FROM device WHERE revoked_at IS NULL
                AND (last_seen < now() - interval '180 days'
                     OR (profile = 'web' AND last_seen < now() - interval '30 days'))
            RETURNING 1
        )
        SELECT count(*) FROM gone
    $$;
