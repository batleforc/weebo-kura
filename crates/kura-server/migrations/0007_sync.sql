-- PR 3.2: entries, revisions and commits (RFC 0001 §6.1, §6.3). Every row
-- carries its tenant and is isolated by it. Additive only.

-- The vault row is the commit lock; head_seq is the last assigned seq.
-- epoch changes when the server loses history (phase 5, decision 32);
-- horizon_seq is where purged deletions end (phase 5).
ALTER TABLE vault
    ADD COLUMN epoch       uuid   NOT NULL DEFAULT gen_random_uuid(),
    ADD COLUMN head_seq    bigint NOT NULL DEFAULT 0 CHECK (head_seq >= 0),
    ADD COLUMN horizon_seq bigint NOT NULL DEFAULT 0 CHECK (horizon_seq >= 0);

-- One file of a vault, under a stable id chosen by the client (UUIDv7), so a
-- rename keeps its history. path_key is computed by the server only (§6.3).
CREATE TABLE entry (
    vault_id      uuid    NOT NULL,
    tenant_id     uuid    NOT NULL,
    id            uuid    NOT NULL,
    namespace     text    NOT NULL DEFAULT 'notes' CHECK (namespace = 'notes'),
    path          text    NOT NULL CHECK (octet_length(path) BETWEEN 1 AND 4096),
    path_key      text    NOT NULL,
    head_revision uuid    NOT NULL,
    last_seq      bigint  NOT NULL CHECK (last_seq > 0),
    deleted       boolean NOT NULL DEFAULT false,
    PRIMARY KEY (vault_id, id),
    FOREIGN KEY (vault_id, tenant_id) REFERENCES vault (id, tenant_id)
);
-- Changes after a cursor; path lookup and uniqueness among live entries (§6.1).
CREATE INDEX entry_changes ON entry (vault_id, last_seq);
CREATE UNIQUE INDEX entry_path ON entry (vault_id, namespace, path_key) WHERE NOT deleted;
CREATE INDEX entry_tenant ON entry (tenant_id);

-- Immutable. device_id has no foreign key: purging a device keeps attribution.
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
    CHECK (deleted = (blob_hash IS NULL)),
    FOREIGN KEY (vault_id, entry_id) REFERENCES entry (vault_id, id),
    FOREIGN KEY (vault_id, blob_hash) REFERENCES blob (vault_id, hash),
    FOREIGN KEY (vault_id, tenant_id) REFERENCES vault (id, tenant_id)
);
-- History of an entry (§6.1).
CREATE INDEX revision_history ON revision (vault_id, entry_id, seq);
CREATE INDEX revision_tenant ON revision (tenant_id);

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
        EXECUTE format('CREATE POLICY app_tenant ON %I TO kura_app USING (tenant_id = kura_current_tenant()) WITH CHECK (tenant_id = kura_current_tenant())', t);
    END LOOP;
END
$$;
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
REVOKE ALL ON FUNCTION kura_commit_purge() FROM PUBLIC;
GRANT EXECUTE ON FUNCTION kura_commit_purge() TO kura_app;
