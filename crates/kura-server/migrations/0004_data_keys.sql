-- PR 2.3: one data key per vault, wrapped by the tenant's Transit key and
-- written once (RFC 0001 §6.6, decision 29). Additive only.
ALTER TABLE vault ADD COLUMN wrapped_dek text;
-- AES-GCM with random nonces: about 2^32 encryptions per key; warn at 2^30.
ALTER TABLE vault ADD COLUMN encryptions bigint NOT NULL DEFAULT 0;
