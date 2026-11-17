-- RFC 0005 §4.5 (history): who wrote each revision, as its readers see it —
-- the device's name ("laptop"), or the scoped token's ("discord-recap"),
-- never the OIDC subject. Device rows are their owner's under row-level
-- security, so this definer function answers for the current tenant's
-- vault only, and only for the revisions asked.
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
REVOKE ALL ON FUNCTION kura_revision_authors(uuid, uuid[]) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION kura_revision_authors(uuid, uuid[]) TO kura_app;
