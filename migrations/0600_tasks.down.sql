-- 0600_tasks (down). Children first. The shared role factory_app, app_current_account() and the
-- accounts tables belong to 0100 and stay.
DROP FUNCTION IF EXISTS tasks_recovery_scan();
DROP TABLE IF EXISTS workspace_leases;
DROP TABLE IF EXISTS tool_receipts;
DROP TABLE IF EXISTS tool_intents;
DROP TABLE IF EXISTS agent_events;
DROP TABLE IF EXISTS agent_attempts;
DROP TABLE IF EXISTS context_bundles;
DROP TABLE IF EXISTS agent_tasks;
DROP TABLE IF EXISTS host_incarnations;
DROP FUNCTION IF EXISTS agent_attempts_guard();
DROP FUNCTION IF EXISTS tool_receipts_forward_only();
DROP FUNCTION IF EXISTS tasks_forbid_change();
DROP FUNCTION IF EXISTS tasks_can_see_project(uuid, uuid);
