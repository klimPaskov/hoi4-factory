-- 0700_agent_grants (up): C-08 approval grants (crates/broker).
--
-- A grant is the host's record of what an agent run may do: operation classes, path roots inside
-- the task worktree, provider destinations, limits, expiry and a revocation epoch. The broker
-- checks it at dispatch and again inside the transaction that records an effect.
--
-- Row-level security
--   Same pattern as 0400_setup_sessions: ENABLE and FORCE, policies keyed on app_current_account().
--   Project owner and members see the project's grants. Only the grantor may insert (and the
--   grantor's claimed role must match the grantor's membership row, so a role cannot be
--   overstated). The one permitted update is a revocation: the policy requires revoked_at and
--   revoked_by = the caller, and the column grant covers only those columns plus the epoch.
--   No DELETE policy: grants are an audit trail and leave only with their project.
--
-- Locking: only new objects.

CREATE FUNCTION broker_can_see_project(p_project uuid, p_owner uuid) RETURNS boolean
    LANGUAGE sql STABLE PARALLEL SAFE
    AS $$ SELECT p_owner = app_current_account()
              OR EXISTS (SELECT 1 FROM project_members m
                         WHERE m.project_id = p_project AND m.account_id = app_current_account()) $$;

CREATE TABLE agent_grants (
    id                   uuid PRIMARY KEY,
    project_id           uuid NOT NULL,
    owner_account_id     uuid NOT NULL,
    grantor_account_id   uuid NOT NULL REFERENCES accounts (id),
    grantor_role         text NOT NULL CHECK (grantor_role IN ('owner', 'maintainer', 'contributor', 'viewer')),
    -- Run scope: the task this grant belongs to (owned by the tasks crate, so no foreign key).
    task_id              uuid NOT NULL,
    operation_classes    text[] NOT NULL CHECK (cardinality(operation_classes) BETWEEN 1 AND 16),
    -- Normalized relative path prefixes inside the task worktree. Empty means no path.
    allowed_roots        text[] NOT NULL CHECK (cardinality(allowed_roots) <= 64),
    -- Strings like 'anthropic:https://api.anthropic.com'.
    provider_destinations text[] NOT NULL CHECK (cardinality(provider_destinations) <= 16),
    max_cost_micros      bigint NOT NULL CHECK (max_cost_micros >= 0),
    max_turns            integer NOT NULL CHECK (max_turns >= 0),
    max_changed_files    integer NOT NULL CHECK (max_changed_files >= 0),
    expires_at           timestamptz NOT NULL,
    policy_version       text NOT NULL CHECK (char_length(policy_version) BETWEEN 1 AND 64),
    -- A grant authorizes either a plan class (NULL) or exactly one proposal hash.
    proposal_sha256      text CHECK (proposal_sha256 ~ '^[0-9a-f]{64}$'),
    revocation_epoch     bigint NOT NULL DEFAULT 1 CHECK (revocation_epoch >= 1),
    revoked_at           timestamptz,
    revoked_by           uuid REFERENCES accounts (id),
    created_at           timestamptz NOT NULL,
    FOREIGN KEY (project_id, owner_account_id) REFERENCES projects (id, owner_account_id) ON DELETE CASCADE,
    CONSTRAINT agent_grants_revoked_pair CHECK ((revoked_at IS NULL) = (revoked_by IS NULL))
);
CREATE INDEX agent_grants_project_idx ON agent_grants (project_id, created_at DESC);
CREATE INDEX agent_grants_owner_idx ON agent_grants (owner_account_id);
CREATE INDEX agent_grants_task_idx ON agent_grants (task_id);
CREATE INDEX agent_grants_grantor_idx ON agent_grants (grantor_account_id);
CREATE INDEX agent_grants_revoked_by_idx ON agent_grants (revoked_by);

ALTER TABLE agent_grants ENABLE ROW LEVEL SECURITY;
ALTER TABLE agent_grants FORCE ROW LEVEL SECURITY;

CREATE POLICY agent_grants_select ON agent_grants FOR SELECT
    USING (broker_can_see_project(project_id, owner_account_id));
CREATE POLICY agent_grants_insert ON agent_grants FOR INSERT WITH CHECK (
    grantor_account_id = app_current_account()
    AND (CASE WHEN owner_account_id = app_current_account() THEN grantor_role = 'owner'
              ELSE EXISTS (SELECT 1 FROM project_members m
                           WHERE m.project_id = agent_grants.project_id
                             AND m.account_id = app_current_account()
                             AND m.role = agent_grants.grantor_role) END)
    AND revoked_at IS NULL AND revocation_epoch = 1);
-- USING lets any project member lock the row (SELECT ... FOR SHARE needs the update policy too).
-- WITH CHECK allows only a revocation made by the caller.
CREATE POLICY agent_grants_update ON agent_grants FOR UPDATE
    USING (broker_can_see_project(project_id, owner_account_id))
    WITH CHECK (broker_can_see_project(project_id, owner_account_id)
                AND revoked_at IS NOT NULL AND revoked_by = app_current_account());

GRANT SELECT, INSERT ON agent_grants TO factory_app;
GRANT UPDATE (revocation_epoch, revoked_at, revoked_by) ON agent_grants TO factory_app;
