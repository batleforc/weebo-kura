-- Phase 1 tables (RFC 0001 §6.1): configuration state, tenants, vaults, role
-- bindings, devices, and the persisted JWKS (plan D1). Every table forces
-- row-level security; the gate test `every_table_forces_rls` fails on any
-- future table that forgets it. Runs as the migration owner; kura_app comes
-- from sql/bootstrap-roles.sql.

-- The caller's tenant and subject, set per transaction with
-- set_config(..., true). Unset, or reset to '' when a pooled connection's
-- transaction ends, both read as NULL, which matches no row.
CREATE FUNCTION kura_current_tenant() RETURNS uuid
    LANGUAGE sql STABLE
    AS $$ SELECT NULLIF(current_setting('kura.tenant_id', true), '')::uuid $$;

CREATE FUNCTION kura_current_subject() RETURNS text
    LANGUAGE sql STABLE
    AS $$ SELECT NULLIF(current_setting('kura.subject', true), '') $$;

-- One row per reconciliation; the current one is the highest id. History is
-- kept 30 days (purged by the reconciler's write function, PR 1.5).
CREATE TABLE config_state (
    id             bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    generation     bytea NOT NULL CHECK (octet_length(generation) = 32),
    writer_version text NOT NULL,
    documents      jsonb NOT NULL,
    status         jsonb NOT NULL,
    written_at     timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE tenant (
    id               uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    slug             text NOT NULL UNIQUE,
    claim_mappings   jsonb,
    vault_namespaces text[] NOT NULL DEFAULT '{}',
    retention_policy jsonb NOT NULL DEFAULT '{}',
    quota_bytes      bigint CHECK (quota_bytes >= 0),
    max_file_size    bigint CHECK (max_file_size >= 0),
    transit_key      text,
    search_enabled   boolean NOT NULL DEFAULT false,
    deletion_policy  text NOT NULL DEFAULT 'Orphan' CHECK (deletion_policy IN ('Orphan', 'Delete')),
    disabled_at      timestamptz,
    delete_after     timestamptz,
    created_at       timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE vault (
    id              uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id       uuid NOT NULL REFERENCES tenant (id),
    namespace       text NOT NULL,
    name            text NOT NULL,
    chunking_params jsonb NOT NULL,
    config_sync     jsonb,
    managed_by      text NOT NULL CHECK (managed_by IN ('config', 'api')),
    deletion_policy text NOT NULL DEFAULT 'Orphan' CHECK (deletion_policy IN ('Orphan', 'Delete')),
    disabled_at     timestamptz,
    delete_after    timestamptz,
    created_at      timestamptz NOT NULL DEFAULT now(),
    UNIQUE (namespace, name)
);
CREATE INDEX vault_tenant ON vault (tenant_id);

-- A vault role bound to a claim value of the tenant's mapped claim, or to one
-- OIDC subject.
CREATE TABLE role_binding (
    id           uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id    uuid NOT NULL REFERENCES tenant (id),
    vault_id     uuid NOT NULL REFERENCES vault (id) ON DELETE CASCADE,
    subject_kind text NOT NULL CHECK (subject_kind IN ('claim', 'user')),
    subject      text NOT NULL CHECK (subject <> ''),
    role         text NOT NULL CHECK (role IN ('viewer', 'editor', 'manager')),
    managed_by   text NOT NULL CHECK (managed_by IN ('config', 'api')),
    UNIQUE (vault_id, subject_kind, subject)
);
CREATE INDEX role_binding_tenant ON role_binding (tenant_id);

-- Owned by an OIDC subject that may sit in several tenants: isolated by
-- user_sub, not tenant_id (§6.1). Purged after 180 days idle; revoked ids and
-- their sid are kept (decision 35).
CREATE TABLE device (
    id          uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    user_sub    text NOT NULL,
    session_sid text NOT NULL,
    name        text NOT NULL CHECK (char_length(name) BETWEEN 1 AND 64),
    profile     text NOT NULL CHECK (profile IN ('desktop', 'mobile')),
    created_at  timestamptz NOT NULL DEFAULT now(),
    last_seen   timestamptz NOT NULL DEFAULT now(),
    revoked_at  timestamptz
);
CREATE INDEX device_user ON device (user_sub);
CREATE INDEX device_sid ON device (session_sid) WHERE revoked_at IS NOT NULL;

-- Authentik's signing keys, so a replica started during an Authentik outage
-- can still verify (§6.2). Not tenant data; read and written only through the
-- functions below.
CREATE TABLE jwks_key (
    kid        text PRIMARY KEY,
    jwk        jsonb NOT NULL,
    fetched_at timestamptz NOT NULL DEFAULT now()
);

-- Row-level security, forced, on every table. The owner keeps full access
-- (FORCE applies to it too) so the SECURITY DEFINER functions work; kura_app
-- sees only what its policies allow.
DO $$
DECLARE
    t text;
BEGIN
    FOREACH t IN ARRAY ARRAY['config_state', 'tenant', 'vault', 'role_binding', 'device', 'jwks_key'] LOOP
        EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY', t);
        EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY', t);
        EXECUTE format('CREATE POLICY owner_all ON %I TO %I USING (true) WITH CHECK (true)', t, current_user);
    END LOOP;
END
$$;

CREATE POLICY app_read ON config_state FOR SELECT TO kura_app USING (true);
CREATE POLICY app_tenant ON tenant FOR SELECT TO kura_app USING (id = kura_current_tenant());
CREATE POLICY app_tenant ON vault TO kura_app
    USING (tenant_id = kura_current_tenant()) WITH CHECK (tenant_id = kura_current_tenant());
CREATE POLICY app_tenant ON role_binding TO kura_app
    USING (tenant_id = kura_current_tenant()) WITH CHECK (tenant_id = kura_current_tenant());
CREATE POLICY app_subject ON device TO kura_app
    USING (user_sub = kura_current_subject()) WITH CHECK (user_sub = kura_current_subject());

GRANT SELECT ON config_state, tenant TO kura_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON vault, role_binding, device TO kura_app;
-- The server checks at startup that the schema is not older than itself.
GRANT SELECT ON _sqlx_migrations TO kura_app;

-- Pre-tenant lookups (T4 residual): each one is named, returns the least it
-- can, and is tested in tests/db.rs.

-- The tenant of a vault, so the caller's rights can be resolved before the
-- tenant setting is known.
CREATE FUNCTION kura_vault_tenant(p_vault uuid) RETURNS uuid
    LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$ SELECT tenant_id FROM vault WHERE id = p_vault $$;

CREATE FUNCTION kura_jwks_load() RETURNS TABLE (kid text, jwk jsonb)
    LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$ SELECT kid, jwk FROM jwks_key ORDER BY fetched_at DESC LIMIT 16 $$;

-- Replaces the stored key set with the one Authentik publishes now: a key it
-- stopped publishing is purged (plan D1).
CREATE FUNCTION kura_jwks_replace(p_keys jsonb) RETURNS void
    LANGUAGE sql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        -- NOT EXISTS, not NOT IN: one published key without a kid must not stop the purge.
        DELETE FROM jwks_key j
            WHERE NOT EXISTS (SELECT 1 FROM jsonb_array_elements(p_keys) k WHERE k->>'kid' = j.kid);
        INSERT INTO jwks_key (kid, jwk)
            SELECT k->>'kid', k FROM jsonb_array_elements(p_keys) k WHERE k ? 'kid' LIMIT 16
            ON CONFLICT (kid) DO UPDATE SET jwk = EXCLUDED.jwk, fetched_at = now();
    $$;

REVOKE ALL ON FUNCTION kura_vault_tenant(uuid), kura_jwks_load(), kura_jwks_replace(jsonb) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION kura_vault_tenant(uuid), kura_jwks_load(), kura_jwks_replace(jsonb) TO kura_app;
