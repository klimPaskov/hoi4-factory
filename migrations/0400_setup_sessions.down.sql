-- 0400_setup_sessions (down). Children first. The shared role factory_app and
-- app_current_account() belong to 0100 and stay.
DROP TABLE IF EXISTS setup_approvals;
DROP TABLE IF EXISTS setup_plans;
DROP TABLE IF EXISTS setup_analyses;
DROP TABLE IF EXISTS setup_scans;
DROP TABLE IF EXISTS setup_sessions;
DROP FUNCTION IF EXISTS setup_can_see_project(uuid, uuid);
