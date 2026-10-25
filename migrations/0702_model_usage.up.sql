-- 0702_model_usage (up): metering of model calls, kept apart by funding source.
--
-- Every call reserves its conservative cost before it is made and settles afterwards. A call whose
-- usage is missing keeps its reserved amount and is marked usage_unknown: unknown cost is counted,
-- never treated as zero. The broker serializes reservations per account with an advisory
-- transaction lock, so concurrent reservations cannot overspend a budget.
--
-- Row-level security: an account sees and writes only its own usage. No DELETE policy.

CREATE TABLE model_usage (
    id              uuid PRIMARY KEY,
    account_id      uuid NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    project_id      uuid NOT NULL,
    task_id         uuid NOT NULL,
    attempt_id      uuid NOT NULL,
    funding         text NOT NULL CHECK (funding IN ('operator_allowance', 'user_key')),
    -- Set for user_key funding; cleared if the owner deletes the key, the usage row stays.
    user_key_id     uuid REFERENCES user_api_keys (id) ON DELETE SET NULL,
    provider        text NOT NULL CHECK (char_length(provider) BETWEEN 1 AND 64),
    model           text NOT NULL CHECK (char_length(model) BETWEEN 1 AND 128),
    -- UTC first day of the month the reservation was made in.
    period_month    date NOT NULL CHECK (date_part('day', period_month) = 1),
    reserved_micros bigint NOT NULL CHECK (reserved_micros >= 0),
    actual_micros   bigint CHECK (actual_micros >= 0),
    usage           jsonb,
    status          text NOT NULL DEFAULT 'reserved'
                    CHECK (status IN ('reserved', 'settled', 'usage_unknown', 'released')),
    created_at      timestamptz NOT NULL,
    settled_at      timestamptz,
    CONSTRAINT model_usage_operator_has_no_key CHECK (funding <> 'operator_allowance' OR user_key_id IS NULL),
    CONSTRAINT model_usage_settled_actual CHECK ((status = 'settled') = (actual_micros IS NOT NULL))
);
CREATE INDEX model_usage_account_month_idx ON model_usage (account_id, period_month, funding);
CREATE INDEX model_usage_task_idx ON model_usage (task_id);
CREATE INDEX model_usage_user_key_idx ON model_usage (user_key_id);

ALTER TABLE model_usage ENABLE ROW LEVEL SECURITY;
ALTER TABLE model_usage FORCE ROW LEVEL SECURITY;

CREATE POLICY model_usage_select ON model_usage FOR SELECT USING (account_id = app_current_account());
CREATE POLICY model_usage_insert ON model_usage FOR INSERT
    WITH CHECK (account_id = app_current_account() AND status = 'reserved');
CREATE POLICY model_usage_update ON model_usage FOR UPDATE
    USING (account_id = app_current_account()) WITH CHECK (account_id = app_current_account());

GRANT SELECT, INSERT ON model_usage TO factory_app;
GRANT UPDATE (actual_micros, usage, status, settled_at) ON model_usage TO factory_app;
