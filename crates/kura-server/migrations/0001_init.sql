-- kura schema (RFC 0001 §6.1 and the RFCs that extended it), described as it
-- stands. Every table forces row-level security; the gate test
-- `every_table_forces_rls` fails on any table that forgets it. Runs as the
-- migration owner; kura_app comes from sql/bootstrap-roles.sql.
--
-- Conventions:
--   * owner_all: the owner keeps full access (FORCE applies to it too), so the
--     SECURITY DEFINER functions work; kura_app sees only what its policies allow.
--   * Pre-tenant and cross-tenant work goes through named SECURITY DEFINER
--     functions (T4 residual): each returns the least it can and is tested.
--
-- Sections: helpers, identity (tenants, vaults, bindings, JWKS), reconciler
-- and config, devices, chunks and blobs, sync (entries, revisions, commits),
-- events, scoped tokens, history and retention, search, notifications, device
-- sync, schema floor, maintenance.

-- ============================================================================
-- Helpers
-- ============================================================================

-- The caller's tenant and subject, set per transaction with
-- set_config(..., true). Unset, or reset to '' when a pooled connection's
-- transaction ends, both read as NULL, which matches no row.
CREATE FUNCTION kura_current_tenant() RETURNS uuid
    LANGUAGE sql STABLE
    AS $$ SELECT NULLIF(current_setting('kura.tenant_id', true), '')::uuid $$;

CREATE FUNCTION kura_current_subject() RETURNS text
    LANGUAGE sql STABLE
    AS $$ SELECT NULLIF(current_setting('kura.subject', true), '') $$;

-- "1.10.2" -> {1,10,2}: versions compare numerically, never as text.
CREATE FUNCTION kura_version(p text) RETURNS int[]
    LANGUAGE sql IMMUTABLE
    AS $$ SELECT string_to_array(split_part(split_part(p, '-', 1), '+', 1), '.')::int[] $$;

-- ============================================================================
-- Identity: configuration state, tenants, vaults, role bindings, JWKS
-- ============================================================================

-- One row per reconciliation; the current one is the highest id. History is
-- kept 30 days (purged by kura_config_state_insert).
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
    created_at       timestamptz NOT NULL DEFAULT now(),
    -- The removal policy the object was last declared with: the annotation
    -- and the policy leave with the document, so the row remembers them.
    force_orphan     boolean NOT NULL DEFAULT false,
    -- Set once a Delete purge emptied the tenant; the row stays until an
    -- operator destroys its Transit key (decision 33).
    purged_at        timestamptz,
    -- Whether the tenant's search index matches its setting: backfilled once
    -- enabled, dropped once disabled.
    search_indexed   boolean NOT NULL DEFAULT false
);

CREATE TABLE vault (
    id               uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id        uuid NOT NULL REFERENCES tenant (id),
    namespace        text NOT NULL,
    name             text NOT NULL,
    chunking_params  jsonb NOT NULL,
    config_sync      jsonb,
    managed_by       text NOT NULL CHECK (managed_by IN ('config', 'api')),
    deletion_policy  text NOT NULL DEFAULT 'Orphan' CHECK (deletion_policy IN ('Orphan', 'Delete')),
    disabled_at      timestamptz,
    delete_after     timestamptz,
    created_at       timestamptz NOT NULL DEFAULT now(),
    force_orphan     boolean NOT NULL DEFAULT false,
    -- One data key per vault, wrapped by the tenant's Transit key and written
    -- once (§6.6, decision 29).
    wrapped_dek      text,
    -- AES-GCM with random nonces: about 2^32 encryptions per key; warn at 2^30.
    encryptions      bigint NOT NULL DEFAULT 0,
    -- The vault row is the commit lock; head_seq is the last assigned seq.
    -- epoch changes when the server loses history (decision 32); horizon_seq
    -- is where purged deletions end.
    epoch            uuid   NOT NULL DEFAULT gen_random_uuid(),
    head_seq         bigint NOT NULL DEFAULT 0 CHECK (head_seq >= 0),
    horizon_seq      bigint NOT NULL DEFAULT 0 CHECK (horizon_seq >= 0),
    -- Set by the first batch of a Delete purge; every batch gives the vault a
    -- new epoch: if its document comes back mid-purge, every device gets
    -- `history_rewound` and pushes its copies back instead of trashing them
    -- as deleted elsewhere.
    purge_started_at timestamptz,
    UNIQUE (namespace, name),
    -- A row's tenant must be its vault's tenant, enforced by the database,
    -- not only by the handlers (phase 2 review P6).
    CONSTRAINT vault_id_tenant UNIQUE (id, tenant_id)
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
    -- RFC 0005: a `viewer` binding may list 1 to 16 folders; NULL is the whole
    -- vault. The CHECK makes "only a viewer is restricted" hold in the
    -- database: a role above viewer always reads the whole vault (§5.1).
    paths        text[],
    UNIQUE (vault_id, subject_kind, subject),
    CONSTRAINT role_binding_paths_check
        CHECK (paths IS NULL OR (role = 'viewer' AND cardinality(paths) BETWEEN 1 AND 16))
);
CREATE INDEX role_binding_tenant ON role_binding (tenant_id);

-- Authentik's signing keys, so a replica started during an Authentik outage
-- can still verify (§6.2). Not tenant data; read and written only through the
-- functions below.
CREATE TABLE jwks_key (
    kid        text PRIMARY KEY,
    jwk        jsonb NOT NULL,
    fetched_at timestamptz NOT NULL DEFAULT now()
);

DO $$
DECLARE
    t text;
BEGIN
    FOREACH t IN ARRAY ARRAY['config_state', 'tenant', 'vault', 'role_binding', 'jwks_key'] LOOP
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

GRANT SELECT ON config_state, tenant TO kura_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON vault, role_binding TO kura_app;
-- The server checks at startup that the schema is not older than itself.
GRANT SELECT ON _sqlx_migrations TO kura_app;

-- Pre-tenant lookups: each one is named, returns the least it can, and is
-- tested in tests/db.rs.

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

-- ============================================================================
-- Reconciler and config (RFC 0001 §6.10)
-- ============================================================================
-- The server reconciles every tenant at once, so each write is a named
-- SECURITY DEFINER function (T4 residual), tested in tests/reconcile.rs.

-- Takes the transaction-scoped lock and says whether this replica may write
-- (§6.10): never when the stored writer is newer or the generation is the
-- same; always when the stored writer is older or nothing is stored; for an
-- equal version, only when this replica's own files changed (p_local_change),
-- so a replica whose mount lags behind never reverts a fresher one.
CREATE FUNCTION kura_config_begin(p_generation bytea, p_version text, p_local_change boolean) RETURNS boolean
    LANGUAGE plpgsql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
DECLARE
    stored record;
BEGIN
    PERFORM pg_advisory_xact_lock(hashtext('kura.config_state'));
    SELECT generation, writer_version INTO stored FROM config_state ORDER BY id DESC LIMIT 1;
    IF NOT FOUND THEN
        RETURN true;
    END IF;
    IF stored.generation = p_generation OR kura_version(stored.writer_version) > kura_version(p_version) THEN
        RETURN false;
    END IF;
    IF kura_version(stored.writer_version) < kura_version(p_version) THEN
        RETURN true;
    END IF;
    RETURN p_local_change;
END
$$;

-- What validation needs from the stored state (kura_api::PreviousState).
CREATE FUNCTION kura_config_previous()
    RETURNS TABLE (kind text, namespace text, name text, chunking jsonb, managed_by text, disabled boolean)
    LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        SELECT 'tenant', NULL, slug, NULL, 'config', disabled_at IS NOT NULL FROM tenant
        UNION ALL
        SELECT 'vault', namespace, name, chunking_params, managed_by, disabled_at IS NOT NULL FROM vault
    $$;

-- Enabled tenants, so the authorization cache can map a slug to its id.
CREATE FUNCTION kura_tenant_ids() RETURNS TABLE (slug text, id uuid)
    LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$ SELECT slug, id FROM tenant WHERE disabled_at IS NULL $$;

-- Declares a tenant. A returning document cancels a scheduled deletion.
CREATE FUNCTION kura_tenant_upsert(
    p_slug text, p_claim_mappings jsonb, p_vault_namespaces text[], p_retention jsonb,
    p_quota_bytes bigint, p_max_file_size bigint, p_search boolean, p_deletion_policy text, p_force_orphan boolean
) RETURNS uuid
    LANGUAGE sql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        INSERT INTO tenant (slug, claim_mappings, vault_namespaces, retention_policy, quota_bytes, max_file_size,
                            search_enabled, deletion_policy, force_orphan)
        VALUES (p_slug, p_claim_mappings, p_vault_namespaces, p_retention, p_quota_bytes, p_max_file_size,
                p_search, p_deletion_policy, p_force_orphan)
        ON CONFLICT (slug) DO UPDATE SET
            claim_mappings = EXCLUDED.claim_mappings, vault_namespaces = EXCLUDED.vault_namespaces,
            retention_policy = EXCLUDED.retention_policy, quota_bytes = EXCLUDED.quota_bytes,
            max_file_size = EXCLUDED.max_file_size, search_enabled = EXCLUDED.search_enabled,
            deletion_policy = EXCLUDED.deletion_policy, force_orphan = EXCLUDED.force_orphan,
            disabled_at = NULL, delete_after = NULL
        RETURNING id
    $$;

