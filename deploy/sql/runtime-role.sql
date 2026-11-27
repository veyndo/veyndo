-- Database-manager step for a database you manage yourself; run it as a role that may create roles.
-- It creates only the passwordless runtime group. Create the logins (runtime/system, tenant, connector)
-- and their passwords in your secret manager, grant this group to the runtime/system login, and give
-- every login CONNECT on the database and USAGE on schema public.
-- Grants on tables and sequences, and the database authority, come from the product:
--   BOOTSTRAP_MODE=migrate DATABASE_URL=<migration owner> veyndo --provision
-- with DB_RUNTIME_ROLE, DB_CONNECTOR_LOGIN, DB_SYSTEM_LOGIN and DB_TENANT_LOGIN (docs/deployment.md).
-- Every later BOOTSTRAP_MODE=migrate grants the tables a release adds. The runtime login must inherit
-- this group, but NEVER the migration owner.
CREATE ROLE veyndo_runtime NOLOGIN NOSUPERUSER NOBYPASSRLS;
GRANT CONNECT ON DATABASE :"DBNAME" TO veyndo_runtime;
GRANT USAGE ON SCHEMA public TO veyndo_runtime;
