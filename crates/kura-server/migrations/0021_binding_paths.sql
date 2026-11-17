-- RFC 0005: a `viewer` binding may list 1 to 16 folders; NULL is the whole
-- vault, as before. The CHECK makes "only a viewer is restricted" hold in
-- the database: a role above viewer always reads the whole vault (§5.1).
ALTER TABLE role_binding
    ADD COLUMN paths text[],
    ADD CONSTRAINT role_binding_paths_check
        CHECK (paths IS NULL OR (role = 'viewer' AND cardinality(paths) BETWEEN 1 AND 16));

-- A binary from before this migration would read a restricted binding as the
-- whole vault: it may no longer serve (RFC 0004 decision 11).
UPDATE schema_floor SET min_binary_migration = greatest(min_binary_migration, 21);

-- `paths` from the reconciled JSON, on insert and on update, so a change of
-- `paths` alone reaches the row and keeps its id (tokens hang on it).
CREATE OR REPLACE FUNCTION kura_bindings_replace(p_vault uuid, p_bindings jsonb) RETURNS void
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
