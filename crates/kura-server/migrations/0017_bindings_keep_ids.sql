-- Review 2026-11-15 #2: re-applying the configuration deleted and re-inserted
-- every file-declared binding, so each got a new id. A scoped token keeps the
-- id of the manager binding that allowed it (`granted_by_binding`, ON DELETE
-- SET NULL): any configuration change, even an unrelated one, cut it. Bindings
-- still declared now keep their row (and id); only those gone are deleted,
-- and a changed role is updated in place. API-managed bindings stay untouched.
CREATE OR REPLACE FUNCTION kura_bindings_replace(p_vault uuid, p_bindings jsonb) RETURNS void
    LANGUAGE sql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        DELETE FROM role_binding rb
        WHERE rb.vault_id = p_vault AND rb.managed_by = 'config'
          AND NOT EXISTS (
              SELECT 1 FROM jsonb_array_elements(p_bindings) b
              WHERE b->>'kind' = rb.subject_kind AND b->>'subject' = rb.subject
          );
        -- One row per subject (the first declared), so an upsert never meets
        -- the same row twice.
        INSERT INTO role_binding (tenant_id, vault_id, subject_kind, subject, role, managed_by)
            SELECT DISTINCT ON (b->>'kind', b->>'subject')
                   v.tenant_id, v.id, b->>'kind', b->>'subject', b->>'role', 'config'
            FROM vault v, jsonb_array_elements(p_bindings) WITH ORDINALITY AS x(b, n)
            WHERE v.id = p_vault
            ORDER BY b->>'kind', b->>'subject', n
            ON CONFLICT (vault_id, subject_kind, subject) DO UPDATE SET role = EXCLUDED.role
                WHERE role_binding.managed_by = 'config';
    $$;
