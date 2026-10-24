-- Development database roles. Runs once, when the db volume is first created.
--
-- POSTGRES_USER (factory) is a superuser. It owns the tables and runs the migrations, so it is
-- the DATABASE_MIGRATION_URL role. The server itself connects as factory_server, a plain login
-- role that is a member of factory_app: no superuser, no BYPASSRLS, not a table owner. Row-level
-- security therefore applies in development exactly as it does in production.
--
-- Development only: the database accepts any local connection without a password and its port
-- is not published outside the Compose network.

CREATE ROLE factory_app NOLOGIN NOSUPERUSER NOBYPASSRLS NOCREATEDB NOCREATEROLE;
CREATE ROLE factory_server LOGIN NOSUPERUSER NOBYPASSRLS NOCREATEDB NOCREATEROLE;
GRANT factory_app TO factory_server;
