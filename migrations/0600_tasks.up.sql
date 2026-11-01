-- 0600_tasks (up): the task ledger of crates/tasks (S03-W2, S03-W4).
--
-- Tasks, attempts, C-03 run events, C-06 context bundles, C-04 tool intents and receipts, host
-- incarnations and workspace leases. PostgreSQL is the single authority for these (SPEC-003): the
-- agent loop's conversation lives in agent_attempts.conversation so one transaction can bind it
-- to the journal.
--
-- Row-level security
--   Same pattern as 0400_setup_sessions: ENABLE and FORCE, policies keyed on app_current_account()
--   through tasks_can_see_project(), failing closed when the setting is missing. Every tenant
--   table carries project_id and owner_account_id (composite foreign key to projects, and to the
--   parent task where there is one) so a child row can never name another tenant than its parent.
--   There is no DELETE policy and no DELETE grant on any table: rows leave only by the project
--   purge cascade, which runs under the operator role.
--
-- Append-only tables
--   agent_events, context_bundles and tool_intents have no UPDATE grant and a trigger that raises
--   on UPDATE and DELETE (defense in depth against a future grant or a superuser mistake). The
--   trigger lets a DELETE through only when it runs inside another trigger (the foreign key
--   cascade of a project purge), so retention still works.
--
-- Locking: all statements create new objects. Later indexes on populated tables belong in their
-- own `-- no-transaction` migration with CREATE INDEX CONCURRENTLY (see 0100_accounts).

CREATE FUNCTION tasks_can_see_project(p_project uuid, p_owner uuid) RETURNS boolean
    LANGUAGE sql STABLE PARALLEL SAFE
    AS $$ SELECT p_owner = app_current_account()
              OR EXISTS (SELECT 1 FROM project_members m
                         WHERE m.project_id = p_project AND m.account_id = app_current_account()) $$;

CREATE FUNCTION tasks_forbid_change() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    -- A cascading delete from projects or agent_tasks fires this trigger one level deep inside
    -- the referential-integrity trigger. A direct DELETE statement fires it at depth 1.
    IF TG_OP = 'DELETE' AND pg_trigger_depth() > 1 THEN
        RETURN OLD;
    END IF;
    RAISE EXCEPTION '% is append-only: % refused', TG_TABLE_NAME, TG_OP;
END
$$;

-- ---------------------------------------------------------------------------------------------
-- Host incarnations: one row per server process start. Not tenant data (no RLS, no content).
-- ---------------------------------------------------------------------------------------------
CREATE TABLE host_incarnations (
    id           uuid PRIMARY KEY,
    -- A short label such as a build or process name. Never a host name or path.
    label        text NOT NULL CHECK (label ~ '^[A-Za-z0-9._:-]{1,64}$'),
    started_at   timestamptz NOT NULL DEFAULT now(),
    last_seen_at timestamptz NOT NULL DEFAULT now()
);

-- ---------------------------------------------------------------------------------------------
-- Tasks
-- ---------------------------------------------------------------------------------------------
CREATE TABLE agent_tasks (
    id                    uuid PRIMARY KEY,
    project_id            uuid NOT NULL,
    owner_account_id      uuid NOT NULL,
    created_by_account_id uuid NOT NULL REFERENCES accounts (id),
    objective             text NOT NULL CHECK (char_length(objective) BETWEEN 1 AND 4000),
    requirement_ids       text[] NOT NULL DEFAULT '{}' CHECK (cardinality(requirement_ids) <= 64),
    mode                  text NOT NULL CHECK (mode IN ('read_only', 'edit')),
    -- Relative path prefixes the task may write. Empty for read_only, at least one for edit.
    allowed_paths         text[] NOT NULL DEFAULT '{}' CHECK (cardinality(allowed_paths) <= 64),
    state                 text NOT NULL DEFAULT 'planned' CHECK (state IN (
                              'planned', 'ready', 'running', 'waiting_for_input', 'in_review',
                              'completed', 'blocked', 'cancelled')),
    -- Pause is a scheduler condition, not a state: it survives restart and is not cancellation.
    paused                boolean NOT NULL DEFAULT false,
    pause_reason          text CHECK (char_length(pause_reason) <= 1000),
    budget_micros         bigint NOT NULL CHECK (budget_micros >= 0),
    max_turns             integer NOT NULL CHECK (max_turns BETWEEN 1 AND 1000),
    -- Opaque to the ledger: the model profile the user picked.
    model_choice          jsonb NOT NULL DEFAULT '{}'::jsonb,
    -- Expected-version counter for state changes, and the C-03 event counter.
    version               bigint NOT NULL DEFAULT 1 CHECK (version >= 1),
    last_seq              bigint NOT NULL DEFAULT 0 CHECK (last_seq >= 0),
    created_at            timestamptz NOT NULL DEFAULT now(),
    updated_at            timestamptz NOT NULL DEFAULT now(),
    FOREIGN KEY (project_id, owner_account_id) REFERENCES projects (id, owner_account_id) ON DELETE CASCADE,
    CONSTRAINT agent_tasks_scope_key UNIQUE (id, project_id, owner_account_id),
    CONSTRAINT agent_tasks_mode_paths CHECK (mode = 'edit' OR cardinality(allowed_paths) = 0),
    CONSTRAINT agent_tasks_edit_paths CHECK (mode = 'read_only' OR cardinality(allowed_paths) >= 1)
);
CREATE INDEX agent_tasks_project_idx ON agent_tasks (project_id, created_at DESC);
CREATE INDEX agent_tasks_owner_idx ON agent_tasks (owner_account_id);
CREATE INDEX agent_tasks_creator_idx ON agent_tasks (created_by_account_id);

