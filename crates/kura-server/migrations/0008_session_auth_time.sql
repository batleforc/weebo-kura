-- D34 follow-up: Authentik drops `sid` from tokens issued on a refresh but
-- keeps `auth_time`, the time the session signed in. A device records both;
-- a revoked session is matched by either (sid, or the owner's auth_time).
ALTER TABLE device ALTER COLUMN session_sid DROP NOT NULL;
ALTER TABLE device ADD COLUMN session_auth_time bigint;
ALTER TABLE device ADD CONSTRAINT device_session
    CHECK (session_sid IS NOT NULL OR session_auth_time IS NOT NULL);
CREATE INDEX device_auth_time ON device (user_sub, session_auth_time) WHERE revoked_at IS NOT NULL;
