-- Run once per cluster by a superuser or the database administrator, before the
-- first migration (CloudNativePG: `postInitApplicationSQL`). RFC 0001 §6.11,
-- decision 26. The migration owner is the database owner (CloudNativePG's
-- application owner); it owns every table, function and policy.
--
-- kura_app is the role the server connects with: not the owner, never
-- superuser, never BYPASSRLS, so every forced row-level policy applies to it.
-- Its password is set by the operator (or CloudNativePG managed roles).
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'kura_app') THEN
        CREATE ROLE kura_app LOGIN;
    END IF;
END
$$;
ALTER ROLE kura_app NOSUPERUSER NOBYPASSRLS NOCREATEDB NOCREATEROLE NOREPLICATION;
-- A dropped request must not hold the vault lock (§6.11): 5 s per statement and
-- per lock wait, 10 s idle inside a transaction.
ALTER ROLE kura_app SET lock_timeout = '5s';
ALTER ROLE kura_app SET statement_timeout = '5s';
ALTER ROLE kura_app SET idle_in_transaction_session_timeout = '10s';
