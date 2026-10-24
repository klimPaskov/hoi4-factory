-- 0401_setup_transactions (up): native transaction projections, C-04 style receipts and the
-- fencing sequence of the project lease.
--
-- setup_transactions projects the native Setup journal (which stays on the workspace volume).
-- The journal is the authority: every row here can be rebuilt from it, and recovery reads the
-- journal, never this table, to decide what is allowed.
--
-- setup_receipts follows C-04: the intent (operation, request, idempotency key, implementation,
-- normalized request and its hash, root identity, grant, fencing generation, retry class,
-- reconciliation method) is inserted BEFORE the effect starts, and the outcome columns are
-- filled afterwards. The application role may update only the outcome columns, so an intent
-- cannot be rewritten after the fact.

CREATE SEQUENCE setup_fence_seq AS bigint;

CREATE TABLE setup_transactions (
    id                     uuid PRIMARY KEY,
    -- The native transaction ID, equal to the plan ID that was applied.
    journal_id             uuid NOT NULL,
    plan_id                uuid,
    session_id             uuid,
    project_id             uuid NOT NULL,
    owner_account_id       uuid NOT NULL,
    mod_root               text NOT NULL DEFAULT '',
    -- Adapter-level state. 'applying' and 'recovering' are intents with an unknown outcome until
    -- the journal has been read again.
    state                  text NOT NULL CHECK (state IN (
                               'applying', 'completed', 'rolled_back', 'staging_discarded',
                               'recovery_required', 'recovering', 'refused_before_effect', 'unknown')),
    -- Native journal state and what it permits, as last read.
    journal_state          text,
    recovery_variant       text CHECK (recovery_variant IN (
                               'resumable_staging', 'discardable_staging',
                               'rollback_required_partial_apply', 'verifiable_finalization',
                               'manual_review')),
    allowed_action         text CHECK (allowed_action IN ('resume', 'rollback', 'discard_staging')),
    project_apply_started  boolean,
    journal_sha256         text,
    plan_sha256            text,
    root_identity          text,
    pre_inventory_sha256   text,
    post_inventory_sha256  text,
    fence_generation       bigint,
    holder                 text NOT NULL,
    started_at             timestamptz NOT NULL,
    updated_at             timestamptz NOT NULL,
    finished_at            timestamptz,
    FOREIGN KEY (plan_id, project_id) REFERENCES setup_plans (id, project_id) ON DELETE CASCADE,
    FOREIGN KEY (session_id, project_id) REFERENCES setup_sessions (id, project_id) ON DELETE CASCADE,
    FOREIGN KEY (project_id, owner_account_id) REFERENCES projects (id, owner_account_id) ON DELETE CASCADE,
    CONSTRAINT setup_transactions_journal_key UNIQUE (project_id, journal_id)
);
CREATE INDEX setup_transactions_plan_idx ON setup_transactions (plan_id, project_id);
CREATE INDEX setup_transactions_session_idx ON setup_transactions (session_id, project_id);
CREATE INDEX setup_transactions_owner_idx ON setup_transactions (owner_account_id);
CREATE INDEX setup_transactions_project_idx ON setup_transactions (project_id, started_at DESC);
-- At most one unfinished transaction per project: two owners cannot both be recorded as driving
-- one project's setup.
CREATE UNIQUE INDEX setup_transactions_one_unfinished_key ON setup_transactions (project_id)
    WHERE state IN ('applying', 'recovery_required', 'recovering', 'unknown');

