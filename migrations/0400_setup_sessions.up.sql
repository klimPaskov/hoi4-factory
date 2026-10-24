-- 0400_setup_sessions (up): onboarding sessions, scans, analyses, plans and approvals of the
-- C-09 Setup adapter (crates/setup-adapter).
--
-- The Setup journal itself stays on the workspace volume where Setup wrote it. These tables are
-- the factory's projection: what was inspected, analysed, planned and approved, bound to the
-- hashes and the root identity Setup produced. Nothing here is authority to apply anything; the
-- Setup service re-checks every gate itself.
--
-- Row-level security
--   Same pattern as 0100_accounts: ENABLE and FORCE, policies keyed on app_current_account()
--   (the app.account_id setting every TenantRepo transaction sets first). A row is visible to
--   the project owner and to project members, and fails closed when the setting is missing.
--   owner_account_id repeats projects.owner_account_id (a composite foreign key keeps it
--   consistent) because the workspace path is derived from the owner.
--
-- Locking: all statements create new objects. Later indexes on populated tables belong in their
-- own `-- no-transaction` migration with CREATE INDEX CONCURRENTLY (see 0100_accounts).

-- True for the project owner and for any member of the project, for the current account.
-- SQL language and no SECURITY DEFINER: it runs with the caller's rights, so row-level security
-- of project_members applies to the inner query.
CREATE FUNCTION setup_can_see_project(p_project uuid, p_owner uuid) RETURNS boolean
    LANGUAGE sql STABLE PARALLEL SAFE
    AS $$ SELECT p_owner = app_current_account()
              OR EXISTS (SELECT 1 FROM project_members m
                         WHERE m.project_id = p_project AND m.account_id = app_current_account()) $$;

CREATE TABLE setup_sessions (
    id                         uuid PRIMARY KEY,
    project_id                 uuid NOT NULL,
    owner_account_id           uuid NOT NULL,
    opened_by_account_id       uuid NOT NULL REFERENCES accounts (id),
    -- The Setup revision this session ran against, so a gate can tell what it counted.
    setup_repository           text NOT NULL,
    setup_revision             text NOT NULL CHECK (setup_revision ~ '^[0-9a-f]{40}$'),
    setup_has_pr105_fix        boolean NOT NULL,
    setup_default_plan_builder boolean NOT NULL,
    -- Relative path of the mod root inside source/ (empty means source/ itself).
    mod_root                   text NOT NULL DEFAULT '' CHECK (octet_length(mod_root) <= 1024),
    state                      text NOT NULL DEFAULT 'open' CHECK (state IN ('open', 'closed')),
    opened_at                  timestamptz NOT NULL,
    closed_at                  timestamptz,
    FOREIGN KEY (project_id, owner_account_id) REFERENCES projects (id, owner_account_id) ON DELETE CASCADE,
    CONSTRAINT setup_sessions_id_project_key UNIQUE (id, project_id)
);
CREATE INDEX setup_sessions_project_idx ON setup_sessions (project_id, opened_at DESC);
CREATE INDEX setup_sessions_owner_idx ON setup_sessions (owner_account_id);
CREATE INDEX setup_sessions_opened_by_idx ON setup_sessions (opened_by_account_id);

