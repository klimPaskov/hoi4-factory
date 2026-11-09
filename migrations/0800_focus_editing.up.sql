-- 0800_focus_editing (up): S04 focus projections, C-02 change proposals and publications.
--
-- Every row belongs to one task (an editor session and an agent task are both tasks), so the
-- tenant columns, composite foreign keys and row-level security follow 0600_tasks: ENABLE and
-- FORCE, policies through tasks_can_see_project(), no DELETE grant or policy. Rows leave only by
-- the project purge cascade.
--
-- focus_projections: what hoi4.focus_inspect returned for one tree in the task's worktree, with
--   the tool version, the mod file hashes it read and a hash of the projection. Append-only.
-- change_proposals: one C-02 proposal per applied focus operation batch (a person's gesture, a
--   native agent call or a coding-client call), with the operations, the tool request hash, the
--   expected and resulting file hashes and the engine check. Only its state and check columns
--   change.
-- publications: commit, push, pull request and restore effects. The intent row is written before
--   the effect, and a lost reply is reconciled by reading state (C-10, GIT-09).

CREATE TABLE focus_projections (
    id               uuid PRIMARY KEY,
    task_id          uuid NOT NULL,
    project_id       uuid NOT NULL,
    owner_account_id uuid NOT NULL,
    tree_id          text NOT NULL CHECK (char_length(tree_id) BETWEEN 1 AND 256),
    relative_path    text NOT NULL CHECK (char_length(relative_path) BETWEEN 1 AND 1024),
    tool_name        text NOT NULL CHECK (char_length(tool_name) <= 200),
    tool_version     text NOT NULL CHECK (char_length(tool_version) <= 200),
    -- Mod file path -> SHA-256 for every mod file the inspection read.
    source_hashes    jsonb NOT NULL,
    projection_hash  text NOT NULL CHECK (projection_hash ~ '^[0-9a-f]{64}$'),
    projection       jsonb NOT NULL,
    -- The tool's own plan of the tree, which focus operations change before a rewrite.
    plan             jsonb NOT NULL,
    created_at       timestamptz NOT NULL DEFAULT clock_timestamp(),
    FOREIGN KEY (task_id, project_id, owner_account_id) REFERENCES agent_tasks (id, project_id, owner_account_id) ON DELETE CASCADE
);
CREATE INDEX focus_projections_latest_idx ON focus_projections (task_id, tree_id, created_at DESC);
CREATE INDEX focus_projections_project_idx ON focus_projections (project_id, owner_account_id);
CREATE TRIGGER focus_projections_append_only BEFORE UPDATE OR DELETE ON focus_projections
    FOR EACH ROW EXECUTE FUNCTION tasks_forbid_change();

CREATE TABLE change_proposals (
    id                   uuid PRIMARY KEY,
    task_id              uuid NOT NULL,
    project_id           uuid NOT NULL,
    owner_account_id     uuid NOT NULL,
    actor_kind           text NOT NULL CHECK (actor_kind IN ('person', 'native_agent', 'coding_client')),
    actor_account_id     uuid REFERENCES accounts (id),
    attempt_id           uuid,
    tree_id              text NOT NULL CHECK (char_length(tree_id) BETWEEN 1 AND 256),
    tool_operation       text NOT NULL CHECK (char_length(tool_operation) <= 200),
    tool_version         text NOT NULL CHECK (char_length(tool_version) <= 200),
    base_projection_hash text NOT NULL CHECK (base_projection_hash ~ '^[0-9a-f]{64}$'),
    operations           jsonb NOT NULL,
    request_sha256       text NOT NULL CHECK (request_sha256 ~ '^[0-9a-f]{64}$'),
    -- Path -> SHA-256 before and after, "absent" for a missing file.
    before_hashes        jsonb NOT NULL,
    after_hashes         jsonb NOT NULL,
    changed_paths        text[] NOT NULL DEFAULT '{}',
    changed_ids          text[] NOT NULL DEFAULT '{}',
    -- Focuses added and fields changed with before and after values, for the review card.
    semantic_summary     jsonb NOT NULL DEFAULT '{}'::jsonb,
    -- [{path, diff}] unified diffs of the changed paths.
    diffs                jsonb NOT NULL DEFAULT '[]'::jsonb,
    -- Path -> base64 of the bytes before the change (null for a file that did not exist), so an
    -- uncommitted proposal can be discarded exactly.
    before_contents      jsonb NOT NULL DEFAULT '{}'::jsonb,
    -- SHA-256 of the canonical proposal (operations, base, hashes, paths). Approval binds to it.
    proposal_hash        text NOT NULL CHECK (proposal_hash ~ '^[0-9a-f]{64}$'),
    state                text NOT NULL CHECK (state IN ('applied', 'committed', 'discarded', 'reverted')),
    after_projection_id  uuid REFERENCES focus_projections (id),
    engine_report        jsonb,
    finding_delta        jsonb,
    created_at           timestamptz NOT NULL DEFAULT clock_timestamp(),
    updated_at           timestamptz NOT NULL DEFAULT clock_timestamp(),
    FOREIGN KEY (task_id, project_id, owner_account_id) REFERENCES agent_tasks (id, project_id, owner_account_id) ON DELETE CASCADE,
    CONSTRAINT change_proposals_actor_key CHECK ((actor_kind = 'person') = (actor_account_id IS NOT NULL))
);
CREATE INDEX change_proposals_task_idx ON change_proposals (task_id, created_at);
CREATE INDEX change_proposals_project_idx ON change_proposals (project_id, owner_account_id);