-- ---------------------------------------------------------------------------------------------
-- C-06 context bundles (immutable once stored)
-- ---------------------------------------------------------------------------------------------
CREATE TABLE context_bundles (
    id               uuid PRIMARY KEY,
    task_id          uuid NOT NULL,
    project_id       uuid NOT NULL,
    owner_account_id uuid NOT NULL,
    manifest_sha256  text NOT NULL CHECK (manifest_sha256 ~ '^[0-9a-f]{64}$'),
    bundle           jsonb NOT NULL,
    created_at       timestamptz NOT NULL DEFAULT now(),
    FOREIGN KEY (task_id, project_id, owner_account_id) REFERENCES agent_tasks (id, project_id, owner_account_id) ON DELETE CASCADE,
    FOREIGN KEY (project_id, owner_account_id) REFERENCES projects (id, owner_account_id) ON DELETE CASCADE,
    CONSTRAINT context_bundles_task_key UNIQUE (id, task_id)
);
CREATE INDEX context_bundles_task_idx ON context_bundles (task_id, created_at DESC);
CREATE INDEX context_bundles_project_idx ON context_bundles (project_id, owner_account_id);

-- ---------------------------------------------------------------------------------------------
-- Attempts
-- ---------------------------------------------------------------------------------------------
CREATE TABLE agent_attempts (
    id                 uuid PRIMARY KEY,
    task_id            uuid NOT NULL,
    project_id         uuid NOT NULL,
    owner_account_id   uuid NOT NULL,
    attempt_no         integer NOT NULL CHECK (attempt_no >= 1),
    state              text NOT NULL CHECK (state IN (
                           'admitted', 'running', 'interrupt_requested', 'reconciling',
                           'succeeded', 'failed', 'cancelled', 'unknown_effect')),
    bundle_id          uuid NOT NULL,
    grant_id           uuid NOT NULL,
    grant_epoch        bigint NOT NULL CHECK (grant_epoch >= 0),
    host_incarnation   uuid NOT NULL REFERENCES host_incarnations (id),
    fencing_generation bigint NOT NULL CHECK (fencing_generation >= 0),
    model_profile      jsonb NOT NULL,
    funding            text NOT NULL CHECK (funding IN ('operator_allowance', 'user_key')),
    -- A key reference, never the key. Required exactly when the user's own key pays.
    user_key_id        uuid,
    -- The agent loop's persisted conversation: subordinate state in the same database as the
    -- journal. It cannot approve or repeat an effect.
    conversation       jsonb NOT NULL DEFAULT '{}'::jsonb,
    outcome            jsonb,
    error_code         text CHECK (char_length(error_code) <= 200),
    version            bigint NOT NULL DEFAULT 1 CHECK (version >= 1),
    admitted_at        timestamptz NOT NULL DEFAULT now(),
    started_at         timestamptz,
    finished_at        timestamptz,
    FOREIGN KEY (task_id, project_id, owner_account_id) REFERENCES agent_tasks (id, project_id, owner_account_id) ON DELETE CASCADE,
    FOREIGN KEY (project_id, owner_account_id) REFERENCES projects (id, owner_account_id) ON DELETE CASCADE,
    FOREIGN KEY (bundle_id, task_id) REFERENCES context_bundles (id, task_id),
    CONSTRAINT agent_attempts_no_key UNIQUE (task_id, attempt_no),
    CONSTRAINT agent_attempts_task_key UNIQUE (id, task_id),
    CONSTRAINT agent_attempts_funding_key CHECK ((funding = 'user_key') = (user_key_id IS NOT NULL))
);
-- Single writer: at most one nonterminal attempt per task.
CREATE UNIQUE INDEX agent_attempts_one_active ON agent_attempts (task_id)
    WHERE state NOT IN ('succeeded', 'failed', 'cancelled');
