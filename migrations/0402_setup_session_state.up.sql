-- 0402_setup_session_state (up): what a browser needs to resume an onboarding session.
--
-- The Setup gates (approved evidence, analyses, plans, approvals) live in memory inside the
-- facade and cannot be reopened after a restart. The session row survives, so the server can tell
-- the person that a session existed and was lost (`setup_session_expired`) and offer the same
-- scan again with the same intent. client_state holds only non-secret, bounded intent:
-- { "mode": "create" | "existing", "brief": "...", "approved_references": [...] }.
--
-- Locking: adding a column with a constant default does not rewrite the table.
ALTER TABLE setup_sessions
    ADD COLUMN client_state jsonb NOT NULL DEFAULT '{}'::jsonb
        CONSTRAINT setup_sessions_client_state_size CHECK (octet_length(client_state::text) <= 65536);
GRANT UPDATE (client_state) ON setup_sessions TO factory_app;