-- Declares a file-managed vault. The chunking is written at creation only
-- (validation refuses a change); an API-managed vault is never taken over.
CREATE FUNCTION kura_vault_upsert(
    p_tenant text, p_namespace text, p_name text, p_chunking jsonb, p_config_sync jsonb,
    p_deletion_policy text, p_force_orphan boolean
) RETURNS uuid
    LANGUAGE sql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        INSERT INTO vault (tenant_id, namespace, name, chunking_params, config_sync, managed_by, deletion_policy, force_orphan)
        SELECT id, p_namespace, p_name, p_chunking, p_config_sync, 'config', p_deletion_policy, p_force_orphan
        FROM tenant WHERE slug = p_tenant
        ON CONFLICT (namespace, name) DO UPDATE SET
            config_sync = EXCLUDED.config_sync, deletion_policy = EXCLUDED.deletion_policy,
            force_orphan = EXCLUDED.force_orphan, disabled_at = NULL, delete_after = NULL
            WHERE vault.managed_by = 'config' AND vault.tenant_id = EXCLUDED.tenant_id
        RETURNING id
    $$;

-- Replaces the file-managed bindings of a vault; API-managed ones stay.
-- p_bindings: [{"kind": "claim"|"user", "subject": "...", "role": "...", "paths": [...]}].
-- A binding still declared keeps its row and id (a scoped token hangs on it,
-- `granted_by_binding`): only those gone are deleted, a changed role or
-- `paths` is updated in place.
CREATE FUNCTION kura_bindings_replace(p_vault uuid, p_bindings jsonb) RETURNS void
    LANGUAGE sql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        DELETE FROM role_binding rb
        WHERE rb.vault_id = p_vault AND rb.managed_by = 'config'
          AND NOT EXISTS (
              SELECT 1 FROM jsonb_array_elements(p_bindings) b
              WHERE b->>'kind' = rb.subject_kind AND b->>'subject' = rb.subject
          );
        INSERT INTO role_binding (tenant_id, vault_id, subject_kind, subject, role, managed_by, paths)
            SELECT DISTINCT ON (b->>'kind', b->>'subject')
                   v.tenant_id, v.id, b->>'kind', b->>'subject', b->>'role', 'config',
                   CASE WHEN jsonb_typeof(b->'paths') = 'array'
                        THEN ARRAY(SELECT jsonb_array_elements_text(b->'paths')) END
            FROM vault v, jsonb_array_elements(p_bindings) WITH ORDINALITY AS x(b, n)
            WHERE v.id = p_vault
            ORDER BY b->>'kind', b->>'subject', n
            ON CONFLICT (vault_id, subject_kind, subject) DO UPDATE
                SET role = EXCLUDED.role, paths = EXCLUDED.paths
                WHERE role_binding.managed_by = 'config';
    $$;

-- Disables the file-managed objects no longer declared. `Delete` schedules
-- the purge 30 days out; `Orphan`, or the force-orphan annotation, keeps the
-- data with no deletion date. Returns what changed.
CREATE FUNCTION kura_config_disable_missing(p_tenants text[], p_vaults jsonb)
    RETURNS TABLE (kind text, namespace text, name text, delete_after timestamptz)
    LANGUAGE sql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        WITH t AS (
            UPDATE tenant SET disabled_at = now(),
                delete_after = CASE WHEN deletion_policy = 'Delete' AND NOT force_orphan THEN now() + interval '30 days' END
            WHERE disabled_at IS NULL AND slug <> ALL (p_tenants)
            RETURNING 'tenant'::text, NULL::text, slug, delete_after
        ), v AS (
            UPDATE vault SET disabled_at = now(),
                delete_after = CASE WHEN deletion_policy = 'Delete' AND NOT force_orphan THEN now() + interval '30 days' END
            WHERE managed_by = 'config' AND disabled_at IS NULL
              AND NOT EXISTS (SELECT 1 FROM jsonb_array_elements(p_vaults) k
                              WHERE k->>'namespace' = vault.namespace AND k->>'name' = vault.name)
            RETURNING 'vault'::text, namespace, name, delete_after
        )
        SELECT * FROM t UNION ALL SELECT * FROM v
    $$;

-- Records the reconciled state, last of the transaction, and drops history
-- older than 30 days (§6.1), always keeping the current row.
CREATE FUNCTION kura_config_state_insert(p_generation bytea, p_version text, p_documents jsonb, p_status jsonb)
    RETURNS bigint
    LANGUAGE sql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        DELETE FROM config_state WHERE written_at < now() - interval '30 days'
            AND id <> (SELECT max(id) FROM config_state);
        INSERT INTO config_state (generation, writer_version, documents, status)
            VALUES (p_generation, p_version, p_documents, p_status)
            RETURNING id;
    $$;

DO $$
DECLARE
    f text;
BEGIN
    FOREACH f IN ARRAY ARRAY[
        'kura_config_begin(bytea, text, boolean)', 'kura_config_previous()', 'kura_tenant_ids()',
        'kura_tenant_upsert(text, jsonb, text[], jsonb, bigint, bigint, boolean, text, boolean)',
        'kura_vault_upsert(text, text, text, jsonb, jsonb, text, boolean)',
        'kura_bindings_replace(uuid, jsonb)', 'kura_config_disable_missing(text[], jsonb)',
        'kura_config_state_insert(bytea, text, jsonb, jsonb)'
    ] LOOP
        EXECUTE format('REVOKE ALL ON FUNCTION %s FROM PUBLIC', f);
        EXECUTE format('GRANT EXECUTE ON FUNCTION %s TO kura_app', f);
    END LOOP;
END
$$;

-- ============================================================================
-- Devices (RFC 0001 §4.4, §6.1; RFC 0003 web profile)
-- ============================================================================