CREATE INDEX agent_attempts_project_idx ON agent_attempts (project_id, owner_account_id);
CREATE INDEX agent_attempts_bundle_idx ON agent_attempts (bundle_id);
CREATE INDEX agent_attempts_incarnation_idx ON agent_attempts (host_incarnation);

CREATE FUNCTION agent_attempts_guard() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    IF OLD.state IN ('succeeded', 'failed', 'cancelled') THEN
        RAISE EXCEPTION 'agent_attempts: a terminal attempt (%) never changes', OLD.state;
    END IF;
    RETURN NEW;
END
$$;
CREATE TRIGGER agent_attempts_terminal_guard BEFORE UPDATE ON agent_attempts
    FOR EACH ROW EXECUTE FUNCTION agent_attempts_guard();

-- ---------------------------------------------------------------------------------------------
-- C-03 run events (append-only, ordered per task by a gapless sequence)
-- ---------------------------------------------------------------------------------------------
CREATE TABLE agent_events (
    task_id            uuid NOT NULL,
    seq                bigint NOT NULL CHECK (seq >= 1),
    project_id         uuid NOT NULL,
    owner_account_id   uuid NOT NULL,
    event_id           uuid NOT NULL UNIQUE,
    attempt_id         uuid,
    schema_version     integer NOT NULL CHECK (schema_version >= 1),
    actor              text NOT NULL CHECK (actor ~ '^(host|user:[0-9a-f-]{36}|worker:[0-9]{1,18})$'),
    event_type         text NOT NULL CHECK (event_type ~ '^[a-z][a-z0-9_.]{0,63}$'),
    payload            jsonb NOT NULL,
    payload_sha256     text NOT NULL CHECK (payload_sha256 ~ '^[0-9a-f]{64}$'),
    workspace_snapshot text,
    policy_version     text NOT NULL,
    evidence_refs      jsonb NOT NULL DEFAULT '[]'::jsonb,
    created_at         timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (task_id, seq),
    FOREIGN KEY (task_id, project_id, owner_account_id) REFERENCES agent_tasks (id, project_id, owner_account_id) ON DELETE CASCADE,
    FOREIGN KEY (project_id, owner_account_id) REFERENCES projects (id, owner_account_id) ON DELETE CASCADE,
    FOREIGN KEY (attempt_id, task_id) REFERENCES agent_attempts (id, task_id) ON DELETE CASCADE
);
CREATE INDEX agent_events_project_idx ON agent_events (project_id, owner_account_id);
CREATE INDEX agent_events_attempt_idx ON agent_events (attempt_id, task_id);