CREATE TABLE setup_scans (
    -- The scan ID Setup assigned.
    id                         uuid PRIMARY KEY,
    session_id                 uuid NOT NULL,
    project_id                 uuid NOT NULL,
    owner_account_id           uuid NOT NULL,
    root_identity              text NOT NULL,
    mod_root                   text NOT NULL DEFAULT '',
    coverage                   text NOT NULL,
    partial                    boolean NOT NULL,
    cancelled                  boolean NOT NULL,
    files_scanned              bigint NOT NULL CHECK (files_scanned >= 0),
    directories_scanned        bigint NOT NULL CHECK (directories_scanned >= 0),
    bytes_read                 bigint NOT NULL CHECK (bytes_read >= 0),
    finding_count              integer NOT NULL CHECK (finding_count >= 0),
    conflict_count             integer NOT NULL CHECK (conflict_count >= 0),
    semantic_analysis_required boolean NOT NULL,
    -- SHA-256 of the scan result as Setup returned it (host paths included). The stored result
    -- has host paths replaced by logical locators, as shown to the browser.
    scan_sha256                text NOT NULL,
    result                     jsonb NOT NULL,
    -- Workspace inventory hashes around the scan. Equal means the scan changed no byte.
    snapshot_before_sha256     text,
    snapshot_after_sha256      text,
    snapshot_unchanged         boolean,
    evidence_approved_at       timestamptz,
    evidence_count             integer,
    evidence_sha256            text,
    created_at                 timestamptz NOT NULL,
    FOREIGN KEY (session_id, project_id) REFERENCES setup_sessions (id, project_id) ON DELETE CASCADE,
    FOREIGN KEY (project_id, owner_account_id) REFERENCES projects (id, owner_account_id) ON DELETE CASCADE,
    CONSTRAINT setup_scans_id_project_key UNIQUE (id, project_id)
);
CREATE INDEX setup_scans_session_idx ON setup_scans (session_id, project_id, created_at DESC);
CREATE INDEX setup_scans_project_idx ON setup_scans (project_id, created_at DESC);
CREATE INDEX setup_scans_owner_idx ON setup_scans (owner_account_id);

CREATE TABLE setup_analyses (
    -- The analysis ID Setup assigned. A model never chooses it.
    id                         uuid PRIMARY KEY,
    session_id                 uuid NOT NULL,
    scan_id                    uuid,
    project_id                 uuid NOT NULL,
    owner_account_id           uuid NOT NULL,
    mode                       text NOT NULL,
    provider                   text NOT NULL,
    model                      text NOT NULL,
    reasoning_effort           text NOT NULL,
    input_sha256               text NOT NULL,
    output_sha256              text NOT NULL,
    source_revision            text,
    source_manifest_sha256     text,
    status                     text NOT NULL DEFAULT 'recorded' CHECK (status IN ('recorded', 'confirmed')),
    confirmed_fields           jsonb,
    confirmed_values_sha256    text,
    confirmed_at               timestamptz,
    -- The record Setup keeps in plans and locks, and the validated proposals.
    record                     jsonb NOT NULL,
    proposals                  jsonb NOT NULL,
    model_response_id          text,
    usage                      jsonb,
    attempts                   integer NOT NULL DEFAULT 1 CHECK (attempts >= 1),
    created_at                 timestamptz NOT NULL,
    FOREIGN KEY (session_id, project_id) REFERENCES setup_sessions (id, project_id) ON DELETE CASCADE,
    FOREIGN KEY (scan_id, project_id) REFERENCES setup_scans (id, project_id) ON DELETE CASCADE,
    FOREIGN KEY (project_id, owner_account_id) REFERENCES projects (id, owner_account_id) ON DELETE CASCADE,
    CONSTRAINT setup_analyses_id_project_key UNIQUE (id, project_id),
    CONSTRAINT setup_analyses_confirmed_pair CHECK ((status = 'confirmed') = (confirmed_at IS NOT NULL))
);
CREATE INDEX setup_analyses_session_idx ON setup_analyses (session_id, project_id, created_at DESC);
CREATE INDEX setup_analyses_scan_idx ON setup_analyses (scan_id, project_id);
CREATE INDEX setup_analyses_project_idx ON setup_analyses (project_id, created_at DESC);
CREATE INDEX setup_analyses_owner_idx ON setup_analyses (owner_account_id);