CREATE TABLE setup_receipts (
    id                     uuid PRIMARY KEY,
    session_id             uuid,
    project_id             uuid NOT NULL,
    owner_account_id       uuid NOT NULL,
    actor_account_id       uuid NOT NULL REFERENCES accounts (id),
    -- Intent, written before the effect.
    operation              text NOT NULL CHECK (operation IN (
                               'inspect_project', 'confirm_setup_evidence', 'analyze_setup',
                               'confirm_setup_analysis', 'plan_setup', 'resolve_setup_conflicts',
                               'approve_setup_plan', 'apply_setup_plan', 'recover_setup')),
    request_id             uuid NOT NULL,
    idempotency_key        text,
    implementation         text NOT NULL,
    setup_revision         text NOT NULL,
    setup_has_pr105_fix    boolean NOT NULL,
    schema_version         text NOT NULL DEFAULT '1',
    -- Final normalized non-secret arguments; credential values never appear here.
    request                jsonb NOT NULL,
    request_sha256         text NOT NULL,
    subject_id             uuid,
    root_identity          text,
    grant_id               text,
    revocation_epoch       bigint,
    fence_generation       bigint,
    effect_class           text NOT NULL CHECK (effect_class IN ('read_only', 'project_write', 'external_call')),
    retry_class            text NOT NULL CHECK (retry_class IN (
                               'safe_same_inputs', 'refresh_preconditions', 'reconcile_first',
                               'requires_authorization', 'terminal')),
    reconciliation         text NOT NULL,
    started_at             timestamptz NOT NULL,
    -- Outcome, written after the effect.
    status                 text NOT NULL DEFAULT 'intent' CHECK (status IN (
                               'intent', 'succeeded', 'refused', 'failed', 'unknown_effect')),
    finished_at            timestamptz,
    before_inventory_sha256 text,
    after_inventory_sha256  text,
    result                 jsonb,
    result_sha256          text,
    error_code             text,
    error_message          text,
    FOREIGN KEY (session_id, project_id) REFERENCES setup_sessions (id, project_id) ON DELETE CASCADE,
    FOREIGN KEY (project_id, owner_account_id) REFERENCES projects (id, owner_account_id) ON DELETE CASCADE,
    CONSTRAINT setup_receipts_finished_pair CHECK ((status = 'intent') = (finished_at IS NULL))
);
CREATE INDEX setup_receipts_session_idx ON setup_receipts (session_id, project_id);
CREATE INDEX setup_receipts_project_idx ON setup_receipts (project_id, started_at DESC);
CREATE INDEX setup_receipts_owner_idx ON setup_receipts (owner_account_id);
CREATE INDEX setup_receipts_actor_idx ON setup_receipts (actor_account_id);
CREATE INDEX setup_receipts_subject_idx ON setup_receipts (subject_id) WHERE subject_id IS NOT NULL;
-- A caller idempotency key is used once per project for an effect.
CREATE UNIQUE INDEX setup_receipts_idempotency_key ON setup_receipts (project_id, idempotency_key)
    WHERE idempotency_key IS NOT NULL;
-- Intents that never settled: what a reconciler looks at after a crash.
CREATE INDEX setup_receipts_unsettled_idx ON setup_receipts (project_id) WHERE status = 'intent';

ALTER TABLE setup_transactions ENABLE ROW LEVEL SECURITY;
ALTER TABLE setup_transactions FORCE ROW LEVEL SECURITY;
ALTER TABLE setup_receipts ENABLE ROW LEVEL SECURITY;
ALTER TABLE setup_receipts FORCE ROW LEVEL SECURITY;

CREATE POLICY setup_transactions_select ON setup_transactions FOR SELECT USING (setup_can_see_project(project_id, owner_account_id));
CREATE POLICY setup_transactions_insert ON setup_transactions FOR INSERT WITH CHECK (setup_can_see_project(project_id, owner_account_id));
CREATE POLICY setup_transactions_update ON setup_transactions FOR UPDATE
    USING (setup_can_see_project(project_id, owner_account_id))
    WITH CHECK (setup_can_see_project(project_id, owner_account_id));

CREATE POLICY setup_receipts_select ON setup_receipts FOR SELECT USING (setup_can_see_project(project_id, owner_account_id));
CREATE POLICY setup_receipts_insert ON setup_receipts FOR INSERT WITH CHECK (
    setup_can_see_project(project_id, owner_account_id) AND actor_account_id = app_current_account());
CREATE POLICY setup_receipts_update ON setup_receipts FOR UPDATE
    USING (setup_can_see_project(project_id, owner_account_id))
    WITH CHECK (setup_can_see_project(project_id, owner_account_id));

GRANT SELECT, INSERT ON setup_transactions, setup_receipts TO factory_app;
GRANT UPDATE (state, journal_state, recovery_variant, allowed_action, project_apply_started,
              journal_sha256, pre_inventory_sha256, post_inventory_sha256, fence_generation, holder,
              updated_at, finished_at) ON setup_transactions TO factory_app;
-- Outcome columns only: the intent columns are immutable for the application role.
GRANT UPDATE (status, finished_at, before_inventory_sha256, after_inventory_sha256, result,
              result_sha256, error_code, error_message) ON setup_receipts TO factory_app;
GRANT USAGE, SELECT ON SEQUENCE setup_fence_seq TO factory_app;
