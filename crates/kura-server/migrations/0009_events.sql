-- PR 4.2: WebSocket tickets (RFC 0001 §6.3, decision 36). A ticket is 256
-- random bits, stored as its SHA-256, valid 30 seconds, consumed once.
CREATE TABLE ws_ticket (
    ticket_hash      bytea PRIMARY KEY CHECK (octet_length(ticket_hash) = 32),
    tenant_id        uuid NOT NULL,
    vault_id         uuid NOT NULL,
    device_id        uuid NOT NULL,
    token_expires_at timestamptz NOT NULL,
    expires_at       timestamptz NOT NULL,
    FOREIGN KEY (vault_id, tenant_id) REFERENCES vault (id, tenant_id) ON DELETE CASCADE
);
CREATE INDEX ws_ticket_expires ON ws_ticket (expires_at);
CREATE INDEX ws_ticket_tenant ON ws_ticket (tenant_id);
ALTER TABLE ws_ticket ENABLE ROW LEVEL SECURITY;
ALTER TABLE ws_ticket FORCE ROW LEVEL SECURITY;
DO $$
BEGIN
    EXECUTE format('CREATE POLICY owner_all ON ws_ticket TO %I USING (true) WITH CHECK (true)', current_user);
END
$$;
CREATE POLICY app_tenant ON ws_ticket TO kura_app
    USING (tenant_id = kura_current_tenant()) WITH CHECK (tenant_id = kura_current_tenant());
GRANT INSERT ON ws_ticket TO kura_app;

-- A socket presents only its ticket: the tenant is not known yet, so it is
-- consumed through a named function. `DELETE … RETURNING`: a second use finds
-- nothing (T11).
CREATE FUNCTION kura_ws_ticket_consume(p_hash bytea)
    RETURNS TABLE (tenant_id uuid, vault_id uuid, device_id uuid, token_expires_at timestamptz)
    LANGUAGE sql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        DELETE FROM ws_ticket WHERE ticket_hash = p_hash AND expires_at > now()
        RETURNING tenant_id, vault_id, device_id, token_expires_at
    $$;

-- Expired tickets, removed every heartbeat: kept at most 30 seconds (§6.1).
CREATE FUNCTION kura_ws_ticket_purge() RETURNS bigint
    LANGUAGE sql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        WITH gone AS (DELETE FROM ws_ticket WHERE expires_at <= now() RETURNING 1)
        SELECT count(*) FROM gone
    $$;

-- The head a socket pings with, for the vault its ticket named.
CREATE FUNCTION kura_vault_head(p_vault uuid) RETURNS bigint
    LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$ SELECT head_seq FROM vault WHERE id = p_vault $$;

REVOKE ALL ON FUNCTION kura_ws_ticket_consume(bytea), kura_ws_ticket_purge(), kura_vault_head(uuid) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION kura_ws_ticket_consume(bytea), kura_ws_ticket_purge(), kura_vault_head(uuid) TO kura_app;