CREATE TABLE setup_plans (
    -- The plan ID Setup assigned. It is also the ID of the native transaction that applies it.
    id                         uuid PRIMARY KEY,
    session_id                 uuid NOT NULL,
    analysis_id                uuid,
    scan_id                    uuid,
    project_id                 uuid NOT NULL,
    owner_account_id           uuid NOT NULL,
    -- SHA-256 of the plan a reviewer approves. A conflict decision produces a new one.
    plan_sha256                text NOT NULL,
    root_identity              text NOT NULL,
    plan                       jsonb NOT NULL,
    operation_count            integer NOT NULL CHECK (operation_count >= 0),
    conflict_decisions         jsonb NOT NULL DEFAULT '[]'::jsonb,
    state                      text NOT NULL DEFAULT 'built'
                               CHECK (state IN ('built', 'approved', 'applying', 'applied', 'apply_failed')),
    created_at                 timestamptz NOT NULL,
    updated_at                 timestamptz NOT NULL,
    FOREIGN KEY (session_id, project_id) REFERENCES setup_sessions (id, project_id) ON DELETE CASCADE,
    FOREIGN KEY (analysis_id, project_id) REFERENCES setup_analyses (id, project_id) ON DELETE CASCADE,
    FOREIGN KEY (scan_id, project_id) REFERENCES setup_scans (id, project_id) ON DELETE CASCADE,
    FOREIGN KEY (project_id, owner_account_id) REFERENCES projects (id, owner_account_id) ON DELETE CASCADE,
    CONSTRAINT setup_plans_id_project_key UNIQUE (id, project_id)
);
CREATE INDEX setup_plans_session_idx ON setup_plans (session_id, project_id, created_at DESC);
CREATE INDEX setup_plans_analysis_idx ON setup_plans (analysis_id, project_id);
CREATE INDEX setup_plans_scan_idx ON setup_plans (scan_id, project_id);
CREATE INDEX setup_plans_project_idx ON setup_plans (project_id, created_at DESC);
CREATE INDEX setup_plans_owner_idx ON setup_plans (owner_account_id);
CREATE INDEX setup_plans_hash_idx ON setup_plans (plan_sha256);

CREATE TABLE setup_approvals (
    id                         uuid PRIMARY KEY,
    plan_id                    uuid NOT NULL,
    session_id                 uuid NOT NULL,
    project_id                 uuid NOT NULL,
    owner_account_id           uuid NOT NULL,
    approved_by_account_id     uuid NOT NULL REFERENCES accounts (id),
    -- The digest the reviewer saw, the digest after Setup set its approval flags, and the root.
    reviewed_plan_sha256       text NOT NULL,
    approved_plan_sha256       text NOT NULL,
    root_identity              text NOT NULL,
    effect_scope               text NOT NULL DEFAULT 'apply_setup_plan',
    -- C-08 grant the server checked before calling. Recorded, not enforced here.
    grant_id                   text,
    revocation_epoch           bigint,
    state                      text NOT NULL DEFAULT 'active' CHECK (state IN ('active', 'consumed', 'withdrawn')),
    withdrawn_reason           text,
    created_at                 timestamptz NOT NULL,
    consumed_at                timestamptz,
    withdrawn_at               timestamptz,
    FOREIGN KEY (plan_id, project_id) REFERENCES setup_plans (id, project_id) ON DELETE CASCADE,
    FOREIGN KEY (session_id, project_id) REFERENCES setup_sessions (id, project_id) ON DELETE CASCADE,
    FOREIGN KEY (project_id, owner_account_id) REFERENCES projects (id, owner_account_id) ON DELETE CASCADE
);
CREATE INDEX setup_approvals_plan_idx ON setup_approvals (plan_id, project_id);
CREATE INDEX setup_approvals_session_idx ON setup_approvals (session_id, project_id);
CREATE INDEX setup_approvals_project_idx ON setup_approvals (project_id, created_at DESC);
CREATE INDEX setup_approvals_owner_idx ON setup_approvals (owner_account_id);
CREATE INDEX setup_approvals_approver_idx ON setup_approvals (approved_by_account_id);
-- One live approval per plan.
CREATE UNIQUE INDEX setup_approvals_one_active_key ON setup_approvals (plan_id) WHERE state = 'active';

