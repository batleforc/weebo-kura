-- Plan V4: an open socket re-checks its vault role on every ping, so the
-- ticket carries the claims of the token it was minted with (kept at most
-- 30 seconds, deleted when consumed). The socket closes when that token
-- expires, so the claims never outlive it.
ALTER TABLE ws_ticket ADD COLUMN claims jsonb NOT NULL DEFAULT '{}'::jsonb;

DROP FUNCTION kura_ws_ticket_consume(bytea);
CREATE FUNCTION kura_ws_ticket_consume(p_hash bytea)
    RETURNS TABLE (tenant_id uuid, vault_id uuid, device_id uuid, token_expires_at timestamptz, claims jsonb)
    LANGUAGE sql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        DELETE FROM ws_ticket WHERE ticket_hash = p_hash AND expires_at > now()
        RETURNING tenant_id, vault_id, device_id, token_expires_at, claims
    $$;
REVOKE ALL ON FUNCTION kura_ws_ticket_consume(bytea) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION kura_ws_ticket_consume(bytea) TO kura_app;
