-- RFC 0003 §4.3: the rules stored across tenants, for the startup warning
-- while `notifications.enabled` is false. A count only, no tenant in it.
CREATE FUNCTION kura_notification_rule_count() RETURNS bigint
    LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$ SELECT count(*) FROM notification_rule $$;

REVOKE ALL ON FUNCTION kura_notification_rule_count() FROM PUBLIC;
GRANT EXECUTE ON FUNCTION kura_notification_rule_count() TO kura_app;
