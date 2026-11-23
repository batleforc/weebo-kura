-- RFC 0010: a member's configuration shared between their profiles
-- (`config:shared`, still theirs), and plugin code installed on a vault
-- (`config:vault`, the vault's, no owner). Reversible by dropping
-- config_share and narrowing the check once no row of the new namespaces is
-- live.
ALTER TABLE entry DROP CONSTRAINT entry_namespace_check;
ALTER TABLE entry ADD CONSTRAINT entry_namespace_check CHECK (
    namespace IN ('notes', 'config:desktop', 'config:mobile', 'config:shared', 'config:vault')
    AND (namespace IN ('notes', 'config:vault')) = (owner_sub IS NULL)
);

-- Each member's own choices of what crosses their profiles, per vault: a
-- row overrides the vault's `sharedByDefault` for that item. Visible to its
-- owner only, as their configuration is (T1).
CREATE TABLE config_share (
    tenant_id uuid    NOT NULL,
    vault_id  uuid    NOT NULL,
    owner_sub text    NOT NULL,
    item      text    NOT NULL CHECK (octet_length(item) BETWEEN 1 AND 72),
    shared    boolean NOT NULL,
    PRIMARY KEY (vault_id, owner_sub, item),
    FOREIGN KEY (vault_id, tenant_id) REFERENCES vault (id, tenant_id) ON DELETE CASCADE
);
CREATE INDEX config_share_tenant ON config_share (tenant_id);
ALTER TABLE config_share ENABLE ROW LEVEL SECURITY;
ALTER TABLE config_share FORCE ROW LEVEL SECURITY;
DO $$ BEGIN
    EXECUTE format('CREATE POLICY owner_all ON config_share TO %I USING (true) WITH CHECK (true)', current_user);
END $$;
CREATE POLICY app_tenant ON config_share TO kura_app
    USING (tenant_id = kura_current_tenant() AND owner_sub = kura_current_subject())
    WITH CHECK (tenant_id = kura_current_tenant() AND owner_sub = kura_current_subject());
GRANT SELECT, INSERT, UPDATE, DELETE ON config_share TO kura_app;
