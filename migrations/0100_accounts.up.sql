-- 0100_accounts (up): accounts, sessions, email verification, projects and project members.
--
-- All timestamps are timestamptz (stored as UTC instants). Accounts and projects are soft
-- deleted through deleted_at; delete_after is when the retention job may remove them for good.
-- Locking: every statement here creates new objects, so nothing blocks existing traffic. When a
-- later migration adds an index to a table that already holds rows, use CREATE INDEX
-- CONCURRENTLY, which cannot run inside the transaction sqlx wraps around a migration. Put
-- that statement in its own migration file whose first line is `-- no-transaction`.
--
-- Roles
--   The application connects as a login role that is a MEMBER of the NOLOGIN group role
--   `factory_app`. It must not be a superuser, must not have BYPASSRLS and must not own the
--   tables below. The migration runner connects as a different role (the table owner).
--   Operators create the login role once, for example:
--     CREATE ROLE factory_login LOGIN PASSWORD '...' NOSUPERUSER NOBYPASSRLS;
--     GRANT factory_app TO factory_login;
--   The group role is created here when the migration role may create roles. Pre-create it
--   when it may not; the block below then does nothing.
--
-- Row-level security
--   projects and project_members have RLS enabled AND forced, so even the table owner is
--   subject to the policies (a superuser or BYPASSRLS role still bypasses them, which is why
--   `Database::ensure_rls_enforced` refuses such a role in production). The policies key on
--   app.account_id, which the application sets with set_config(..., true) (SET LOCAL
--   semantics) as the first statement of every tenant-scoped transaction. When the setting is
--   missing or empty, app_current_account() is NULL and every policy fails closed.

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'factory_app') THEN
        CREATE ROLE factory_app NOLOGIN NOSUPERUSER NOBYPASSRLS NOCREATEDB NOCREATEROLE;
    END IF;
EXCEPTION WHEN duplicate_object THEN
    NULL;
END
$$;

CREATE TABLE accounts (
    id                uuid PRIMARY KEY,
    email             text NOT NULL,
    email_verified_at timestamptz,
    password_hash     text,
    github_user_id    bigint UNIQUE,
    display_name      text NOT NULL,
    created_at        timestamptz NOT NULL DEFAULT now(),
    disabled_at       timestamptz,
    deleted_at        timestamptz,
    delete_after      timestamptz,
    CONSTRAINT accounts_has_credential CHECK (password_hash IS NOT NULL OR github_user_id IS NOT NULL),
    -- The address is kept exactly as typed; uniqueness below ignores case.
    CONSTRAINT accounts_email_shape CHECK (position('@' in email) > 1 AND octet_length(email) <= 254),
    -- Bytes, not characters: the application also limits grapheme clusters.
    CONSTRAINT accounts_display_name_len CHECK (octet_length(display_name) BETWEEN 1 AND 320)
);
CREATE UNIQUE INDEX accounts_email_ci_key ON accounts (lower(email));
CREATE INDEX accounts_delete_after_idx ON accounts (delete_after) WHERE delete_after IS NOT NULL;

CREATE TABLE sessions (
    id           uuid PRIMARY KEY,
    account_id   uuid NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    token_hash   bytea NOT NULL UNIQUE,
    created_at   timestamptz NOT NULL,
    expires_at   timestamptz NOT NULL,
    last_seen_at timestamptz NOT NULL,
    user_agent   text
);
CREATE INDEX sessions_account_idx ON sessions (account_id);
CREATE INDEX sessions_expires_idx ON sessions (expires_at);

CREATE TABLE email_verification_tokens (
    id          uuid PRIMARY KEY,
    account_id  uuid NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    email       text NOT NULL,
    token_hash  bytea NOT NULL UNIQUE,
    created_at  timestamptz NOT NULL,
    expires_at  timestamptz NOT NULL,
    consumed_at timestamptz
);
CREATE INDEX email_verification_tokens_account_idx ON email_verification_tokens (account_id);

