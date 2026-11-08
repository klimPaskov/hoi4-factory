-- 0703_user_key_probe (down).
ALTER TABLE user_api_keys
    DROP COLUMN IF EXISTS probe_observed_at,
    DROP COLUMN IF EXISTS probe_capabilities,
    DROP COLUMN IF EXISTS probe_model;