ALTER TABLE setup_sessions ENABLE ROW LEVEL SECURITY;
ALTER TABLE setup_sessions FORCE ROW LEVEL SECURITY;
ALTER TABLE setup_scans ENABLE ROW LEVEL SECURITY;
ALTER TABLE setup_scans FORCE ROW LEVEL SECURITY;
ALTER TABLE setup_analyses ENABLE ROW LEVEL SECURITY;
ALTER TABLE setup_analyses FORCE ROW LEVEL SECURITY;
ALTER TABLE setup_plans ENABLE ROW LEVEL SECURITY;
ALTER TABLE setup_plans FORCE ROW LEVEL SECURITY;
ALTER TABLE setup_approvals ENABLE ROW LEVEL SECURITY;
ALTER TABLE setup_approvals FORCE ROW LEVEL SECURITY;

CREATE POLICY setup_sessions_select ON setup_sessions FOR SELECT USING (setup_can_see_project(project_id, owner_account_id));
CREATE POLICY setup_sessions_insert ON setup_sessions FOR INSERT WITH CHECK (
    setup_can_see_project(project_id, owner_account_id) AND opened_by_account_id = app_current_account());
CREATE POLICY setup_sessions_update ON setup_sessions FOR UPDATE
    USING (setup_can_see_project(project_id, owner_account_id))
    WITH CHECK (setup_can_see_project(project_id, owner_account_id));

CREATE POLICY setup_scans_select ON setup_scans FOR SELECT USING (setup_can_see_project(project_id, owner_account_id));
CREATE POLICY setup_scans_insert ON setup_scans FOR INSERT WITH CHECK (setup_can_see_project(project_id, owner_account_id));
CREATE POLICY setup_scans_update ON setup_scans FOR UPDATE
    USING (setup_can_see_project(project_id, owner_account_id))
    WITH CHECK (setup_can_see_project(project_id, owner_account_id));

CREATE POLICY setup_analyses_select ON setup_analyses FOR SELECT USING (setup_can_see_project(project_id, owner_account_id));
CREATE POLICY setup_analyses_insert ON setup_analyses FOR INSERT WITH CHECK (setup_can_see_project(project_id, owner_account_id));
CREATE POLICY setup_analyses_update ON setup_analyses FOR UPDATE
    USING (setup_can_see_project(project_id, owner_account_id))
    WITH CHECK (setup_can_see_project(project_id, owner_account_id));

CREATE POLICY setup_plans_select ON setup_plans FOR SELECT USING (setup_can_see_project(project_id, owner_account_id));
CREATE POLICY setup_plans_insert ON setup_plans FOR INSERT WITH CHECK (setup_can_see_project(project_id, owner_account_id));
CREATE POLICY setup_plans_update ON setup_plans FOR UPDATE
    USING (setup_can_see_project(project_id, owner_account_id))
    WITH CHECK (setup_can_see_project(project_id, owner_account_id));

CREATE POLICY setup_approvals_select ON setup_approvals FOR SELECT USING (setup_can_see_project(project_id, owner_account_id));
CREATE POLICY setup_approvals_insert ON setup_approvals FOR INSERT WITH CHECK (
    setup_can_see_project(project_id, owner_account_id) AND approved_by_account_id = app_current_account());
CREATE POLICY setup_approvals_update ON setup_approvals FOR UPDATE
    USING (setup_can_see_project(project_id, owner_account_id))
    WITH CHECK (setup_can_see_project(project_id, owner_account_id));
-- No DELETE policy: the application role cannot remove setup records. Rows leave with their
-- project through ON DELETE CASCADE, run by the retention role.

GRANT SELECT, INSERT ON setup_sessions, setup_scans, setup_analyses, setup_plans, setup_approvals TO factory_app;
GRANT UPDATE (state, closed_at) ON setup_sessions TO factory_app;
GRANT UPDATE (evidence_approved_at, evidence_count, evidence_sha256) ON setup_scans TO factory_app;
GRANT UPDATE (status, confirmed_fields, confirmed_values_sha256, confirmed_at, record) ON setup_analyses TO factory_app;
GRANT UPDATE (plan_sha256, plan, conflict_decisions, state, updated_at) ON setup_plans TO factory_app;
GRANT UPDATE (state, withdrawn_reason, consumed_at, withdrawn_at) ON setup_approvals TO factory_app;
