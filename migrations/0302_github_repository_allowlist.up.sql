-- 0302_github_repository_allowlist (up): the repositories a project may import from.
--
-- S01 finding (EXP-04): a repository-scoped installation token was not a sufficient boundary
-- on its own, so the host enforces its own per-project allowlist before every GitHub call. An
-- empty allowlist allows nothing. Names are stored lowercase because GitHub compares them
-- without regard to case.
--
-- INTEGRATION NOTE (accounts merge): no foreign keys yet, see 0300.
-- Locking: new objects only.

CREATE TABLE github_repository_allowlist (
    account_id           uuid NOT NULL,
    project_id           uuid NOT NULL,
    repository_full_name text NOT NULL CHECK (
        repository_full_name = lower(repository_full_name)
        AND repository_full_name ~ '^[a-z0-9-]{1,39}/[a-z0-9._-]{1,100}$'),
    added_at             timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (project_id, repository_full_name)
);
CREATE INDEX github_repository_allowlist_account_idx ON github_repository_allowlist (account_id);

ALTER TABLE github_repository_allowlist ENABLE ROW LEVEL SECURITY;
ALTER TABLE github_repository_allowlist FORCE ROW LEVEL SECURITY;
CREATE POLICY github_repository_allowlist_isolation ON github_repository_allowlist
    USING (account_id = NULLIF(current_setting('app.account_id', true), '')::uuid)
    WITH CHECK (account_id = NULLIF(current_setting('app.account_id', true), '')::uuid);

GRANT SELECT, INSERT, DELETE ON github_repository_allowlist TO factory_app;