-- ---------------------------------------------------------------------------------------------
-- C-04 tool intents (written before the effect, immutable) and receipts (the observed outcome)
-- ---------------------------------------------------------------------------------------------
CREATE TABLE tool_intents (
    operation_id        uuid PRIMARY KEY,
    task_id             uuid NOT NULL,
    attempt_id          uuid NOT NULL,
    project_id          uuid NOT NULL,
    owner_account_id    uuid NOT NULL,
    request_id          text NOT NULL CHECK (char_length(request_id) BETWEEN 1 AND 200),
    idempotency_key     text NOT NULL CHECK (char_length(idempotency_key) BETWEEN 1 AND 200),
    tool                text NOT NULL CHECK (char_length(tool) BETWEEN 1 AND 200),
    tool_version        text NOT NULL CHECK (char_length(tool_version) BETWEEN 1 AND 200),
    schema_version      text NOT NULL CHECK (char_length(schema_version) BETWEEN 1 AND 100),
    effect_class        text NOT NULL CHECK (effect_class IN (
                            'pure_read', 'derived_job', 'idempotent_write', 'unknown_effect')),
    args_sha256         text NOT NULL CHECK (args_sha256 ~ '^[0-9a-f]{64}$'),
    -- Final normalized nonsecret arguments and credential references. Never a credential value.
    request_artifact    jsonb NOT NULL,
    workspace_id        text NOT NULL,
    root_identity       text NOT NULL,
    source_snapshot     text NOT NULL,
    grant_id            uuid NOT NULL,
    grant_epoch         bigint NOT NULL CHECK (grant_epoch >= 0),
    fencing_generation  bigint NOT NULL CHECK (fencing_generation >= 0),
    retry_class         text NOT NULL CHECK (retry_class IN (
                            'safe_same_inputs', 'refresh_preconditions', 'reconcile_first',
                            'requires_authorization', 'terminal')),
    reconciliation_method text NOT NULL CHECK (char_length(reconciliation_method) BETWEEN 1 AND 200),
    observation_inputs  jsonb NOT NULL DEFAULT '{}'::jsonb,
    created_at          timestamptz NOT NULL DEFAULT now(),
    FOREIGN KEY (task_id, project_id, owner_account_id) REFERENCES agent_tasks (id, project_id, owner_account_id) ON DELETE CASCADE,
    FOREIGN KEY (project_id, owner_account_id) REFERENCES projects (id, owner_account_id) ON DELETE CASCADE,
    FOREIGN KEY (attempt_id, task_id) REFERENCES agent_attempts (id, task_id) ON DELETE CASCADE,
    CONSTRAINT tool_intents_idempotency_key UNIQUE (task_id, idempotency_key)
);
CREATE INDEX tool_intents_attempt_idx ON tool_intents (attempt_id, task_id);
CREATE INDEX tool_intents_project_idx ON tool_intents (project_id, owner_account_id);

CREATE TABLE tool_receipts (
    operation_id     uuid PRIMARY KEY REFERENCES tool_intents (operation_id) ON DELETE CASCADE,
    task_id          uuid NOT NULL,
    project_id       uuid NOT NULL,
    owner_account_id uuid NOT NULL,
    status           text NOT NULL CHECK (status IN (
                         'started', 'succeeded', 'unchanged', 'refused', 'failed',
                         'unknown_effect', 'reconciled_applied', 'reconciled_not_applied',
                         'reconciled_conflict')),
    started_at       timestamptz NOT NULL DEFAULT now(),
    finished_at      timestamptz,
    before_sha256    text CHECK (before_sha256 ~ '^[0-9a-f]{64}$'),
    after_sha256     text CHECK (after_sha256 ~ '^[0-9a-f]{64}$'),
    result_artifact  jsonb,
    result_sha256    text CHECK (result_sha256 ~ '^[0-9a-f]{64}$'),
    partial          boolean NOT NULL DEFAULT false,
    -- What recovery saw when it could not tell whether the effect happened.
    observation      jsonb,
    -- How an unknown effect was decided: method, observation, decided_by, at.
    reconciliation   jsonb,
    FOREIGN KEY (task_id, project_id, owner_account_id) REFERENCES agent_tasks (id, project_id, owner_account_id) ON DELETE CASCADE,
    FOREIGN KEY (project_id, owner_account_id) REFERENCES projects (id, owner_account_id) ON DELETE CASCADE,
    CONSTRAINT tool_receipts_reconciled_pair CHECK ((status LIKE 'reconciled_%') = (reconciliation IS NOT NULL))
);
CREATE INDEX tool_receipts_task_idx ON tool_receipts (task_id, status);
CREATE INDEX tool_receipts_project_idx ON tool_receipts (project_id, owner_account_id);

-- Status moves forward only: started -> a settled status or unknown_effect, and
-- unknown_effect -> a reconciled status. Everything else is final.
CREATE FUNCTION tool_receipts_forward_only() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    IF OLD.status = 'started' AND NEW.status IN
           ('succeeded', 'unchanged', 'refused', 'failed', 'unknown_effect') THEN
        RETURN NEW;
    END IF;
    IF OLD.status = 'unknown_effect' AND NEW.status IN
           ('reconciled_applied', 'reconciled_not_applied', 'reconciled_conflict') THEN
        RETURN NEW;
    END IF;
    RAISE EXCEPTION 'tool_receipts: status % -> % is not a forward move', OLD.status, NEW.status;
