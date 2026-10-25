-- 0700_agent_grants (down).
DROP TABLE IF EXISTS agent_grants;
DROP FUNCTION IF EXISTS broker_can_see_project(uuid, uuid);
