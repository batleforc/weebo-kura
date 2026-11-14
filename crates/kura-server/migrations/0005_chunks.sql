-- PR 2.4: chunks and blobs (RFC 0001 §6.1, §6.4). Every row carries its
-- tenant and is isolated by it, like the phase 1 tables. Additive only.

-- A row's tenant must be its vault's tenant, enforced by the database, not
-- only by the handlers (phase 2 review P6).
ALTER TABLE vault ADD CONSTRAINT vault_id_tenant UNIQUE (id, tenant_id);

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
-- Garbage collection sweep (phase 5).
CREATE INDEX chunk_last_touched ON chunk (last_touched);
CREATE INDEX chunk_tenant ON chunk (tenant_id);

-- The manifest of one file version: the ordered list of its chunks.
CREATE TABLE blob (
    vault_id   uuid NOT NULL,
    tenant_id  uuid NOT NULL,
    hash       bytea NOT NULL CHECK (octet_length(hash) = 32),
    size       bigint NOT NULL CHECK (size >= 0),
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (vault_id, hash),
    FOREIGN KEY (vault_id, tenant_id) REFERENCES vault (id, tenant_id)
);

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
