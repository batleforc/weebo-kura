-- Review 2026-11-15 #19: a scoped token is valid while the binding that
-- granted it exists with the manager role. A binding downgraded from manager
-- (by the API, or by the configuration since 0017 keeps the row) and later
-- raised again revived the token. Leaving the manager role now cuts it for
-- good, as deleting the binding does (ON DELETE SET NULL).
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
