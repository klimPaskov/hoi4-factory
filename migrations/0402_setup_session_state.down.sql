-- 0402_setup_session_state (down).
ALTER TABLE setup_sessions DROP COLUMN IF EXISTS client_state;
