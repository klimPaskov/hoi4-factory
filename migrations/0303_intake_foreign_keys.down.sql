-- 0303_intake_foreign_keys (down): drops the keys again. The data is untouched.
ALTER TABLE github_repository_allowlist
    DROP CONSTRAINT IF EXISTS github_repository_allowlist_project_fk,
    DROP CONSTRAINT IF EXISTS github_repository_allowlist_account_fk;
ALTER TABLE repository_links
    DROP CONSTRAINT IF EXISTS repository_links_project_fk,
    DROP CONSTRAINT IF EXISTS repository_links_account_fk;
ALTER TABLE intake_uploads
    DROP CONSTRAINT IF EXISTS intake_uploads_project_fk,
    DROP CONSTRAINT IF EXISTS intake_uploads_account_fk;
