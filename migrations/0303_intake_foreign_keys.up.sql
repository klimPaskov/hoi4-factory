-- 0303_intake_foreign_keys (up): the foreign keys that the intake migrations (0300 to 0302) left out
-- because the accounts tables did not exist yet when they were written.
--
-- account_id references accounts (id). (project_id, account_id) references projects (id,
-- owner_account_id), the unique pair that 0100 declares (projects_id_owner_key): it makes project_id
-- a foreign key to projects and also pins account_id to the project owner, which is how the
-- storage layout and the row policies use it. Both cascade, so the retention job that removes a
-- project or an account for good (it runs under an operator role) also removes the intake rows.
--
-- Row-level security: PostgreSQL runs the referential checks with the table owner's rights, so the
-- policies on the referenced tables do not hide a parent row from the check, and the policies on
-- the intake tables (app.account_id = account_id) are unchanged.
--
-- Existing rows must already satisfy the keys. No deployment has intake rows before this migration
-- because the intake routes arrive with it. Locking: ADD FOREIGN KEY takes SHARE ROW EXCLUSIVE on
-- the intake table and on accounts or projects for the duration of the validation scan; the intake
-- tables are empty or tiny, so the lock is held for a moment.

ALTER TABLE intake_uploads
    ADD CONSTRAINT intake_uploads_account_fk FOREIGN KEY (account_id) REFERENCES accounts (id) ON DELETE CASCADE,
    ADD CONSTRAINT intake_uploads_project_fk FOREIGN KEY (project_id, account_id)
        REFERENCES projects (id, owner_account_id) ON DELETE CASCADE;

ALTER TABLE repository_links
    ADD CONSTRAINT repository_links_account_fk FOREIGN KEY (account_id) REFERENCES accounts (id) ON DELETE CASCADE,
    ADD CONSTRAINT repository_links_project_fk FOREIGN KEY (project_id, account_id)
        REFERENCES projects (id, owner_account_id) ON DELETE CASCADE;

ALTER TABLE github_repository_allowlist
    ADD CONSTRAINT github_repository_allowlist_account_fk FOREIGN KEY (account_id) REFERENCES accounts (id) ON DELETE CASCADE,
    ADD CONSTRAINT github_repository_allowlist_project_fk FOREIGN KEY (project_id, account_id)
        REFERENCES projects (id, owner_account_id) ON DELETE CASCADE;