CREATE TABLE projects (
    id                     uuid PRIMARY KEY,
    owner_account_id       uuid NOT NULL REFERENCES accounts (id),
    name                   text NOT NULL CHECK (octet_length(name) BETWEEN 1 AND 400),
    visibility             text NOT NULL DEFAULT 'private' CHECK (visibility IN ('private', 'public')),
    created_at             timestamptz NOT NULL DEFAULT now(),
    deleted_at  timestamptz,
    delete_after           timestamptz,
    CONSTRAINT projects_soft_delete_pair CHECK ((deleted_at IS NULL) = (delete_after IS NULL)),
    CONSTRAINT projects_id_owner_key UNIQUE (id, owner_account_id)
);
CREATE INDEX projects_owner_idx ON projects (owner_account_id);
CREATE INDEX projects_delete_after_idx ON projects (delete_after) WHERE delete_after IS NOT NULL;

-- project_owner_id repeats projects.owner_account_id so that the policies on project_members
-- never need to read projects (which would make the two policy sets recurse into each other).
-- The composite foreign key keeps the copy consistent.
CREATE TABLE project_members (
    project_id       uuid NOT NULL,
    account_id       uuid NOT NULL REFERENCES accounts (id),
    project_owner_id uuid NOT NULL,
    role             text NOT NULL CHECK (role IN ('owner', 'maintainer', 'contributor', 'viewer')),
    created_at       timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (project_id, account_id),
    FOREIGN KEY (project_id, project_owner_id) REFERENCES projects (id, owner_account_id) ON DELETE CASCADE,
    CONSTRAINT project_members_owner_role CHECK ((role = 'owner') = (account_id = project_owner_id))
);
CREATE INDEX project_members_account_idx ON project_members (account_id);
-- Foreign key (project_id, project_owner_id) and the policy column project_owner_id.
CREATE INDEX project_members_project_owner_idx ON project_members (project_id, project_owner_id);
CREATE INDEX project_members_owner_idx ON project_members (project_owner_id);

CREATE FUNCTION app_current_account() RETURNS uuid
    LANGUAGE sql STABLE PARALLEL SAFE
    AS $$ SELECT NULLIF(current_setting('app.account_id', true), '')::uuid $$;

ALTER TABLE projects ENABLE ROW LEVEL SECURITY;
ALTER TABLE projects FORCE ROW LEVEL SECURITY;
ALTER TABLE project_members ENABLE ROW LEVEL SECURITY;
ALTER TABLE project_members FORCE ROW LEVEL SECURITY;

-- projects: members see their projects, everyone sees public ones read-only.
CREATE POLICY projects_select ON projects FOR SELECT USING (
    owner_account_id = app_current_account()
    OR visibility = 'public'
    OR EXISTS (SELECT 1 FROM project_members m
               WHERE m.project_id = projects.id AND m.account_id = app_current_account())
);
CREATE POLICY projects_insert ON projects FOR INSERT WITH CHECK (
    owner_account_id = app_current_account()
);
CREATE POLICY projects_update ON projects FOR UPDATE USING (
    owner_account_id = app_current_account()
    OR EXISTS (SELECT 1 FROM project_members m
               WHERE m.project_id = projects.id AND m.account_id = app_current_account()
                 AND m.role IN ('owner', 'maintainer'))
) WITH CHECK (
    owner_account_id = app_current_account()
    OR EXISTS (SELECT 1 FROM project_members m
               WHERE m.project_id = projects.id AND m.account_id = app_current_account()
                 AND m.role IN ('owner', 'maintainer'))
);
-- No DELETE policy: the application role cannot hard-delete projects. The retention job that
-- purges projects after their grace period runs under a separate operator role.

-- project_members: a member sees their own row, the project owner sees every row.
CREATE POLICY project_members_select ON project_members FOR SELECT USING (
    account_id = app_current_account() OR project_owner_id = app_current_account()
);
CREATE POLICY project_members_insert ON project_members FOR INSERT WITH CHECK (
    project_owner_id = app_current_account()
);
CREATE POLICY project_members_update ON project_members FOR UPDATE USING (
    project_owner_id = app_current_account()
) WITH CHECK (
    project_owner_id = app_current_account()
);
CREATE POLICY project_members_delete ON project_members FOR DELETE USING (
    project_owner_id = app_current_account()
);

GRANT USAGE ON SCHEMA public TO factory_app;
GRANT SELECT, INSERT, UPDATE ON accounts TO factory_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON sessions TO factory_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON email_verification_tokens TO factory_app;
GRANT SELECT, INSERT ON projects TO factory_app;
GRANT UPDATE (name, visibility, deleted_at, delete_after) ON projects TO factory_app;
GRANT SELECT, INSERT, DELETE ON project_members TO factory_app;
GRANT UPDATE (role) ON project_members TO factory_app;
