-- RFC 0003 §6.4: notification rules, the events they subscribe to, and their
-- deliveries. Every table carries tenant_id and row-level security. Events and
-- deliveries live 7 days. (0023 went to the code review of phase 1.)

-- One row per tenant: its data key for notification secrets (decision 8),
-- wrapped by the tenant's Transit key, and whether its storage is over 90%
-- of the quota (`quota.high` once per crossing).
CREATE TABLE notification_tenant (
    tenant_id   uuid PRIMARY KEY REFERENCES tenant (id) ON DELETE CASCADE,
    wrapped_dek text,
    quota_high  boolean NOT NULL DEFAULT false
);

CREATE TABLE notification_rule (
    id                   uuid PRIMARY KEY,
    tenant_id            uuid NOT NULL REFERENCES tenant (id) ON DELETE CASCADE,
    vault_id             uuid,
    name                 text NOT NULL CHECK (char_length(name) BETWEEN 1 AND 64),
    events               text[] NOT NULL CHECK (cardinality(events) BETWEEN 1 AND 32),
    channel              text NOT NULL DEFAULT 'webhook' CHECK (channel = 'webhook'),
    -- Only the host is ever shown again (§4.5); the rest is sealed (§7.4).
    host                 text NOT NULL,
    sealed_url           bytea NOT NULL,
    sealed_secret        bytea NOT NULL,
    sealed_authorization bytea,
    enabled              boolean NOT NULL DEFAULT true,
    disabled_reason      text,
    consecutive_failures integer NOT NULL DEFAULT 0,
    created_by           text NOT NULL,
    created_at           timestamptz NOT NULL DEFAULT now(),
    updated_at           timestamptz NOT NULL DEFAULT now(),
    FOREIGN KEY (vault_id, tenant_id) REFERENCES vault (id, tenant_id) ON DELETE CASCADE
);
CREATE INDEX notification_rule_tenant ON notification_rule (tenant_id);

CREATE TABLE notification_event (
    id         uuid PRIMARY KEY,
    tenant_id  uuid NOT NULL REFERENCES tenant (id) ON DELETE CASCADE,
    vault_id   uuid,
    type       text NOT NULL,
    payload    jsonb NOT NULL CHECK (octet_length(payload::text) <= 4096),
    -- `vault.changed` once a vault a minute, `token.denied` once a token an
    -- hour: later facts of the window count in the same event.
    dedupe     text,
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, dedupe)
);
CREATE INDEX notification_event_created ON notification_event (created_at);

CREATE TABLE notification_delivery (
    id              uuid PRIMARY KEY,
    tenant_id       uuid NOT NULL REFERENCES tenant (id) ON DELETE CASCADE,
    event_id        uuid NOT NULL REFERENCES notification_event (id) ON DELETE CASCADE,
    rule_id         uuid NOT NULL REFERENCES notification_rule (id) ON DELETE CASCADE,
    attempts        integer NOT NULL DEFAULT 0,
    next_attempt_at timestamptz NOT NULL DEFAULT now(),
    claimed_at      timestamptz,
    status          text NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'delivered', 'failed')),
    -- Coarse only (§4.5): an HTTP status, or timeout, connect, tls,
    -- address_refused, redirect, status_4xx, status_5xx.
    result          text,
    updated_at      timestamptz NOT NULL DEFAULT now(),
    UNIQUE (event_id, rule_id)
);
CREATE INDEX notification_delivery_due ON notification_delivery (next_attempt_at) WHERE status = 'pending';
CREATE INDEX notification_delivery_rule ON notification_delivery (rule_id, updated_at DESC);