END
$$;
CREATE TRIGGER tool_receipts_forward BEFORE UPDATE ON tool_receipts
    FOR EACH ROW EXECUTE FUNCTION tool_receipts_forward_only();

-- ---------------------------------------------------------------------------------------------
-- Workspace leases: one writer per project workspace, with a fencing generation.
-- ---------------------------------------------------------------------------------------------
CREATE TABLE workspace_leases (
    project_id         uuid PRIMARY KEY,
    owner_account_id   uuid NOT NULL,
    holder_incarnation uuid NOT NULL REFERENCES host_incarnations (id),
    generation         bigint NOT NULL CHECK (generation >= 1),
    expires_at         timestamptz NOT NULL,
    updated_at         timestamptz NOT NULL DEFAULT now(),
    FOREIGN KEY (project_id, owner_account_id) REFERENCES projects (id, owner_account_id) ON DELETE CASCADE
);
CREATE INDEX workspace_leases_owner_idx ON workspace_leases (owner_account_id);
CREATE INDEX workspace_leases_holder_idx ON workspace_leases (holder_incarnation);

-- ---------------------------------------------------------------------------------------------
-- Append-only triggers
-- ---------------------------------------------------------------------------------------------
CREATE TRIGGER agent_events_append_only BEFORE UPDATE OR DELETE ON agent_events
    FOR EACH ROW EXECUTE FUNCTION tasks_forbid_change();
CREATE TRIGGER context_bundles_append_only BEFORE UPDATE OR DELETE ON context_bundles
    FOR EACH ROW EXECUTE FUNCTION tasks_forbid_change();
CREATE TRIGGER tool_intents_append_only BEFORE UPDATE OR DELETE ON tool_intents
    FOR EACH ROW EXECUTE FUNCTION tasks_forbid_change();

-- ---------------------------------------------------------------------------------------------
-- Row-level security
-- ---------------------------------------------------------------------------------------------
ALTER TABLE agent_tasks ENABLE ROW LEVEL SECURITY;
ALTER TABLE agent_tasks FORCE ROW LEVEL SECURITY;
ALTER TABLE context_bundles ENABLE ROW LEVEL SECURITY;
ALTER TABLE context_bundles FORCE ROW LEVEL SECURITY;
ALTER TABLE agent_attempts ENABLE ROW LEVEL SECURITY;
ALTER TABLE agent_attempts FORCE ROW LEVEL SECURITY;
ALTER TABLE agent_events ENABLE ROW LEVEL SECURITY;
ALTER TABLE agent_events FORCE ROW LEVEL SECURITY;
ALTER TABLE tool_intents ENABLE ROW LEVEL SECURITY;
ALTER TABLE tool_intents FORCE ROW LEVEL SECURITY;
ALTER TABLE tool_receipts ENABLE ROW LEVEL SECURITY;
ALTER TABLE tool_receipts FORCE ROW LEVEL SECURITY;
ALTER TABLE workspace_leases ENABLE ROW LEVEL SECURITY;
ALTER TABLE workspace_leases FORCE ROW LEVEL SECURITY;

CREATE POLICY agent_tasks_select ON agent_tasks FOR SELECT USING (tasks_can_see_project(project_id, owner_account_id));
CREATE POLICY agent_tasks_insert ON agent_tasks FOR INSERT WITH CHECK (
    tasks_can_see_project(project_id, owner_account_id) AND created_by_account_id = app_current_account());
CREATE POLICY agent_tasks_update ON agent_tasks FOR UPDATE
    USING (tasks_can_see_project(project_id, owner_account_id))
    WITH CHECK (tasks_can_see_project(project_id, owner_account_id));

CREATE POLICY context_bundles_select ON context_bundles FOR SELECT USING (tasks_can_see_project(project_id, owner_account_id));
CREATE POLICY context_bundles_insert ON context_bundles FOR INSERT WITH CHECK (tasks_can_see_project(project_id, owner_account_id));

CREATE POLICY agent_attempts_select ON agent_attempts FOR SELECT USING (tasks_can_see_project(project_id, owner_account_id));
CREATE POLICY agent_attempts_insert ON agent_attempts FOR INSERT WITH CHECK (tasks_can_see_project(project_id, owner_account_id));
CREATE POLICY agent_attempts_update ON agent_attempts FOR UPDATE
    USING (tasks_can_see_project(project_id, owner_account_id))
    WITH CHECK (tasks_can_see_project(project_id, owner_account_id));

