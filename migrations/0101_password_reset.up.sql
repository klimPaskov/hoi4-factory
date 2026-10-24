-- 0101_password_reset (up): single-use, expiring password reset links. A new table only, so
-- nothing here blocks existing traffic. Only the SHA-256 digest of a token is stored.
CREATE TABLE password_reset_tokens (
    id          uuid PRIMARY KEY,
    account_id  uuid NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    token_hash  bytea NOT NULL UNIQUE,
    created_at  timestamptz NOT NULL,
    expires_at  timestamptz NOT NULL,
    consumed_at timestamptz
);
CREATE INDEX password_reset_tokens_account_idx ON password_reset_tokens (account_id);
CREATE INDEX password_reset_tokens_expires_idx ON password_reset_tokens (expires_at);

GRANT SELECT, INSERT, UPDATE, DELETE ON password_reset_tokens TO factory_app;
