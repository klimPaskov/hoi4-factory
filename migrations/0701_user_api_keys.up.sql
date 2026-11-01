-- 0701_user_api_keys (up): bring-your-own model keys (ADR-016), sealed per owner.
--
-- `sealed` holds the AES-256-GCM value produced by the accounts crate's SecretStore. Its
-- authenticated context is user_api_key:v1:<account>:<key>:<provider>, so a ciphertext moved to
-- another row or account does not open. Only the last four characters are kept in clear (key_hint).
--
-- Row-level security: a key is visible only to its owning account, for every command.
-- This is the one table where the application role may DELETE: a user may remove their own
-- secret for good. Every other broker table keeps no DELETE policy.

CREATE TABLE user_api_keys (
    id           uuid PRIMARY KEY,
    account_id   uuid NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    provider     text NOT NULL CHECK (provider IN ('anthropic', 'openai', 'openai_compatible')),
    label        text NOT NULL CHECK (char_length(label) BETWEEN 1 AND 64),
    base_url     text CHECK (char_length(base_url) <= 512),
    sealed       bytea NOT NULL,
    key_hint     text NOT NULL CHECK (char_length(key_hint) = 4),
    created_at   timestamptz NOT NULL DEFAULT now(),
    last_used_at timestamptz,
    revoked_at   timestamptz,
    CONSTRAINT user_api_keys_base_url_provider CHECK ((provider = 'openai_compatible') = (base_url IS NOT NULL))
);
CREATE INDEX user_api_keys_account_idx ON user_api_keys (account_id, created_at DESC);

ALTER TABLE user_api_keys ENABLE ROW LEVEL SECURITY;
ALTER TABLE user_api_keys FORCE ROW LEVEL SECURITY;

CREATE POLICY user_api_keys_select ON user_api_keys FOR SELECT USING (account_id = app_current_account());
CREATE POLICY user_api_keys_insert ON user_api_keys FOR INSERT
    WITH CHECK (account_id = app_current_account() AND revoked_at IS NULL);
CREATE POLICY user_api_keys_update ON user_api_keys FOR UPDATE
    USING (account_id = app_current_account()) WITH CHECK (account_id = app_current_account());
CREATE POLICY user_api_keys_delete ON user_api_keys FOR DELETE USING (account_id = app_current_account());

GRANT SELECT, INSERT, DELETE ON user_api_keys TO factory_app;
GRANT UPDATE (last_used_at, revoked_at) ON user_api_keys TO factory_app;
