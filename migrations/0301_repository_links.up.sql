-- 0301_repository_links (up): C-10 RepositoryLink, the connection between a server workspace
-- and a GitHub repository.
--
-- installation_ref NAMES a secret-store entry (which holds the installation id). It never holds a
-- token value: the check constraint refuses strings that look like GitHub credentials.
-- selected_scope records that the installation is limited to selected repositories. Intake
-- refuses all-repositories installations, so 'selected_repositories' is the only value.
-- Link state:
--   pending         recorded before the import starts
--   active          imported, remote operations allowed
--   access_revoked  GitHub answered 401 or 404 for the installation or repository. The server
--                   workspace is NOT touched.
--   disconnected    the user disconnected GitHub
--   failed          the import failed for another reason (state_reason holds the code)
--
-- INTEGRATION NOTE (accounts merge): account_id and project_id carry no foreign keys yet. Add them
-- in a later migration once the accounts tables exist (see 0300). workspace_id is the opaque
-- workspace identity of C-01, the same lowercase id alphabet factory-workspace uses.
-- Locking: new objects only.

CREATE TABLE repository_links (
    id                   uuid PRIMARY KEY,
    account_id           uuid NOT NULL,
    project_id           uuid NOT NULL,
    workspace_id         text NOT NULL CHECK (workspace_id ~ '^[a-z0-9][a-z0-9_-]{0,63}$'),
    provider             text NOT NULL DEFAULT 'github' CHECK (provider = 'github'),
    repository_full_name text NOT NULL CHECK (repository_full_name ~ '^[A-Za-z0-9-]{1,39}/[A-Za-z0-9._-]{1,100}$'),
    -- Known once GitHub has described the repository.
    repository_node_id   text,
    repository_id        bigint,
    installation_ref     text NOT NULL CHECK (
        installation_ref ~ '^[A-Za-z0-9._/:-]{1,200}$'
        AND installation_ref !~ '^(ghs_|ghp_|gho_|ghu_|ghr_|github_pat_)'),
    selected_scope       text NOT NULL DEFAULT 'selected_repositories' CHECK (selected_scope = 'selected_repositories'),
    default_branch       text,
    imported_branch      text,
    imported_commit      text CHECK (imported_commit IS NULL OR imported_commit ~ '^[0-9a-f]{40}([0-9a-f]{24})?$'),
    -- [{"name": "refs/heads/main", "sha": "..."}], as last observed.
    observed_refs        jsonb NOT NULL DEFAULT '[]'::jsonb,
    observed_at          timestamptz,
    state                text NOT NULL CHECK (state IN ('pending', 'active', 'access_revoked', 'disconnected', 'failed')),
    state_reason         text,
    created_at           timestamptz NOT NULL DEFAULT now(),
    updated_at           timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT repository_links_active_complete CHECK (
        state <> 'active'
        OR (repository_node_id IS NOT NULL AND default_branch IS NOT NULL
            AND imported_branch IS NOT NULL AND imported_commit IS NOT NULL AND observed_at IS NOT NULL))
);
CREATE INDEX repository_links_account_idx ON repository_links (account_id);
CREATE INDEX repository_links_project_idx ON repository_links (project_id);
-- Fan-out when one installation is revoked.
CREATE INDEX repository_links_installation_idx ON repository_links (installation_ref);
-- A project has at most one live link.
CREATE UNIQUE INDEX repository_links_project_live_key ON repository_links (project_id)
    WHERE state IN ('pending', 'active');

ALTER TABLE repository_links ENABLE ROW LEVEL SECURITY;
ALTER TABLE repository_links FORCE ROW LEVEL SECURITY;
CREATE POLICY repository_links_isolation ON repository_links
    USING (account_id = NULLIF(current_setting('app.account_id', true), '')::uuid)
    WITH CHECK (account_id = NULLIF(current_setting('app.account_id', true), '')::uuid);

GRANT SELECT, INSERT, UPDATE ON repository_links TO factory_app;
