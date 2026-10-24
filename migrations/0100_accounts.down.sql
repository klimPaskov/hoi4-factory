-- 0100_accounts (down). Drops everything the up migration created, in dependency order.
-- The group role factory_app is shared and is deliberately left in place.
-- One statement: the RLS policies on projects reference project_members.
DROP TABLE IF EXISTS project_members, projects;
DROP FUNCTION IF EXISTS app_current_account();
DROP TABLE IF EXISTS email_verification_tokens;
DROP TABLE IF EXISTS sessions;
DROP TABLE IF EXISTS accounts;