CREATE POLICY agent_events_select ON agent_events FOR SELECT USING (tasks_can_see_project(project_id, owner_account_id));
CREATE POLICY agent_events_insert ON agent_events FOR INSERT WITH CHECK (tasks_can_see_project(project_id, owner_account_id));

CREATE POLICY tool_intents_select ON tool_intents FOR SELECT USING (tasks_can_see_project(project_id, owner_account_id));
CREATE POLICY tool_intents_insert ON tool_intents FOR INSERT WITH CHECK (tasks_can_see_project(project_id, owner_account_id));

CREATE POLICY tool_receipts_select ON tool_receipts FOR SELECT USING (tasks_can_see_project(project_id, owner_account_id));
CREATE POLICY tool_receipts_insert ON tool_receipts FOR INSERT WITH CHECK (tasks_can_see_project(project_id, owner_account_id));
CREATE POLICY tool_receipts_update ON tool_receipts FOR UPDATE
    USING (tasks_can_see_project(project_id, owner_account_id))
    WITH CHECK (tasks_can_see_project(project_id, owner_account_id));

CREATE POLICY workspace_leases_select ON workspace_leases FOR SELECT USING (tasks_can_see_project(project_id, owner_account_id));
CREATE POLICY workspace_leases_insert ON workspace_leases FOR INSERT WITH CHECK (tasks_can_see_project(project_id, owner_account_id));
CREATE POLICY workspace_leases_update ON workspace_leases FOR UPDATE
    USING (tasks_can_see_project(project_id, owner_account_id))
    WITH CHECK (tasks_can_see_project(project_id, owner_account_id));

-- Startup recovery needs to see nonterminal attempts of every account before any account is
-- known. The narrow bypass is a SELECT-only policy for the migration role (the owner of
-- tasks_recovery_scan below), which the application role is not a member of. Nothing else
-- gets it: the function exposes ids and states only.
DO $$
BEGIN
    EXECUTE format('CREATE POLICY agent_attempts_recovery_select ON agent_attempts FOR SELECT TO %I USING (true)',
                   current_user);
END
$$;

-- ---------------------------------------------------------------------------------------------
-- Grants
-- ---------------------------------------------------------------------------------------------
GRANT SELECT, INSERT ON agent_tasks, context_bundles, agent_attempts, agent_events, tool_intents,
    tool_receipts, workspace_leases TO factory_app;
GRANT SELECT, INSERT ON host_incarnations TO factory_app;
GRANT UPDATE (last_seen_at) ON host_incarnations TO factory_app;
GRANT UPDATE (state, paused, pause_reason, version, last_seq, updated_at) ON agent_tasks TO factory_app;
GRANT UPDATE (state, conversation, outcome, error_code, version, started_at, finished_at) ON agent_attempts TO factory_app;
GRANT UPDATE (status, finished_at, before_sha256, after_sha256, result_artifact, result_sha256,
              partial, observation, reconciliation) ON tool_receipts TO factory_app;
GRANT UPDATE (holder_incarnation, generation, expires_at, updated_at) ON workspace_leases TO factory_app;

-- ---------------------------------------------------------------------------------------------
-- Recovery scan
-- ---------------------------------------------------------------------------------------------
-- Every nonterminal attempt across accounts: ids and states only, no content. A restarting
-- server calls this once to learn which owners have unfinished work, then opens one scoped
-- transaction per owner for the real recovery. Row-level security alone cannot answer "which
-- accounts have work" because every policy needs a known account, hence this one SECURITY
-- DEFINER function owned by the migration role.
CREATE FUNCTION tasks_recovery_scan()
    RETURNS TABLE (account_id uuid, project_id uuid, task_id uuid, attempt_id uuid, attempt_state text)
    LANGUAGE sql STABLE SECURITY DEFINER
    SET search_path = public, pg_temp
    AS $$ SELECT a.owner_account_id, a.project_id, a.task_id, a.id, a.state
          FROM agent_attempts a
          WHERE a.state NOT IN ('succeeded', 'failed', 'cancelled')
          ORDER BY a.admitted_at, a.id $$;
REVOKE ALL ON FUNCTION tasks_recovery_scan() FROM PUBLIC;
GRANT EXECUTE ON FUNCTION tasks_recovery_scan() TO factory_app;