DO $$
DECLARE t text;
BEGIN
    FOREACH t IN ARRAY ARRAY['notification_tenant', 'notification_rule', 'notification_event', 'notification_delivery'] LOOP
        EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY', t);
        EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY', t);
        EXECUTE format('CREATE POLICY owner_all ON %I TO %I USING (true) WITH CHECK (true)', t, current_user);
        EXECUTE format('CREATE POLICY app_tenant ON %I TO kura_app USING (tenant_id = kura_current_tenant()) WITH CHECK (tenant_id = kura_current_tenant())', t);
        EXECUTE format('GRANT SELECT, INSERT, UPDATE, DELETE ON %I TO kura_app', t);
    END LOOP;
END
$$;

-- `token.expiring` once per token (§4.2).
ALTER TABLE scoped_token ADD COLUMN expiring_notified boolean NOT NULL DEFAULT false;

-- The worker's claim across tenants, as `kura_index_claim` (§6.4): due and
-- pending, or claimed more than 2 minutes ago by a worker that died.
CREATE FUNCTION kura_notification_claim(p_limit integer)
    RETURNS TABLE (id uuid, tenant_id uuid, slug text)
    LANGUAGE sql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        WITH c AS (
            SELECT d.id FROM notification_delivery d
            WHERE d.status = 'pending' AND d.next_attempt_at <= now()
              AND (d.claimed_at IS NULL OR d.claimed_at < now() - interval '2 minutes')
            ORDER BY d.next_attempt_at
            LIMIT least(p_limit, 16)
            FOR UPDATE SKIP LOCKED
        ), claimed AS (
            UPDATE notification_delivery d SET claimed_at = now() FROM c WHERE d.id = c.id
            RETURNING d.id, d.tenant_id
        )
        SELECT c.id, c.tenant_id, t.slug FROM claimed c JOIN tenant t ON t.id = c.tenant_id
    $$;

-- The (tenant, type) pairs with an enabled rule: each replica's subscription
-- cache (§6.4), so a fact nobody listens to records nothing.
CREATE FUNCTION kura_notification_subscriptions() RETURNS TABLE (tenant_id uuid, type text)
    LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        SELECT DISTINCT r.tenant_id, e FROM notification_rule r, unnest(r.events) e WHERE r.enabled
    $$;

-- The gauges of §10.1, across tenants, with no tenant in them.
CREATE FUNCTION kura_notification_stats() RETURNS TABLE (pending bigint, oldest_seconds double precision, disabled bigint)
    LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        SELECT (SELECT count(*) FROM notification_delivery WHERE status = 'pending'),
               coalesce((SELECT extract(epoch FROM now() - min(next_attempt_at)) FROM notification_delivery
                         WHERE status = 'pending' AND next_attempt_at <= now()), 0)::double precision,
               (SELECT count(*) FROM notification_rule WHERE NOT enabled AND disabled_reason IS NOT NULL)
    $$;

-- Events (and their deliveries) after 7 days, daily.
CREATE FUNCTION kura_notification_purge() RETURNS bigint
    LANGUAGE sql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        WITH gone AS (DELETE FROM notification_event WHERE created_at < now() - interval '7 days' RETURNING 1)
        SELECT count(*) FROM gone
    $$;

-- Tokens expiring within 7 days, each returned once and marked (§4.2).
CREATE FUNCTION kura_tokens_expiring()
    RETURNS TABLE (tenant_id uuid, vault_id uuid, id uuid, name text, expires_at timestamptz)
    LANGUAGE sql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        UPDATE scoped_token SET expiring_notified = true
        WHERE NOT expiring_notified AND revoked_at IS NULL
          AND expires_at > now() AND expires_at <= now() + interval '7 days'
        RETURNING tenant_id, vault_id, id, name, expires_at
    $$;

REVOKE ALL ON FUNCTION kura_notification_claim(integer), kura_notification_subscriptions(), kura_notification_stats(),
    kura_notification_purge(), kura_tokens_expiring() FROM PUBLIC;
GRANT EXECUTE ON FUNCTION kura_notification_claim(integer), kura_notification_subscriptions(), kura_notification_stats(),
    kura_notification_purge(), kura_tokens_expiring() TO kura_app;