-- Owned by an OIDC subject that may sit in several tenants: isolated by
-- user_sub, not tenant_id (§6.1). Purged after 180 days idle (web: 30); revoked
-- ids and their sid are kept (decision 35).
CREATE TABLE device (
    id                 uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    user_sub           text NOT NULL,
    -- Authentik drops `sid` from tokens issued on a refresh but keeps
    -- `auth_time`, the time the session signed in. A device records both; a
    -- revoked session is matched by either (sid, or the owner's auth_time).
    session_sid        text,
    name               text NOT NULL CHECK (char_length(name) BETWEEN 1 AND 64),
    profile            text NOT NULL CHECK (profile IN ('desktop', 'mobile', 'web')),
    created_at         timestamptz NOT NULL DEFAULT now(),
    last_seen          timestamptz NOT NULL DEFAULT now(),
    revoked_at         timestamptz,
    session_auth_time  bigint,
    -- A browser signing out marks its device signed out, which bans nothing:
    -- its id is refused as unknown, and the same Authentik session may register
    -- a web device again.
    signed_out_at      timestamptz,
    -- The owner's own `name` (else `preferred_username`) claim, written at
    -- registration and refreshed with `last_seen` (at most once a minute).
    -- Shown only where the owner already is.
    owner_name         text CHECK (char_length(owner_name) BETWEEN 1 AND 128),
    CONSTRAINT device_session CHECK (session_sid IS NOT NULL OR session_auth_time IS NOT NULL)
);
CREATE INDEX device_user ON device (user_sub);
CREATE INDEX device_sid ON device (session_sid) WHERE revoked_at IS NOT NULL;
CREATE INDEX device_auth_time ON device (user_sub, session_auth_time) WHERE revoked_at IS NOT NULL;

-- A tenant admin lists and revokes the tenant's devices (§4.4). A device
-- belongs to a subject, not a tenant (§6.1), so registration links it to the
-- tenants its owner belongs to.
CREATE TABLE device_tenant (
    tenant_id uuid NOT NULL REFERENCES tenant (id),
    device_id uuid NOT NULL REFERENCES device (id) ON DELETE CASCADE,
    PRIMARY KEY (tenant_id, device_id)
);
CREATE INDEX device_tenant_device ON device_tenant (device_id);

ALTER TABLE device ENABLE ROW LEVEL SECURITY;
ALTER TABLE device FORCE ROW LEVEL SECURITY;
ALTER TABLE device_tenant ENABLE ROW LEVEL SECURITY;
ALTER TABLE device_tenant FORCE ROW LEVEL SECURITY;
DO $$
DECLARE
    t text;
BEGIN
    FOREACH t IN ARRAY ARRAY['device', 'device_tenant'] LOOP
        EXECUTE format('CREATE POLICY owner_all ON %I TO %I USING (true) WITH CHECK (true)', t, current_user);
    END LOOP;
END
$$;
CREATE POLICY app_subject ON device TO kura_app
    USING (user_sub = kura_current_subject()) WITH CHECK (user_sub = kura_current_subject());
CREATE POLICY app_tenant ON device_tenant FOR SELECT TO kura_app USING (tenant_id = kura_current_tenant());
GRANT SELECT, INSERT, UPDATE, DELETE ON device TO kura_app;
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

-- The devices linked to the current tenant (a tenant admin's view), without
-- signed-out web devices. Links are refreshed at most once a minute while a
-- device is used (D27).
CREATE FUNCTION kura_tenant_devices()
    RETURNS TABLE (id uuid, user_sub text, owner_name text, name text, profile text, created_at timestamptz, last_seen timestamptz, revoked_at timestamptz)
    LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        SELECT d.id, d.user_sub, d.owner_name, d.name, d.profile, d.created_at, d.last_seen, d.revoked_at
        FROM device d JOIN device_tenant l ON l.device_id = d.id
        WHERE l.tenant_id = kura_current_tenant() AND d.signed_out_at IS NULL
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

-- Daily purge (§6.1): devices idle 180 days go (web devices: 30); revoked
-- ones and their sid stay, so a stolen refresh token still cannot register
-- (decision 35).
CREATE FUNCTION kura_device_purge() RETURNS bigint
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

-- Is a socket's device still allowed: not revoked, not signed out, and its
-- session not revoked on another device (D34, D48)? Checked on every ping.
CREATE FUNCTION kura_device_active(p_device uuid) RETURNS boolean
    LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        SELECT d.revoked_at IS NULL AND d.signed_out_at IS NULL AND NOT EXISTS (
            SELECT 1 FROM device r WHERE r.revoked_at IS NOT NULL
              AND (r.session_sid = d.session_sid
                   OR (r.user_sub = d.user_sub AND r.session_auth_time = d.session_auth_time)))
        FROM device d WHERE d.id = p_device
    $$;

REVOKE ALL ON FUNCTION kura_device_link(uuid, uuid[]), kura_tenant_devices(), kura_tenant_has_member(text),
    kura_tenant_device_revoke(uuid), kura_device_purge(), kura_device_active(uuid) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION kura_device_link(uuid, uuid[]), kura_tenant_devices(), kura_tenant_has_member(text),
    kura_tenant_device_revoke(uuid), kura_device_purge(), kura_device_active(uuid) TO kura_app;

-- ============================================================================
-- Chunks and blobs (RFC 0001 §6.1, §6.4)
-- ============================================================================
-- Every row carries its tenant and is isolated by it, like the identity tables.

-- One stored chunk of a vault, addressed by the BLAKE3 hash of its plaintext.
CREATE TABLE chunk (
    vault_id     uuid NOT NULL,
    tenant_id    uuid NOT NULL,
    hash         bytea NOT NULL CHECK (octet_length(hash) = 32),
    size         integer NOT NULL CHECK (size BETWEEN 0 AND 4194304),
    object_key   text NOT NULL UNIQUE,
    created_at   timestamptz NOT NULL DEFAULT now(),
    last_touched timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (vault_id, hash),
    FOREIGN KEY (vault_id, tenant_id) REFERENCES vault (id, tenant_id)
);
-- Garbage collection sweep.
CREATE INDEX chunk_last_touched ON chunk (last_touched);
CREATE INDEX chunk_tenant ON chunk (tenant_id);

-- The manifest of one file version: the ordered list of its chunks. A blob
-- PUT refreshes last_touched, as `chunks/missing` does a chunk's, so a
-- manifest about to be committed is never collected (§6.9).
CREATE TABLE blob (
    vault_id     uuid NOT NULL,
    tenant_id    uuid NOT NULL,
    hash         bytea NOT NULL CHECK (octet_length(hash) = 32),
    size         bigint NOT NULL CHECK (size >= 0),
    created_at   timestamptz NOT NULL DEFAULT now(),
    last_touched timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (vault_id, hash),
    FOREIGN KEY (vault_id, tenant_id) REFERENCES vault (id, tenant_id)
);
CREATE INDEX blob_last_touched ON blob (vault_id, last_touched);

CREATE TABLE blob_chunk (
    vault_id   uuid NOT NULL,
    tenant_id  uuid NOT NULL,
    blob_hash  bytea NOT NULL,
    position   integer NOT NULL CHECK (position >= 0),
    chunk_hash bytea NOT NULL,
    PRIMARY KEY (vault_id, blob_hash, position),
    FOREIGN KEY (vault_id, blob_hash) REFERENCES blob (vault_id, hash) ON DELETE CASCADE,
    FOREIGN KEY (vault_id, chunk_hash) REFERENCES chunk (vault_id, hash),
    FOREIGN KEY (vault_id, tenant_id) REFERENCES vault (id, tenant_id)
);
-- "Is this chunk still referenced?" (§6.1).
CREATE INDEX blob_chunk_chunk ON blob_chunk (vault_id, chunk_hash);

DO $$
DECLARE
    t text;
BEGIN
    FOREACH t IN ARRAY ARRAY['chunk', 'blob', 'blob_chunk'] LOOP
        EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY', t);
        EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY', t);
        EXECUTE format('CREATE POLICY owner_all ON %I TO %I USING (true) WITH CHECK (true)', t, current_user);
        EXECUTE format('CREATE POLICY app_tenant ON %I TO kura_app USING (tenant_id = kura_current_tenant()) WITH CHECK (tenant_id = kura_current_tenant())', t);
        EXECUTE format('GRANT SELECT, INSERT, UPDATE ON %I TO kura_app', t);
    END LOOP;
END
$$;

-- Stored bytes per tenant, for the QuotaBelowUsage condition (§4.3). A
-- cross-tenant read for platform admins only, through a named function.
CREATE FUNCTION kura_tenant_usage() RETURNS TABLE (slug text, used bigint)
    LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        SELECT t.slug, coalesce(sum(c.size), 0)::bigint
        FROM tenant t LEFT JOIN chunk c ON c.tenant_id = t.id
        WHERE t.disabled_at IS NULL
        GROUP BY t.slug
    $$;

-- Which of these object keys a chunk row still names: the others, old
-- enough, are left by a crash between an object PUT and its row (§6.9).
CREATE FUNCTION kura_known_objects(p_vault uuid, p_keys text[]) RETURNS SETOF text
    LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        SELECT object_key FROM chunk WHERE vault_id = p_vault AND object_key = ANY (p_keys)
    $$;

REVOKE ALL ON FUNCTION kura_tenant_usage(), kura_known_objects(uuid, text[]) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION kura_tenant_usage(), kura_known_objects(uuid, text[]) TO kura_app;

-- ============================================================================
-- Sync: entries, revisions, commits (RFC 0001 §6.1, §6.3; RFC 0007 folders)
-- ============================================================================
-- Every row carries its tenant and is isolated by it.

-- One file of a vault, under a stable id chosen by the client (UUIDv7), so a
-- rename keeps its history. path_key is computed by the server only (§6.3).
-- An entry lives in the note namespace, or in a configuration namespace owned
-- by one user (Obsidian configuration per device profile, §6.8, decision 34,
-- T14): `config:desktop` or `config:mobile`.
CREATE TABLE entry (
    vault_id      uuid    NOT NULL,
    tenant_id     uuid    NOT NULL,
    id            uuid    NOT NULL,
    namespace     text    NOT NULL DEFAULT 'notes',
    path          text    NOT NULL CHECK (octet_length(path) BETWEEN 1 AND 4096),
    path_key      text    NOT NULL,
    head_revision uuid    NOT NULL,
    last_seq      bigint  NOT NULL CHECK (last_seq > 0),
    deleted       boolean NOT NULL DEFAULT false,
    owner_sub     text,
    PRIMARY KEY (vault_id, id),
    FOREIGN KEY (vault_id, tenant_id) REFERENCES vault (id, tenant_id),
    CONSTRAINT entry_namespace_check CHECK (
        namespace IN ('notes', 'config:desktop', 'config:mobile')
        AND (namespace = 'notes') = (owner_sub IS NULL)
    )
);
-- Changes after a cursor; path lookup and uniqueness among live entries
-- (per namespace and owner, §6.3); fuzzy file names (§6.7).
CREATE INDEX entry_changes ON entry (vault_id, last_seq);
CREATE UNIQUE INDEX entry_path ON entry (vault_id, namespace, coalesce(owner_sub, ''), path_key) WHERE NOT deleted;
CREATE INDEX entry_tenant ON entry (tenant_id);

-- Immutable. device_id has no foreign key: purging a device keeps attribution.
-- A live revision without a blob is a folder (size 0); a deleted revision
-- never has a blob.
CREATE TABLE revision (
    id            uuid    PRIMARY KEY,
    vault_id      uuid    NOT NULL,
    tenant_id     uuid    NOT NULL,
    entry_id      uuid    NOT NULL,
    seq           bigint  NOT NULL CHECK (seq > 0),
    base_revision uuid,
    path          text    NOT NULL,
    deleted       boolean NOT NULL,
    blob_hash     bytea,
    size          bigint  NOT NULL CHECK (size >= 0),
    device_id     uuid    NOT NULL,
    user_sub      text    NOT NULL,
    created_at    timestamptz NOT NULL DEFAULT now(),
    UNIQUE (vault_id, seq),
    CONSTRAINT revision_check
        CHECK (NOT (deleted AND blob_hash IS NOT NULL) AND (blob_hash IS NOT NULL OR size = 0)),
    FOREIGN KEY (vault_id, entry_id) REFERENCES entry (vault_id, id),
    FOREIGN KEY (vault_id, blob_hash) REFERENCES blob (vault_id, hash),
    FOREIGN KEY (vault_id, tenant_id) REFERENCES vault (id, tenant_id)
);
-- History of an entry (§6.1); "is this blob still referenced?".
CREATE INDEX revision_history ON revision (vault_id, entry_id, seq);
CREATE INDEX revision_tenant ON revision (tenant_id);
CREATE INDEX revision_blob ON revision (vault_id, blob_hash);

-- A commit's idempotency record, per device, kept 7 days (§6.3). Named
-- commit_record: COMMIT is an SQL keyword.
CREATE TABLE commit_record (
    vault_id        uuid  NOT NULL,
    tenant_id       uuid  NOT NULL,
    device_id       uuid  NOT NULL,
    idempotency_key text  NOT NULL CHECK (length(idempotency_key) BETWEEN 1 AND 128),
    request_hash    bytea NOT NULL CHECK (octet_length(request_hash) = 32),
    result          jsonb NOT NULL,
    created_at      timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (vault_id, device_id, idempotency_key),
    FOREIGN KEY (vault_id, tenant_id) REFERENCES vault (id, tenant_id)
);
CREATE INDEX commit_record_created ON commit_record (created_at);
CREATE INDEX commit_record_tenant ON commit_record (tenant_id);

DO $$
DECLARE
    t text;
BEGIN
    FOREACH t IN ARRAY ARRAY['entry', 'revision', 'commit_record'] LOOP
        EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY', t);
        EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY', t);
        EXECUTE format('CREATE POLICY owner_all ON %I TO %I USING (true) WITH CHECK (true)', t, current_user);
    END LOOP;
END
$$;
-- A configuration entry is visible to its owner only: the transaction's
-- subject (`kura.subject`), never set for a scoped token or a job. Row-level
-- security hides another user's configuration from every request, whatever
-- query it runs (§13: this only narrows reads and writes to the caller's own
-- rows, plan D69).
CREATE POLICY app_tenant ON entry TO kura_app
    USING (tenant_id = kura_current_tenant() AND (owner_sub IS NULL OR owner_sub = kura_current_subject()))
    WITH CHECK (tenant_id = kura_current_tenant() AND (owner_sub IS NULL OR owner_sub = kura_current_subject()));
CREATE POLICY app_tenant ON revision TO kura_app
    USING (tenant_id = kura_current_tenant()) WITH CHECK (tenant_id = kura_current_tenant());
CREATE POLICY app_tenant ON commit_record TO kura_app
    USING (tenant_id = kura_current_tenant()) WITH CHECK (tenant_id = kura_current_tenant());
GRANT SELECT, INSERT, UPDATE ON entry TO kura_app;
GRANT SELECT, INSERT ON revision, commit_record TO kura_app;

-- Daily purge of idempotency records older than 7 days, across tenants.
CREATE FUNCTION kura_commit_purge() RETURNS bigint
    LANGUAGE sql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        WITH gone AS (
            DELETE FROM commit_record WHERE created_at < now() - interval '7 days' RETURNING 1
        )
        SELECT count(*) FROM gone
    $$;

-- Where entry ids live, whoever owns them: a commit naming an id of another
-- namespace or owner is refused rather than written over (review 8).
CREATE FUNCTION kura_entry_home(p_vault uuid, p_ids uuid[])
    RETURNS TABLE (id uuid, namespace text, owner_sub text)
    LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$ SELECT e.id, e.namespace, e.owner_sub FROM entry e WHERE e.vault_id = p_vault AND e.id = ANY (p_ids) $$;

REVOKE ALL ON FUNCTION kura_commit_purge(), kura_entry_home(uuid, uuid[]) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION kura_commit_purge(), kura_entry_home(uuid, uuid[]) TO kura_app;

-- ============================================================================
-- Events: WebSocket tickets (RFC 0001 §6.3, decision 36)
-- ============================================================================

-- A ticket is 256 random bits, stored as its SHA-256, valid 30 seconds,
-- consumed once. An open socket re-checks its vault role on every ping (plan
-- V4), so the ticket carries the claims of the token it was minted with; the
-- socket closes when that token expires, so the claims never outlive it.
CREATE TABLE ws_ticket (
    ticket_hash      bytea PRIMARY KEY CHECK (octet_length(ticket_hash) = 32),
    tenant_id        uuid NOT NULL,
    vault_id         uuid NOT NULL,
    device_id        uuid NOT NULL,
    token_expires_at timestamptz NOT NULL,
    expires_at       timestamptz NOT NULL,
    claims           jsonb NOT NULL DEFAULT '{}'::jsonb,
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
    RETURNS TABLE (tenant_id uuid, vault_id uuid, device_id uuid, token_expires_at timestamptz, claims jsonb)
    LANGUAGE sql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        DELETE FROM ws_ticket WHERE ticket_hash = p_hash AND expires_at > now()
        RETURNING tenant_id, vault_id, device_id, token_expires_at, claims
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

-- ============================================================================
-- Scoped tokens (RFC 0001 §6.2, T5; RFC 0004)
-- ============================================================================

-- For non-interactive readers and writers. Only the secret's SHA-256 is
-- stored. A token is revoked with the grant that let its creator issue it: a
-- binding (the foreign key empties when the binding goes or leaves the
-- manager role), or a tenant role through a claim value (checked against the
-- reconciled configuration at each use).
-- RFC 0004: it carries 1 to 16 folder rules, each `read` or `write`, and a
-- name. `path_prefix` stays for a rollback: a token with one read rule keeps
-- its folder there; any other stores '/', which no vault path starts with, so
-- a binary from before the rules sees nothing through it (T8).
CREATE TABLE scoped_token (
    id                 uuid PRIMARY KEY,
    tenant_id          uuid NOT NULL,
    vault_id           uuid NOT NULL,
    path_prefix        text NOT NULL DEFAULT '' CHECK (octet_length(path_prefix) <= 4096),
    mode               text NOT NULL DEFAULT 'read' CHECK (mode IN ('read', 'write')),
    allowed_cidrs      cidr[] NOT NULL CHECK (cardinality(allowed_cidrs) BETWEEN 1 AND 32),
    expires_at         timestamptz NOT NULL,
    token_hash         bytea NOT NULL CHECK (octet_length(token_hash) = 32),
    created_by         text NOT NULL,
    granted_by_binding uuid REFERENCES role_binding (id) ON DELETE SET NULL,
    granted_by_claim   text,
    created_at         timestamptz NOT NULL DEFAULT now(),
    last_used_at       timestamptz,
    revoked_at         timestamptz,
    name               text NOT NULL CHECK (octet_length(name) BETWEEN 1 AND 64),
    scopes             jsonb NOT NULL,
    -- `token.expiring` once per token (RFC 0003 §4.2).
    expiring_notified  boolean NOT NULL DEFAULT false,
    CONSTRAINT scoped_token_scopes_check
        CHECK (jsonb_typeof(scopes) = 'array' AND jsonb_array_length(scopes) BETWEEN 1 AND 16),
    CHECK (expires_at <= created_at + interval '90 days'),
    FOREIGN KEY (vault_id, tenant_id) REFERENCES vault (id, tenant_id) ON DELETE CASCADE
);
CREATE INDEX scoped_token_vault ON scoped_token (vault_id);
CREATE INDEX scoped_token_tenant ON scoped_token (tenant_id);
CREATE INDEX scoped_token_expires ON scoped_token (expires_at);

-- A binary from before the rules inserts neither `name` nor `scopes` (§13):
-- its token gets the one read rule its prefix means.
CREATE FUNCTION kura_scoped_token_defaults() RETURNS trigger
    LANGUAGE plpgsql SET search_path = pg_catalog, public
    AS $$
    BEGIN
        IF NEW.scopes IS NULL THEN
            NEW.scopes := jsonb_build_array(jsonb_build_object('path', NEW.path_prefix, 'mode', 'read'));
        END IF;
        IF NEW.name IS NULL THEN
            NEW.name := 'key-' || left(NEW.id::text, 8);
        END IF;
        RETURN NEW;
    END
    $$;
CREATE TRIGGER scoped_token_defaults BEFORE INSERT ON scoped_token
    FOR EACH ROW EXECUTE FUNCTION kura_scoped_token_defaults();

-- A scoped token is valid while the binding that granted it exists with the
-- manager role. Leaving the manager role cuts it for good, as deleting the
-- binding does (ON DELETE SET NULL): raising the role again must not revive it.
CREATE FUNCTION kura_binding_downgraded() RETURNS trigger
    LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
    BEGIN
        UPDATE scoped_token SET granted_by_binding = NULL WHERE granted_by_binding = OLD.id;
        RETURN NEW;
    END
    $$;
CREATE TRIGGER binding_downgraded AFTER UPDATE OF role ON role_binding
    FOR EACH ROW WHEN (OLD.role = 'manager' AND NEW.role <> 'manager')
    EXECUTE FUNCTION kura_binding_downgraded();
REVOKE ALL ON FUNCTION kura_binding_downgraded() FROM PUBLIC;

-- What a token proved it holds (RFC 0004 §6.4): content it uploaded itself.
-- With what it can already read, this is all a write token may name in a
-- manifest or a commit, so a hash alone is not a capability (T2, T3).
CREATE TABLE token_upload (
    token_id   uuid NOT NULL REFERENCES scoped_token (id) ON DELETE CASCADE,
    vault_id   uuid NOT NULL,
    tenant_id  uuid NOT NULL,
    kind       text NOT NULL CHECK (kind IN ('chunk', 'blob')),
    hash       bytea NOT NULL CHECK (octet_length(hash) = 32),
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (token_id, kind, hash)
);
CREATE INDEX token_upload_created ON token_upload (created_at);

DO $$
DECLARE
    t text;
BEGIN
    FOREACH t IN ARRAY ARRAY['scoped_token', 'token_upload'] LOOP
        EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY', t);
        EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY', t);
        EXECUTE format('CREATE POLICY owner_all ON %I TO %I USING (true) WITH CHECK (true)', t, current_user);
        EXECUTE format('CREATE POLICY app_tenant ON %I TO kura_app USING (tenant_id = kura_current_tenant()) WITH CHECK (tenant_id = kura_current_tenant())', t);
        EXECUTE format('GRANT SELECT, INSERT, UPDATE ON %I TO kura_app', t);
    END LOOP;
END
$$;

-- The presented token names its id before any tenant is known (T4): one
-- named lookup, which returns the hash to compare and the grant to check.
-- `binding_manager` is whether the granting binding still exists with the
-- manager role.
CREATE FUNCTION kura_scoped_token_lookup(p_id uuid)
    RETURNS TABLE (tenant_id uuid, vault_id uuid, path_prefix text, allowed_cidrs text[],
                   expires_at timestamptz, revoked_at timestamptz, token_hash bytea,
                   granted_by_claim text, binding_manager boolean, scopes jsonb, name text)
    LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        SELECT t.tenant_id, t.vault_id, t.path_prefix, t.allowed_cidrs::text[], t.expires_at, t.revoked_at,
               t.token_hash, t.granted_by_claim,
               EXISTS (SELECT 1 FROM role_binding b WHERE b.id = t.granted_by_binding AND b.role = 'manager'),
               t.scopes, t.name
        FROM scoped_token t WHERE t.id = p_id
    $$;

-- last_used_at, at most once a minute (§6.2).
CREATE FUNCTION kura_scoped_token_touch(p_id uuid) RETURNS void
    LANGUAGE sql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        UPDATE scoped_token SET last_used_at = now()
        WHERE id = p_id AND (last_used_at IS NULL OR last_used_at < now() - interval '1 minute')
    $$;

-- Kept 30 days past expiry, for the audit trail (§9), then purged daily.
CREATE FUNCTION kura_scoped_token_purge() RETURNS bigint
    LANGUAGE sql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        WITH gone AS (DELETE FROM scoped_token WHERE expires_at < now() - interval '30 days' RETURNING 1)
        SELECT count(*) FROM gone
    $$;

-- Kept 24 hours: long enough for an upload to reach its commit (T6).
CREATE FUNCTION kura_token_upload_purge() RETURNS bigint
    LANGUAGE sql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        WITH gone AS (DELETE FROM token_upload WHERE created_at < now() - interval '24 hours' RETURNING 1)
        SELECT count(*) FROM gone
    $$;

-- The tenants holding live write tokens: `GET /v1/admin/status` warns when
-- one of them has `scopedTokens.write` off, where those writes are refused
-- (RFC 0004 §4.3).
CREATE FUNCTION kura_write_token_tenants() RETURNS TABLE (slug text, tokens bigint)
    LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        SELECT t.slug, count(*)
        FROM scoped_token s JOIN tenant t ON t.id = s.tenant_id
        WHERE s.mode = 'write' AND s.revoked_at IS NULL AND s.expires_at > now() AND t.disabled_at IS NULL
        GROUP BY t.slug
    $$;

-- Tokens expiring within 7 days, each returned once and marked (RFC 0003 §4.2).
CREATE FUNCTION kura_tokens_expiring()
    RETURNS TABLE (tenant_id uuid, vault_id uuid, id uuid, name text, expires_at timestamptz)
    LANGUAGE sql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        UPDATE scoped_token SET expiring_notified = true
        WHERE NOT expiring_notified AND revoked_at IS NULL
          AND expires_at > now() AND expires_at <= now() + interval '7 days'
        RETURNING tenant_id, vault_id, id, name, expires_at
    $$;

REVOKE ALL ON FUNCTION kura_scoped_token_lookup(uuid), kura_scoped_token_touch(uuid), kura_scoped_token_purge(),
    kura_token_upload_purge(), kura_write_token_tenants(), kura_tokens_expiring() FROM PUBLIC;
GRANT EXECUTE ON FUNCTION kura_scoped_token_lookup(uuid), kura_scoped_token_touch(uuid), kura_scoped_token_purge(),
    kura_token_upload_purge(), kura_write_token_tenants(), kura_tokens_expiring() TO kura_app;

-- ============================================================================
-- History and retention (RFC 0001 §6.3, §6.9; RFC 0005 §4.5)
-- ============================================================================
-- Every job crosses tenants, so each runs through a named definer function,
-- in batches.

-- Who wrote each revision, as its readers see it: the device's name
-- ("laptop"), or the scoped token's ("discord-recap"), never the OIDC
-- subject. Device rows are their owner's under row-level security, so this
-- definer function answers for the current tenant's vault only, and only for
-- the revisions asked.
CREATE FUNCTION kura_revision_authors(p_vault uuid, p_revisions uuid[])
    RETURNS TABLE (id uuid, kind text, name text)
    LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        SELECT r.id,
               CASE WHEN r.user_sub LIKE 'key:%' THEN 'key' ELSE 'device' END,
               CASE WHEN r.user_sub LIKE 'key:%' THEN t.name ELSE d.name END
        FROM revision r
        LEFT JOIN device d ON d.id = r.device_id AND r.user_sub NOT LIKE 'key:%'
        LEFT JOIN scoped_token t ON t.id = r.device_id AND r.user_sub LIKE 'key:%'
        WHERE r.vault_id = p_vault AND r.tenant_id = kura_current_tenant() AND r.id = ANY(p_revisions)
    $$;

-- Revisions past the policy, at most p_limit: older than p_max_age, or beyond
-- the newest p_max_per_entry of their entry. An entry's head is never pruned,
-- a deletion's included: deleted entries leave only through the purge below,
-- so every device sees the deletion first (§6.9). A policy below one hour or
-- one revision prunes nothing: no caller can pass one that empties history.
-- shortcut: each batch ranks the vault's whole history; walk revision_history
-- per entry if a vault's batch nears the 5 s statement_timeout.
CREATE FUNCTION kura_prune_revisions(p_vault uuid, p_max_age interval, p_max_per_entry integer, p_limit integer)
    RETURNS bigint
    LANGUAGE sql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        WITH ranked AS (
            SELECT r.id, r.created_at, e.head_revision,
                   row_number() OVER (PARTITION BY r.entry_id ORDER BY r.seq DESC) AS rn
            FROM revision r JOIN entry e ON e.vault_id = r.vault_id AND e.id = r.entry_id
            WHERE r.vault_id = p_vault
        ), doomed AS (
            SELECT id FROM ranked
            WHERE id <> head_revision
              AND ((p_max_per_entry >= 1 AND rn > p_max_per_entry)
                OR (p_max_age >= interval '1 hour' AND created_at < now() - p_max_age))
            LIMIT p_limit
        ), gone AS (
            DELETE FROM revision WHERE id IN (SELECT id FROM doomed) RETURNING 1
        )
        SELECT count(*) FROM gone
    $$;

-- Deleted entries whose deletion is older than p_ttl, at most p_limit, purged
-- with their history under the vault lock (no commit interleaves); the
-- horizon moves to the last of them, so an older cursor gets 410 (§6.3).
CREATE FUNCTION kura_purge_deleted(p_vault uuid, p_ttl interval, p_limit integer) RETURNS bigint
    LANGUAGE plpgsql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
    DECLARE
        ids uuid[];
        horizon bigint;
    BEGIN
        IF p_ttl IS NULL OR p_ttl < interval '1 day' THEN
            RETURN 0;
        END IF;
        PERFORM 1 FROM vault WHERE id = p_vault FOR UPDATE;
        SELECT array_agg(d.id), max(d.last_seq) INTO ids, horizon FROM (
            SELECT e.id, e.last_seq
            FROM entry e JOIN revision h ON h.id = e.head_revision
            WHERE e.vault_id = p_vault AND e.deleted AND h.created_at < now() - p_ttl
            ORDER BY e.last_seq
            LIMIT p_limit
        ) d;
        IF ids IS NULL THEN
            RETURN 0;
        END IF;
        DELETE FROM revision WHERE vault_id = p_vault AND entry_id = ANY (ids);
        DELETE FROM entry WHERE vault_id = p_vault AND id = ANY (ids);
        UPDATE vault SET horizon_seq = greatest(horizon_seq, horizon) WHERE id = p_vault;
        RETURN cardinality(ids);
    END
    $$;

REVOKE ALL ON FUNCTION kura_revision_authors(uuid, uuid[]), kura_prune_revisions(uuid, interval, integer, integer),
    kura_purge_deleted(uuid, interval, integer) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION kura_revision_authors(uuid, uuid[]), kura_prune_revisions(uuid, interval, integer, integer),
    kura_purge_deleted(uuid, interval, integer) TO kura_app;

-- ============================================================================
-- Search (RFC 0001 §6.7, decision 39)
-- ============================================================================
-- Opt-in. The index holds the words of each Markdown head in plaintext, in
-- this database and its dumps: a tenant has none unless `search.enabled`. Both
-- extensions are trusted, so the database owner creates them.
CREATE EXTENSION IF NOT EXISTS unaccent;
CREATE EXTENSION IF NOT EXISTS pg_trgm;

-- French and English with accents folded (§6.7).
CREATE TEXT SEARCH CONFIGURATION kura_french (COPY = pg_catalog.french);
ALTER TEXT SEARCH CONFIGURATION kura_french
    ALTER MAPPING FOR hword, hword_part, word WITH unaccent, french_stem;
CREATE TEXT SEARCH CONFIGURATION kura_english (COPY = pg_catalog.english);
ALTER TEXT SEARCH CONFIGURATION kura_english
    ALTER MAPPING FOR hword, hword_part, word WITH unaccent, english_stem;

-- The head revision's lexemes only, one row per entry (§6.1).
CREATE TABLE search_doc (
    vault_id    uuid NOT NULL,
    tenant_id   uuid NOT NULL,
    entry_id    uuid NOT NULL,
    revision_id uuid NOT NULL,
    lex_fr      tsvector NOT NULL,
    lex_en      tsvector NOT NULL,
    PRIMARY KEY (vault_id, entry_id),
    FOREIGN KEY (vault_id, tenant_id) REFERENCES vault (id, tenant_id) ON DELETE CASCADE
);
CREATE INDEX search_doc_fr ON search_doc USING gin (lex_fr);
CREATE INDEX search_doc_en ON search_doc USING gin (lex_en);
CREATE INDEX search_doc_tenant ON search_doc (tenant_id);
-- Fuzzy file names (§6.7).
CREATE INDEX entry_path_trgm ON entry USING gin (path gin_trgm_ops);

-- One pending job per entry, inserted by the commit that changed it; a
-- newer commit replaces it. Claimed for 5 minutes at a time, failed after
-- 5 attempts and kept (counted in kura_index_queue_depth, §6.1).
CREATE TABLE index_job (
    vault_id    uuid NOT NULL,
    tenant_id   uuid NOT NULL,
    entry_id    uuid NOT NULL,
    revision_id uuid NOT NULL,
    attempts    integer NOT NULL DEFAULT 0,
    claimed_at  timestamptz,
    failed_at   timestamptz,
    created_at  timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (vault_id, entry_id),
    FOREIGN KEY (vault_id, tenant_id) REFERENCES vault (id, tenant_id) ON DELETE CASCADE
);
CREATE INDEX index_job_pending ON index_job (created_at) WHERE failed_at IS NULL;
CREATE INDEX index_job_tenant ON index_job (tenant_id);

DO $$
DECLARE
    t text;
BEGIN
    FOREACH t IN ARRAY ARRAY['search_doc', 'index_job'] LOOP
        EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY', t);
        EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY', t);
        EXECUTE format('CREATE POLICY owner_all ON %I TO %I USING (true) WITH CHECK (true)', t, current_user);
        EXECUTE format('CREATE POLICY app_tenant ON %I TO kura_app USING (tenant_id = kura_current_tenant()) WITH CHECK (tenant_id = kura_current_tenant())', t);
    END LOOP;
END
$$;
GRANT SELECT ON search_doc TO kura_app;
GRANT SELECT, INSERT, UPDATE ON index_job TO kura_app;

-- The worker's view of the tenants: backfill the newly enabled, drop the
-- newly disabled. Returns how many jobs were queued and docs dropped. Search
-- never indexes configuration (plugin READMEs are Markdown too).
CREATE FUNCTION kura_search_sync() RETURNS TABLE (queued bigint, dropped bigint)
    LANGUAGE plpgsql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
    DECLARE
        q bigint := 0;
        d bigint := 0;
        n bigint;
        t record;
    BEGIN
        FOR t IN SELECT id FROM tenant WHERE search_enabled AND NOT search_indexed AND disabled_at IS NULL FOR UPDATE SKIP LOCKED LOOP
            INSERT INTO index_job (vault_id, tenant_id, entry_id, revision_id)
            SELECT e.vault_id, e.tenant_id, e.id, e.head_revision
            FROM entry e JOIN vault v ON v.id = e.vault_id
            WHERE e.tenant_id = t.id AND e.namespace = 'notes' AND NOT e.deleted
              AND v.disabled_at IS NULL AND lower(e.path) LIKE '%.md'
            ON CONFLICT (vault_id, entry_id) DO NOTHING;
            GET DIAGNOSTICS n = ROW_COUNT;
            q := q + n;
            UPDATE tenant SET search_indexed = true WHERE id = t.id;
        END LOOP;
        FOR t IN SELECT id FROM tenant WHERE NOT search_enabled AND search_indexed FOR UPDATE SKIP LOCKED LOOP
            DELETE FROM index_job WHERE tenant_id = t.id;
            DELETE FROM search_doc WHERE tenant_id = t.id;
            GET DIAGNOSTICS n = ROW_COUNT;
            d := d + n;
            UPDATE tenant SET search_indexed = false WHERE id = t.id;
        END LOOP;
        RETURN QUERY SELECT q, d;
    END
    $$;

-- Up to p_limit pending jobs (§6.7: 100 a batch), claimed for 5 minutes so
-- two replicas do not do the same work, with what indexing needs.
CREATE FUNCTION kura_index_claim(p_limit integer)
    RETURNS TABLE (vault_id uuid, tenant_id uuid, slug text, entry_id uuid, revision_id uuid,
                   path text, deleted boolean, blob_hash bytea, size bigint)
    LANGUAGE sql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        WITH c AS (
            SELECT j.vault_id, j.entry_id FROM index_job j
            WHERE j.failed_at IS NULL AND (j.claimed_at IS NULL OR j.claimed_at < now() - interval '5 minutes')
            ORDER BY j.created_at
            LIMIT least(p_limit, 100)
            FOR UPDATE SKIP LOCKED
        ), claimed AS (
            UPDATE index_job j SET claimed_at = now() FROM c
            WHERE j.vault_id = c.vault_id AND j.entry_id = c.entry_id
            RETURNING j.vault_id, j.tenant_id, j.entry_id
        )
        -- The entry's current head; a configuration entry indexes as nothing.
        SELECT c.vault_id, c.tenant_id, t.slug, c.entry_id, e.head_revision, e.path,
               e.deleted OR e.namespace <> 'notes', r.blob_hash, r.size
        FROM claimed c
        JOIN tenant t ON t.id = c.tenant_id
        JOIN entry e ON e.vault_id = c.vault_id AND e.id = c.entry_id
        JOIN revision r ON r.id = e.head_revision
    $$;

-- The document of one head, or none (deleted, not Markdown): the job goes
-- unless a newer commit replaced it meanwhile. Only for a claimed job and
-- the entry's current head, so the serving role cannot write words or drop
-- documents of an arbitrary vault through it (review 7 L4).
CREATE FUNCTION kura_index_set(p_vault uuid, p_entry uuid, p_revision uuid, p_text text) RETURNS void
    LANGUAGE plpgsql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
    BEGIN
        IF NOT EXISTS (SELECT 1 FROM index_job j WHERE j.vault_id = p_vault AND j.entry_id = p_entry AND j.claimed_at IS NOT NULL)
           OR NOT EXISTS (SELECT 1 FROM entry e WHERE e.vault_id = p_vault AND e.id = p_entry AND e.head_revision = p_revision) THEN
            RETURN;
        END IF;
        IF p_text IS NULL THEN
            DELETE FROM search_doc WHERE vault_id = p_vault AND entry_id = p_entry;
        ELSE
            INSERT INTO search_doc (vault_id, tenant_id, entry_id, revision_id, lex_fr, lex_en)
            SELECT p_vault, v.tenant_id, p_entry, p_revision,
                   to_tsvector('kura_french', p_text), to_tsvector('kura_english', p_text)
            FROM vault v WHERE v.id = p_vault
            ON CONFLICT (vault_id, entry_id) DO UPDATE
                SET revision_id = EXCLUDED.revision_id, lex_fr = EXCLUDED.lex_fr, lex_en = EXCLUDED.lex_en;
        END IF;
        DELETE FROM index_job j
        WHERE j.vault_id = p_vault AND j.entry_id = p_entry
          AND (j.revision_id = p_revision OR NOT EXISTS (
                SELECT 1 FROM entry e WHERE e.vault_id = p_vault AND e.id = p_entry AND e.head_revision <> p_revision));
    END
    $$;

-- A job that failed: it stays claimed, so the 5-minute reclaim of
-- kura_index_claim is the wait between attempts (a short outage must not use
-- all five); at most 5 attempts, then kept as failed.
CREATE FUNCTION kura_index_failed(p_vault uuid, p_entry uuid) RETURNS void
    LANGUAGE sql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        UPDATE index_job SET attempts = attempts + 1, claimed_at = now(),
               failed_at = CASE WHEN attempts + 1 >= 5 THEN now() END
        WHERE vault_id = p_vault AND entry_id = p_entry AND claimed_at IS NOT NULL
    $$;

CREATE FUNCTION kura_index_depth() RETURNS TABLE (pending bigint, failed bigint)
    LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        SELECT count(*) FILTER (WHERE failed_at IS NULL), count(*) FILTER (WHERE failed_at IS NOT NULL)
        FROM index_job
    $$;

REVOKE ALL ON FUNCTION kura_search_sync(), kura_index_claim(integer), kura_index_set(uuid, uuid, uuid, text),
    kura_index_failed(uuid, uuid), kura_index_depth() FROM PUBLIC;
GRANT EXECUTE ON FUNCTION kura_search_sync(), kura_index_claim(integer), kura_index_set(uuid, uuid, uuid, text),
    kura_index_failed(uuid, uuid), kura_index_depth() TO kura_app;

-- ============================================================================
-- Notifications (RFC 0003 §6.4)
-- ============================================================================
-- Rules, the events they subscribe to, and their deliveries. Every table
-- carries tenant_id and row-level security. Events and deliveries live 7 days.

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

-- The rules stored across tenants, for the startup warning while
-- `notifications.enabled` is false (§4.3). A count only, no tenant in it.
CREATE FUNCTION kura_notification_rule_count() RETURNS bigint
    LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$ SELECT count(*) FROM notification_rule $$;

REVOKE ALL ON FUNCTION kura_notification_claim(integer), kura_notification_subscriptions(), kura_notification_stats(),
    kura_notification_purge(), kura_notification_rule_count() FROM PUBLIC;
GRANT EXECUTE ON FUNCTION kura_notification_claim(integer), kura_notification_subscriptions(), kura_notification_stats(),
    kura_notification_purge(), kura_notification_rule_count() TO kura_app;

-- ============================================================================
-- Device sync (RFC 0006)
-- ============================================================================

-- Where each device stands on each vault it syncs. One row per (device,
-- vault), overwritten, never a timeline (§3, §9). Written by the server from
-- the device's own calls; read through kura_vault_sync() only, since `device`
-- rows are visible to their owner alone.
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
    -- What the device's last run left on its side, as counts only (question
    -- 2). Overwritten by each report, like the rest of the row.
    pending       integer CHECK (pending >= 0),
    conflicts     integer CHECK (conflicts >= 0),
    unsyncable    integer CHECK (unsyncable >= 0),
    failed        integer CHECK (failed >= 0),
    reported_at   timestamptz,
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
    RETURNS TABLE (device_id uuid, name text, profile text, user_sub text, owner_name text, revoked boolean,
                   epoch uuid, caught_up_seq bigint, caught_up_at timestamptz,
                   pushed_at timestamptz, behind bigint,
                   pending integer, conflicts integer, unsyncable integer, failed integer,
                   reported_at timestamptz)
    LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        SELECT d.id, d.name, d.profile, d.user_sub, d.owner_name, d.revoked_at IS NOT NULL,
               s.epoch, s.caught_up_seq, s.caught_up_at, s.pushed_at,
               CASE WHEN s.caught_up_seq IS NULL THEN NULL ELSE (
                   SELECT count(*) FROM (
                       SELECT 1 FROM entry e
                       WHERE e.vault_id = s.vault_id AND e.namespace = 'notes'
                         AND e.last_seq > s.caught_up_seq
                         AND e.path_key LIKE ANY (coalesce(s.scope, ARRAY['%']))
                       LIMIT 1001) b)
               END,
               s.pending, s.conflicts, s.unsyncable, s.failed, s.reported_at
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

-- ============================================================================
-- Schema floor (RFC 0004 decision 11)
-- ============================================================================

-- The oldest binary, by its newest migration, that may serve this schema.
-- `check_schema` refuses to start below it. A migration not safe to roll back
-- raises it: UPDATE schema_floor SET min_binary_migration = greatest(min_binary_migration, <its version>)
CREATE TABLE schema_floor (
    one                  boolean PRIMARY KEY DEFAULT true CHECK (one),
    min_binary_migration bigint NOT NULL
);
INSERT INTO schema_floor (min_binary_migration) VALUES (1);
ALTER TABLE schema_floor ENABLE ROW LEVEL SECURITY;
ALTER TABLE schema_floor FORCE ROW LEVEL SECURITY;
DO $$
BEGIN
    EXECUTE format('CREATE POLICY owner_all ON schema_floor TO %I USING (true) WITH CHECK (true)', current_user);
END
$$;
CREATE POLICY app_read ON schema_floor FOR SELECT TO kura_app USING (true);
GRANT SELECT ON schema_floor TO kura_app;

-- ============================================================================
-- Maintenance: epochs, garbage collection, the Delete purge, metrics, backups
-- (RFC 0001 §6.3, §6.9, §10; decisions 24, 32, 33)
-- ============================================================================
-- Every job crosses tenants, so each runs through a named definer function,
-- in batches.

-- The cluster the vaults' epochs were issued under. PostgreSQL's system
-- identifier changes on a restore into a new cluster, its timeline on every
-- promotion of a standby: either way history may have been lost, so every
-- vault gets a new epoch and devices resync without deleting (§6.3).
CREATE TABLE cluster_identity (
    id                boolean PRIMARY KEY DEFAULT true CHECK (id),
    system_identifier bigint  NOT NULL,
    timeline          integer NOT NULL,
    checked_at        timestamptz NOT NULL DEFAULT now()
);

-- Each completed bucket copy, so a missed or failing backup alerts
-- (kura_backup_last_success_timestamp_seconds). Written by `kura-server
-- backup` through kura_backup_done; read by the metrics refresh.
CREATE TABLE backup_run (
    recover_to  timestamptz PRIMARY KEY,
    objects     bigint NOT NULL,
    finished_at timestamptz NOT NULL DEFAULT now()
);

DO $$
DECLARE
    t text;
BEGIN
    FOREACH t IN ARRAY ARRAY['cluster_identity', 'backup_run'] LOOP
        EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY', t);
        EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY', t);
        EXECUTE format('CREATE POLICY owner_all ON %I TO %I USING (true) WITH CHECK (true)', t, current_user);
    END LOOP;
END
$$;

-- Records the identity; when it changed, gives every vault a new epoch and
-- returns how many. The first record changes nothing. Two replicas racing:
-- the second waits on the row, then finds it current.
CREATE FUNCTION kura_epoch_check() RETURNS bigint
    LANGUAGE plpgsql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
    DECLARE
        sysid bigint;
        -- The timeline of the WAL being written, current from the promotion
        -- on (the checkpoint's lags until the next checkpoint).
        tl integer := ('x' || substr(pg_walfile_name(pg_current_wal_lsn()), 1, 8))::bit(32)::integer;
        updated boolean;
        n bigint := 0;
    BEGIN
        SELECT system_identifier INTO sysid FROM pg_control_system();
        INSERT INTO cluster_identity (system_identifier, timeline) VALUES (sysid, tl)
        ON CONFLICT (id) DO UPDATE
            SET system_identifier = EXCLUDED.system_identifier, timeline = EXCLUDED.timeline, checked_at = now()
            WHERE cluster_identity.system_identifier <> EXCLUDED.system_identifier
               OR cluster_identity.timeline <> EXCLUDED.timeline
        RETURNING xmax::text <> '0' INTO updated;
        IF updated THEN
            UPDATE vault SET epoch = gen_random_uuid();
            GET DIAGNOSTICS n = ROW_COUNT;
        END IF;
        RETURN n;
    END
    $$;

-- The restore runbook's explicit step (decision 32), run as the owner.
CREATE FUNCTION kura_epoch_reset() RETURNS bigint
    LANGUAGE plpgsql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
    DECLARE
        n bigint;
    BEGIN
        UPDATE vault SET epoch = gen_random_uuid();
        GET DIAGNOSTICS n = ROW_COUNT;
        RETURN n;
    END
    $$;

-- Every vault the maintenance pass visits, with its tenant's retention
-- policy; `due` when a Delete grace period has passed (decision 33).
CREATE FUNCTION kura_maintenance_vaults()
    RETURNS TABLE (vault_id uuid, retention jsonb, active boolean, due boolean)
    LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        SELECT v.id, t.retention_policy,
               v.disabled_at IS NULL AND t.disabled_at IS NULL,
               coalesce(v.delete_after <= now(), false)
                 OR (coalesce(t.delete_after <= now(), false) AND NOT v.force_orphan)
        FROM vault v JOIN tenant t ON t.id = v.tenant_id
        ORDER BY v.id
    $$;

-- Garbage collection (§6.9, decision 24). Blobs no revision references,
-- untouched 24 hours, locked so a commit taking them (FOR KEY SHARE) is
-- skipped, then rechecked in a new snapshot.
CREATE FUNCTION kura_gc_blobs(p_vault uuid, p_limit integer) RETURNS bigint
    LANGUAGE plpgsql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
    DECLARE
        hashes bytea[];
        n bigint;
    BEGIN
        SELECT array_agg(hash) INTO hashes FROM (
            SELECT b.hash FROM blob b
            WHERE b.vault_id = p_vault AND b.last_touched < now() - interval '24 hours'
              AND NOT EXISTS (SELECT 1 FROM revision r WHERE r.vault_id = b.vault_id AND r.blob_hash = b.hash)
            ORDER BY b.last_touched
            LIMIT p_limit
            FOR UPDATE SKIP LOCKED
        ) c;
        IF hashes IS NULL THEN
            RETURN 0;
        END IF;
        DELETE FROM blob b
        WHERE b.vault_id = p_vault AND b.hash = ANY (hashes)
          AND NOT EXISTS (SELECT 1 FROM revision r WHERE r.vault_id = b.vault_id AND r.blob_hash = b.hash);
        GET DIAGNOSTICS n = ROW_COUNT;
        RETURN n;
    END
    $$;

-- Chunks no manifest references, untouched 24 hours: locked FOR UPDATE SKIP
-- LOCKED, so a blob PUT holding them FOR KEY SHARE is skipped and one
-- arriving later waits, then finds them gone and asks for a re-upload;
-- rechecked, deleted, and their object keys returned for the caller to
-- delete once this commits (decision 24).
CREATE FUNCTION kura_gc_chunks(p_vault uuid, p_limit integer) RETURNS SETOF text
    LANGUAGE plpgsql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
    DECLARE
        hashes bytea[];
    BEGIN
        SELECT array_agg(hash) INTO hashes FROM (
            SELECT c.hash FROM chunk c
            WHERE c.vault_id = p_vault AND c.last_touched < now() - interval '24 hours'
              AND NOT EXISTS (SELECT 1 FROM blob_chunk b WHERE b.vault_id = c.vault_id AND b.chunk_hash = c.hash)
            ORDER BY c.last_touched
            LIMIT p_limit
            FOR UPDATE SKIP LOCKED
        ) l;
        IF hashes IS NULL THEN
            RETURN;
        END IF;
        RETURN QUERY
            DELETE FROM chunk c
            WHERE c.vault_id = p_vault AND c.hash = ANY (hashes)
              AND NOT EXISTS (SELECT 1 FROM blob_chunk b WHERE b.vault_id = c.vault_id AND b.chunk_hash = c.hash)
            RETURNING c.object_key;
    END
    $$;

-- The Delete purge (decision 33). One batch of a vault's rows, at most
-- p_limit, table by table; 0 when only the vault row is left, -1 when the
-- vault is no longer due (its document came back): checked in every batch,
-- under a lock the reconciler's restore waits on.
CREATE FUNCTION kura_purge_vault(p_vault uuid, p_limit integer) RETURNS bigint
    LANGUAGE plpgsql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
    DECLARE
        n bigint;
        ids uuid[];
    BEGIN
        PERFORM 1 FROM vault v JOIN tenant t ON t.id = v.tenant_id
        WHERE v.id = p_vault
          AND (v.delete_after <= now() OR (t.delete_after <= now() AND NOT v.force_orphan))
        FOR UPDATE OF v FOR SHARE OF t;
        IF NOT FOUND THEN
            RETURN -1;
        END IF;
        -- Every batch, so a vault removed, restored and removed again is
        -- covered on each cycle; it is disabled, so the churn costs nothing.
        UPDATE vault SET purge_started_at = coalesce(purge_started_at, now()), epoch = gen_random_uuid()
        WHERE id = p_vault;
        DELETE FROM commit_record WHERE ctid = ANY (ARRAY(SELECT ctid FROM commit_record WHERE vault_id = p_vault LIMIT p_limit));
        GET DIAGNOSTICS n = ROW_COUNT;
        IF n > 0 THEN RETURN n; END IF;
        DELETE FROM ws_ticket WHERE vault_id = p_vault;
        -- An entry leaves with its whole history in one batch: a vault
        -- restored mid-purge never holds an entry without its head.
        SELECT array_agg(id) INTO ids FROM (SELECT id FROM entry WHERE vault_id = p_vault LIMIT p_limit) e;
        IF ids IS NOT NULL THEN
            DELETE FROM revision WHERE vault_id = p_vault AND entry_id = ANY (ids);
            DELETE FROM entry WHERE vault_id = p_vault AND id = ANY (ids);
            RETURN cardinality(ids);
        END IF;
        DELETE FROM blob WHERE (vault_id, hash) IN (SELECT vault_id, hash FROM blob WHERE vault_id = p_vault LIMIT p_limit);
        GET DIAGNOSTICS n = ROW_COUNT;
        IF n > 0 THEN RETURN n; END IF;
        DELETE FROM chunk WHERE (vault_id, hash) IN (SELECT vault_id, hash FROM chunk WHERE vault_id = p_vault LIMIT p_limit);
        GET DIAGNOSTICS n = ROW_COUNT;
        RETURN n;
    END
    $$;

-- Once the rows and the objects are gone: the vault row (its bindings
-- cascade), and the tenant marked purged when it was its last vault. The
-- tenant row stays until an operator destroys its Transit key (decision 33).
CREATE FUNCTION kura_purge_vault_finish(p_vault uuid) RETURNS boolean
    LANGUAGE plpgsql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
    DECLARE
        tenant uuid;
        tenant_due boolean;
    BEGIN
        SELECT v.tenant_id, coalesce(t.delete_after <= now(), false) INTO tenant, tenant_due
        FROM vault v JOIN tenant t ON t.id = v.tenant_id
        WHERE v.id = p_vault
          AND (v.delete_after <= now() OR (t.delete_after <= now() AND NOT v.force_orphan))
        FOR UPDATE OF v FOR SHARE OF t;
        IF NOT FOUND OR EXISTS (SELECT 1 FROM chunk WHERE vault_id = p_vault) THEN
            RETURN false;
        END IF;
        DELETE FROM vault WHERE id = p_vault;
        IF tenant_due AND NOT EXISTS (SELECT 1 FROM vault WHERE tenant_id = tenant) THEN
            UPDATE tenant SET purged_at = now() WHERE id = tenant AND purged_at IS NULL;
        END IF;
        RETURN true;
    END
    $$;

-- Tenants purged by Delete whose Transit key an operator must still destroy.
CREATE FUNCTION kura_tenants_awaiting_key_destruction() RETURNS bigint
    LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$ SELECT count(*) FROM tenant WHERE purged_at IS NOT NULL AND disabled_at IS NOT NULL $$;

-- Storage per tenant for the metrics of §10.1 (chunk_bytes_stored,
-- dedup_ratio, quota_used_ratio). A cross-tenant read through a named
-- function, as kura_tenant_usage. Sizes only: no name, no path.
CREATE FUNCTION kura_tenant_storage()
    RETURNS TABLE (slug text, stored bigint, logical bigint, quota bigint)
    LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        SELECT t.slug,
               coalesce((SELECT sum(c.size) FROM chunk c WHERE c.tenant_id = t.id), 0)::bigint,
               coalesce((SELECT sum(r.size) FROM entry e JOIN revision r ON r.id = e.head_revision
                         WHERE e.tenant_id = t.id AND NOT e.deleted), 0)::bigint,
               t.quota_bytes
        FROM tenant t
        WHERE t.disabled_at IS NULL
    $$;

-- Tenants and vaults removed with Delete, waiting for their grace period
-- (decision 33): an alert, so an unnoticed removal is noticed.
CREATE FUNCTION kura_deletions_scheduled() RETURNS bigint
    LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        SELECT (SELECT count(*) FROM tenant WHERE delete_after IS NOT NULL AND purged_at IS NULL)
             + (SELECT count(*) FROM vault WHERE delete_after IS NOT NULL)
    $$;

-- The restore drill's last step decrypts a random sample of chunks (§10.3);
-- returns what opening one needs, never content.
CREATE FUNCTION kura_sample_chunks(p_count integer)
    RETURNS TABLE (vault_id uuid, tenant_id uuid, slug text, hash bytea, object_key text)
    LANGUAGE sql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        SELECT c.vault_id, c.tenant_id, t.slug, c.hash, c.object_key
        FROM chunk c JOIN tenant t ON t.id = c.tenant_id
        ORDER BY random()
        LIMIT least(greatest(p_count, 0), 10000)
    $$;

CREATE FUNCTION kura_backup_done(p_recover_to timestamptz, p_objects bigint) RETURNS void
    LANGUAGE sql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        INSERT INTO backup_run (recover_to, objects) VALUES (p_recover_to, p_objects)
        ON CONFLICT (recover_to) DO NOTHING;
        DELETE FROM backup_run WHERE finished_at < now() - interval '90 days';
    $$;

CREATE FUNCTION kura_backup_last() RETURNS timestamptz
    LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$ SELECT max(finished_at) FROM backup_run $$;

-- kura_epoch_reset and kura_sample_chunks are owner only: content addresses
-- and object keys of every tenant never reach the serving role
-- (`check-chunks` runs with the owner's URL, no timeout).
REVOKE ALL ON FUNCTION kura_epoch_check(), kura_epoch_reset(), kura_maintenance_vaults(),
    kura_gc_blobs(uuid, integer), kura_gc_chunks(uuid, integer),
    kura_purge_vault(uuid, integer), kura_purge_vault_finish(uuid),
    kura_tenants_awaiting_key_destruction(), kura_tenant_storage(), kura_deletions_scheduled(),
    kura_sample_chunks(integer), kura_backup_done(timestamptz, bigint), kura_backup_last() FROM PUBLIC;
GRANT EXECUTE ON FUNCTION kura_epoch_check(), kura_maintenance_vaults(),
    kura_gc_blobs(uuid, integer), kura_gc_chunks(uuid, integer),
    kura_purge_vault(uuid, integer), kura_purge_vault_finish(uuid),
    kura_tenants_awaiting_key_destruction(), kura_tenant_storage(), kura_deletions_scheduled(),
    kura_backup_done(timestamptz, bigint), kura_backup_last() TO kura_app;
