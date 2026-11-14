-- PR 2.5: stored bytes per tenant, for the QuotaBelowUsage condition (§4.3).
-- A cross-tenant read for platform admins only, through a named function.
CREATE FUNCTION kura_tenant_usage() RETURNS TABLE (slug text, used bigint)
    LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        SELECT t.slug, coalesce(sum(c.size), 0)::bigint
        FROM tenant t LEFT JOIN chunk c ON c.tenant_id = t.id
        WHERE t.disabled_at IS NULL
        GROUP BY t.slug
    $$;
REVOKE ALL ON FUNCTION kura_tenant_usage() FROM PUBLIC;
GRANT EXECUTE ON FUNCTION kura_tenant_usage() TO kura_app;
