-- 0300_intake_uploads (up): one row per upload attempt (ZIP or folder) into a new workspace.
--
-- Written by factory-intake. The row is inserted as 'receiving' before any byte is stored, so a
-- crash leaves a visible, reconcilable record, and is completed as 'accepted' or 'rejected'.
-- The original upload itself lives on disk in the uploads area; only its locator, size and
-- SHA-256 are kept here.
--
-- INTEGRATION NOTE (accounts merge): account_id and project_id are plain uuid columns with no
-- foreign keys, because the accounts migrations (0100 range) were written in parallel. When they
-- merge, add
--   ALTER TABLE intake_uploads ADD FOREIGN KEY (account_id) REFERENCES accounts (id);
--   ALTER TABLE intake_uploads ADD FOREIGN KEY (project_id, account_id)
--       REFERENCES projects (id, owner_account_id);
-- in a later migration (the existing indexes on both columns already cover the keys). account_id
-- is the account that owns the workspace (the project owner), matching the storage layout.
--
-- Row-level security keys on app.account_id exactly as the accounts design: set with
-- set_config('app.account_id', ..., true) as the first statement of every transaction. The
-- policy fails closed when the setting is missing or empty. The application role has no DELETE.
-- Locking: new objects only.

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'factory_app') THEN
        CREATE ROLE factory_app NOLOGIN NOSUPERUSER NOBYPASSRLS NOCREATEDB NOCREATEROLE;
    END IF;
EXCEPTION WHEN duplicate_object THEN
    NULL;
END
$$;

CREATE TABLE intake_uploads (
    id               uuid PRIMARY KEY,
    account_id       uuid NOT NULL,
    project_id       uuid NOT NULL,
    kind             text NOT NULL CHECK (kind IN ('zip', 'folder')),
    status           text NOT NULL CHECK (status IN ('receiving', 'accepted', 'rejected')),
    -- Display only. Never used as a path.
    original_name    text CHECK (original_name IS NULL OR octet_length(original_name) <= 400),
    original_sha256  text CHECK (original_sha256 IS NULL OR original_sha256 ~ '^[0-9a-f]{64}$'),
    original_bytes   bigint CHECK (original_bytes IS NULL OR original_bytes >= 0),
    -- Logical locator (upload:<project>/<id>), never a host path.
    original_locator text,
    file_count       integer CHECK (file_count IS NULL OR file_count >= 0),
    directory_count  integer CHECK (directory_count IS NULL OR directory_count >= 0),
    extracted_bytes  bigint CHECK (extracted_bytes IS NULL OR extracted_bytes >= 0),
    git_state        text CHECK (git_state IS NULL
                                 OR git_state IN ('not_initialised', 'initialised', 'existing_repository')),
    git_head         text CHECK (git_head IS NULL OR git_head ~ '^[0-9a-f]{40}([0-9a-f]{24})?$'),
    inventory_hash   text CHECK (inventory_hash IS NULL OR inventory_hash ~ '^[0-9a-f]{64}$'),
    findings         jsonb NOT NULL DEFAULT '[]'::jsonb,
    rejection_code   text,
    rejection_message text,
    created_at       timestamptz NOT NULL DEFAULT now(),
    completed_at     timestamptz,
    CONSTRAINT intake_uploads_accepted_complete CHECK (
        status <> 'accepted'
        OR (original_sha256 IS NOT NULL AND original_locator IS NOT NULL
            AND git_state IS NOT NULL AND inventory_hash IS NOT NULL)),
    CONSTRAINT intake_uploads_rejected_has_code CHECK (status <> 'rejected' OR rejection_code IS NOT NULL),
    CONSTRAINT intake_uploads_completed_pair CHECK ((status = 'receiving') = (completed_at IS NULL))
);
CREATE INDEX intake_uploads_account_idx ON intake_uploads (account_id);
CREATE INDEX intake_uploads_project_idx ON intake_uploads (project_id, created_at DESC);
-- Reconciliation of uploads that never completed.
CREATE INDEX intake_uploads_receiving_idx ON intake_uploads (created_at) WHERE status = 'receiving';

ALTER TABLE intake_uploads ENABLE ROW LEVEL SECURITY;
ALTER TABLE intake_uploads FORCE ROW LEVEL SECURITY;
CREATE POLICY intake_uploads_isolation ON intake_uploads
    USING (account_id = NULLIF(current_setting('app.account_id', true), '')::uuid)
    WITH CHECK (account_id = NULLIF(current_setting('app.account_id', true), '')::uuid);

GRANT USAGE ON SCHEMA public TO factory_app;
GRANT SELECT, INSERT, UPDATE ON intake_uploads TO factory_app;
