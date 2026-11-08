-- 0703_user_key_probe (up): what an inference probe observed on a user's OpenAI-compatible key
-- (S04, issue #7). The probe's model and capabilities make the endpoint an agent profile, with
-- only the observed capabilities marked supported. Nothing here is a secret.
ALTER TABLE user_api_keys
    ADD COLUMN probe_model        text CHECK (char_length(probe_model) <= 200),
    ADD COLUMN probe_capabilities jsonb,
    ADD COLUMN probe_observed_at  timestamptz;
GRANT UPDATE (probe_model, probe_capabilities, probe_observed_at) ON user_api_keys TO factory_app;