CREATE TABLE publications (
    id               uuid PRIMARY KEY,
    task_id          uuid NOT NULL,
    project_id       uuid NOT NULL,
    owner_account_id uuid NOT NULL,
    kind             text NOT NULL CHECK (kind IN ('commit', 'push', 'pull_request', 'restore')),
    idempotency_key  text NOT NULL CHECK (char_length(idempotency_key) BETWEEN 1 AND 200),
    requested_by     uuid NOT NULL REFERENCES accounts (id),
    grant_id         uuid,
    intent           jsonb NOT NULL,
    state            text NOT NULL DEFAULT 'intent' CHECK (state IN ('intent', 'succeeded', 'failed', 'unknown')),
    result           jsonb,
    created_at       timestamptz NOT NULL DEFAULT clock_timestamp(),
    settled_at       timestamptz,
    FOREIGN KEY (task_id, project_id, owner_account_id) REFERENCES agent_tasks (id, project_id, owner_account_id) ON DELETE CASCADE,
    CONSTRAINT publications_idempotency UNIQUE (task_id, kind, idempotency_key)
);
CREATE INDEX publications_project_idx ON publications (project_id, owner_account_id);

-- engine_baselines: the findings baseline (hoi4-engine-baseline/1) of a project at one commit,
-- with the check report it came from. A commit check compares the worktree against it, so only
-- new findings block (S04 gate 8). Imports record one for the imported commit.
CREATE TABLE engine_baselines (
    id               uuid PRIMARY KEY,
    project_id       uuid NOT NULL,
    owner_account_id uuid NOT NULL,
    commit_oid       text NOT NULL CHECK (commit_oid ~ '^[0-9a-f]{40}([0-9a-f]{24})?$'),
    engine_version   text NOT NULL CHECK (char_length(engine_version) <= 64),
    engine_rev       text NOT NULL CHECK (char_length(engine_rev) <= 64),
    baseline         jsonb NOT NULL,
    report_summary   jsonb NOT NULL,
    created_at       timestamptz NOT NULL DEFAULT clock_timestamp(),
    FOREIGN KEY (project_id, owner_account_id) REFERENCES projects (id, owner_account_id) ON DELETE CASCADE,
    CONSTRAINT engine_baselines_commit UNIQUE (project_id, commit_oid, engine_rev)
);

ALTER TABLE engine_baselines ENABLE ROW LEVEL SECURITY;
ALTER TABLE engine_baselines FORCE ROW LEVEL SECURITY;
CREATE POLICY engine_baselines_select ON engine_baselines FOR SELECT USING (tasks_can_see_project(project_id, owner_account_id));
CREATE POLICY engine_baselines_insert ON engine_baselines FOR INSERT WITH CHECK (tasks_can_see_project(project_id, owner_account_id));
GRANT SELECT, INSERT ON engine_baselines TO factory_app;

ALTER TABLE focus_projections ENABLE ROW LEVEL SECURITY;
ALTER TABLE focus_projections FORCE ROW LEVEL SECURITY;
ALTER TABLE change_proposals ENABLE ROW LEVEL SECURITY;
ALTER TABLE change_proposals FORCE ROW LEVEL SECURITY;
ALTER TABLE publications ENABLE ROW LEVEL SECURITY;
ALTER TABLE publications FORCE ROW LEVEL SECURITY;

CREATE POLICY focus_projections_select ON focus_projections FOR SELECT USING (tasks_can_see_project(project_id, owner_account_id));
CREATE POLICY focus_projections_insert ON focus_projections FOR INSERT WITH CHECK (tasks_can_see_project(project_id, owner_account_id));
CREATE POLICY change_proposals_select ON change_proposals FOR SELECT USING (tasks_can_see_project(project_id, owner_account_id));
CREATE POLICY change_proposals_insert ON change_proposals FOR INSERT WITH CHECK (tasks_can_see_project(project_id, owner_account_id));
CREATE POLICY change_proposals_update ON change_proposals FOR UPDATE
    USING (tasks_can_see_project(project_id, owner_account_id))
    WITH CHECK (tasks_can_see_project(project_id, owner_account_id));
CREATE POLICY publications_select ON publications FOR SELECT USING (tasks_can_see_project(project_id, owner_account_id));
CREATE POLICY publications_insert ON publications FOR INSERT WITH CHECK (tasks_can_see_project(project_id, owner_account_id));
CREATE POLICY publications_update ON publications FOR UPDATE
    USING (tasks_can_see_project(project_id, owner_account_id))
    WITH CHECK (tasks_can_see_project(project_id, owner_account_id));

GRANT SELECT, INSERT ON focus_projections, change_proposals, publications TO factory_app;
GRANT UPDATE (state, engine_report, finding_delta, updated_at) ON change_proposals TO factory_app;
GRANT UPDATE (state, result, settled_at) ON publications TO factory_app;
