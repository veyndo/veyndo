#!/bin/sh
# Database-manager step of the bundled compose stack: creates the runtime group, the runtime login and the
# connector login with their passwords. Secrets are passed through psql variables, never echoed. Every grant
# comes from the product: `veyndo --provision` (compose service `provision`) and each later migration.
set -eu
export PGPASSWORD="$DB_PASSWORD"
psql -h postgres -U commerce -d commerce -v ON_ERROR_STOP=1 -v runtime_password="$DB_RUNTIME_PASSWORD" -v connector_password="$DB_CONNECTOR_PASSWORD" <<'SQL'
SELECT 'CREATE ROLE veyndo_runtime NOLOGIN NOSUPERUSER NOBYPASSRLS' WHERE NOT EXISTS(SELECT 1 FROM pg_roles WHERE rolname='veyndo_runtime') \gexec
SELECT 'CREATE ROLE veyndo_runtime_login LOGIN NOSUPERUSER NOBYPASSRLS' WHERE NOT EXISTS(SELECT 1 FROM pg_roles WHERE rolname='veyndo_runtime_login') \gexec
ALTER ROLE veyndo_runtime_login NOSUPERUSER NOBYPASSRLS NOREPLICATION NOCREATEDB NOCREATEROLE CONNECTION LIMIT 80 PASSWORD :'runtime_password';
GRANT veyndo_runtime TO veyndo_runtime_login;
SELECT 'CREATE ROLE veyndo_connectors LOGIN NOSUPERUSER NOBYPASSRLS' WHERE NOT EXISTS(SELECT 1 FROM pg_roles WHERE rolname='veyndo_connectors') \gexec
ALTER ROLE veyndo_connectors NOSUPERUSER NOBYPASSRLS NOREPLICATION NOCREATEDB NOCREATEROLE CONNECTION LIMIT 16 PASSWORD :'connector_password';
GRANT CONNECT ON DATABASE commerce TO veyndo_runtime,veyndo_connectors;
GRANT USAGE ON SCHEMA public TO veyndo_runtime,veyndo_connectors;
SQL
