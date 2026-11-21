-- Tokens entering their 7-day window, read without marking them (RFC 0003 §4.2).
-- The caller marks each one in the transaction that records its event, so a
-- failure on one token no longer loses the others' warnings. Additive:
-- kura_tokens_expiring() stays for binaries at 0001.
CREATE FUNCTION kura_tokens_expiring_due()
    RETURNS TABLE (tenant_id uuid, vault_id uuid, id uuid, name text, expires_at timestamptz)
    LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        SELECT tenant_id, vault_id, id, name, expires_at FROM scoped_token
        WHERE NOT expiring_notified AND revoked_at IS NULL
          AND expires_at > now() AND expires_at <= now() + interval '7 days'
    $$;

REVOKE ALL ON FUNCTION kura_tokens_expiring_due() FROM PUBLIC;
GRANT EXECUTE ON FUNCTION kura_tokens_expiring_due() TO kura_app;
