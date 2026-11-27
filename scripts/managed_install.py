#!/usr/bin/env python3
"""Fresh install on a managed-PostgreSQL-shaped database: a non-superuser owner owns the objects, another role owns
the database and schema `public`, the provider owns the extensions. Runs migrate → `--provision` (grants, authority,
verification) → `serve` with required RLS and enforced authority → operator bootstrap → a sign-in and native requests.
Creates and drops only uniquely named synthetic roles and one database; passwords are generated and never printed."""
import json
import os
import secrets
import socket
import subprocess
import urllib.error
import urllib.parse
import urllib.request
import uuid
from testing.database import psql
from testing.runtime import ROOT, artifact_path, serve, stop

container = os.environ.get('TEST_DB_CONTAINER') or os.environ.get('DB_CONTAINER', 'veyndo-postgres-1')
suffix = uuid.uuid4().hex[:10]
database = 'managed_install_' + suffix
admin, owner, runtime = 'mi_admin_' + suffix, 'mi_owner_' + suffix, 'mi_runtime_' + suffix
system, tenant, connector = 'mi_system_' + suffix, 'mi_tenant_' + suffix, 'mi_connector_' + suffix
logins = {role: secrets.token_hex(24) for role in (owner, system, tenant, connector)}
binary = str(ROOT / 'target/debug/veyndo')
checks = []


def sql(text, db='postgres'):
    """The database manager: the container's administrative login."""
    return subprocess.run(psql(container, 'commerce', db, '-XqAt', '-v', 'ON_ERROR_STOP=1'),
                          input=text, capture_output=True, text=True, check=True).stdout.strip()


def passed(message):
    checks.append(message)
    print('PASS', message, flush=True)


base_url = urllib.parse.urlsplit(os.environ['DATABASE_URL'])


def url(role):
    netloc = f'{role}:{logins[role]}@{base_url.hostname}:{base_url.port}'
    return urllib.parse.urlunsplit(base_url._replace(netloc=netloc, path='/' + database))


env = dict(os.environ)
# This suite owns another fresh database: never inherit the parent fixture's runtime, listener or authority settings.
for key in ['DATABASE_RUNTIME_URL', 'DATABASE_TENANT_URL', 'DATABASE_LISTENER_URL', 'DB_RLS_REQUIRED',
            'DB_SCOPE_REQUIRED', 'DB_SCOPE_STAGED', 'MERCHANT_TOKEN', 'PAYPAL_ACCOUNTS', 'OPENAI_API_KEY',
            'ANTHROPIC_API_KEY', 'PLATFORM_ADMIN_EMAIL', 'PLATFORM_ADMIN_PASSWORD', 'DB_SYSTEM_LOGIN',
            'DB_TENANT_LOGIN', 'DB_RUNTIME_ROLE', 'DB_CONNECTOR_LOGIN']:
    env.pop(key, None)
env.update(APP_SERVICES='{}', OLLAMA_URL='http://127.0.0.1:1', SEED_DEMO='false', ALLOW_PUBLIC_SIGNUP='false',
           ALLOW_BOOTSTRAP_AUTH='false', PROCESS_ROLE='all', DB_POOL_MAX='6', DB_POOL_MIN='0')
owner_env = {**env, 'BOOTSTRAP_MODE': 'migrate', 'DATABASE_URL': url(owner),
             'DB_RUNTIME_ROLE': runtime, 'DB_CONNECTOR_LOGIN': connector}


def job(*args, extra=None, ok=True):
    result = subprocess.run([binary, *args], cwd=ROOT, env={**owner_env, **(extra or {})},
                            capture_output=True, text=True, timeout=180)
    output = result.stdout + result.stderr
    for secret in logins.values():
        assert secret not in output, 'A password reached the job output'
    assert (result.returncode == 0) == ok, (args, result.returncode, output[-3000:])
    return output


def call(base, path, body=None, token='', tenant='', expected=200):
    headers = {'Content-Type': 'application/json',
               **({'Authorization': 'Bearer ' + token} if token else {}),
               **({'x-tenant': tenant} if tenant else {})}
    request = urllib.request.Request(base + path, data=None if body is None else json.dumps(body).encode(),
                                     headers=headers)
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            status, value = response.status, json.load(response)
    except urllib.error.HTTPError as error:
        status, value = error.code, json.load(error)
    assert status == expected, (path, status, expected, value)
    return value


def privilege(role, table, kind):
    return sql(f"SELECT has_table_privilege('{role}','public.{table}','{kind}')", database) == 't'


server = None
try:
    # Database manager: logins and passwords, a database and schema owned by an administrator rather than the
    # migration owner, and provider-owned extensions (on managed PostgreSQL the provider installs them).
    sql(f"""CREATE ROLE {admin} NOLOGIN;
CREATE ROLE {owner} LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOBYPASSRLS PASSWORD '{logins[owner]}';
CREATE ROLE {runtime} NOLOGIN NOSUPERUSER NOBYPASSRLS;
CREATE ROLE {system} LOGIN NOSUPERUSER NOBYPASSRLS NOCREATEDB NOCREATEROLE PASSWORD '{logins[system]}' IN ROLE {runtime};
CREATE ROLE {tenant} LOGIN NOSUPERUSER NOBYPASSRLS NOCREATEDB NOCREATEROLE NOINHERIT PASSWORD '{logins[tenant]}';
CREATE ROLE {connector} LOGIN NOSUPERUSER NOBYPASSRLS NOCREATEDB NOCREATEROLE PASSWORD '{logins[connector]}';
GRANT {system} TO {owner} WITH ADMIN OPTION;
CREATE DATABASE {database} OWNER {admin};""")
    sql(f"""ALTER SCHEMA public OWNER TO {admin};
REVOKE CREATE ON SCHEMA public FROM PUBLIC;
GRANT USAGE,CREATE ON SCHEMA public TO {owner};
GRANT USAGE ON SCHEMA public TO {runtime},{system},{tenant},{connector};
GRANT CONNECT,CREATE,TEMPORARY ON DATABASE {database} TO {owner};
GRANT CONNECT ON DATABASE {database} TO {runtime},{system},{tenant},{connector};
CREATE EXTENSION pgcrypto;
CREATE EXTENSION pg_trgm WITH SCHEMA public;""", database)
    assert sql(f"SELECT count(*) FROM pg_extension e JOIN pg_roles r ON r.oid=e.extowner WHERE r.rolname='{owner}'", database) == '0'

    output = job()
    assert 'Migration-only setup complete' in output
    assert f'Runtime role {runtime}: ' in output and f'Connector login {connector}: ' in output
    assert sql(f"SELECT count(*) FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname='public' AND c.relkind='r' AND pg_get_userbyid(c.relowner)<>'{owner}'", database) == '0'
    assert privilege(runtime, 'products', 'INSERT') and not privilege(runtime, 'connector_config', 'SELECT')
    assert privilege(connector, 'connector_jobs', 'DELETE') and privilege(connector, 'commerce_migrations', 'SELECT')
    assert not privilege(connector, 'products', 'SELECT')
    passed('migrate as a non-superuser owner beside provider-owned extensions applies every baseline and grants the runtime group and connector login')

    job('--provision', extra={'DB_TENANT_LOGIN': tenant}, ok=False)
    job('--provision', extra={'DB_CONNECTOR_LOGIN': 'mi_missing_' + suffix}, ok=False)
    # PostgreSQL leaves PUBLIC's TEMPORARY grant in place when the migration owner is not the database owner.
    output = job('--provision', extra={'DB_SYSTEM_LOGIN': system, 'DB_TENANT_LOGIN': tenant}, ok=False)
    assert 'can create temporary objects' in output
    passed('provision fails closed on one authority login, an unknown connector login and a tenant login that keeps TEMPORARY')

    sql(f'REVOKE TEMPORARY ON DATABASE {database} FROM PUBLIC; GRANT TEMPORARY ON DATABASE {database} TO {system};', database)
    authority = {'DB_SYSTEM_LOGIN': system, 'DB_TENANT_LOGIN': tenant}
    for _ in range(2):
        output = job('--provision', extra=authority)
        assert f'System login {system}: ' in output and f'Tenant login {tenant}: ' in output
        assert 'Provisioning complete' in output and '"enforced": true' in output.replace('":true', '": true')
    status = json.loads(sql('SELECT public.veyndo_scope_status()', database))
    assert status == {'enforced': True, 'tenantRole': tenant, 'systemRole': system}, status
    assert privilege(tenant, 'products', 'SELECT') and not privilege(tenant, 'user_sessions', 'SELECT')
    passed('repeated provision configures and activates the authority with the named logins and verifies every grant')

    # A later release adds tables: the normal migration job grants them without SQL by hand.
    sql(f"""SET ROLE {owner};
CREATE TABLE public.release_probe(tenant text NOT NULL, id text NOT NULL);
ALTER TABLE public.release_probe ENABLE ROW LEVEL SECURITY; ALTER TABLE public.release_probe FORCE ROW LEVEL SECURITY;
CREATE TABLE public.connector_release_probe(id text);""", database)
    output = job()
    assert f'Tenant login {tenant}: ' in output
    assert privilege(runtime, 'release_probe', 'UPDATE') and privilege(tenant, 'release_probe', 'INSERT')
    assert privilege(connector, 'connector_release_probe', 'INSERT') and not privilege(runtime, 'connector_release_probe', 'SELECT')
    assert not privilege(tenant, 'connector_release_probe', 'SELECT')
    sql(f'SET ROLE {owner}; DROP TABLE public.release_probe, public.connector_release_probe;', database)
    passed('a later migration grants new Core tables to the runtime group and tenant login and new connector tables only to the connector login')

    with socket.socket() as probe:
        probe.bind(('127.0.0.1', 0))
        bind = f'127.0.0.1:{probe.getsockname()[1]}'
    base = 'http://' + bind
    serving = {**env, 'BOOTSTRAP_MODE': 'serve', 'BIND_ADDR': bind, 'DATABASE_URL': url(system),
               'DATABASE_RUNTIME_URL': url(system), 'DATABASE_TENANT_URL': url(tenant),
               'DB_RLS_REQUIRED': 'true', 'DB_SCOPE_REQUIRED': 'true'}
    with artifact_path('managed-install-server.log').open('w') as log:
        server = serve(serving, base, log)
        operator = {'PLATFORM_ADMIN_EMAIL': f'operator-{suffix}@example.test',
                    'PLATFORM_ADMIN_PASSWORD': secrets.token_urlsafe(27),
                    'PLATFORM_ADMIN_NAME': 'Synthetic Operator'}
        output = job('--provision', extra=operator)
        assert operator['PLATFORM_ADMIN_PASSWORD'] not in output
        assert 'Personal operator ready' in output and 'Provisioning complete' in output
        assert sql('SELECT count(*) FROM platform_operators WHERE active', database) == '1'
        passed('provision with operator credentials bootstraps the first operator in system scope while Core serves')

        call(base, '/api/auth/login', {'email': operator['PLATFORM_ADMIN_EMAIL'], 'password': 'Wrong-password-synthetic!'}, expected=401)
        session = call(base, '/api/auth/login', {'email': operator['PLATFORM_ADMIN_EMAIL'],
                                                 'password': operator['PLATFORM_ADMIN_PASSWORD']})
        assert call(base, '/api/platform/session', token=session['token'])['user']['name'] == 'Synthetic Operator'
        shop = 'managed-' + suffix
        call(base, '/api/platform/shops', {'id': shop, 'name': 'Managed install shop', 'seedCatalog': False},
             token=session['token'])
        call(base, '/api/merchant/overview', token=session['token'], tenant=shop)
        assert sql(f"SELECT count(*) FROM tenants WHERE id='{shop}'", database) == '1'
        passed('the operator signs in and creates a shop; a tenant-scoped Studio request succeeds under required RLS and enforced authority')
finally:
    if server:
        stop(server)
    sql(f"SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname='{database}';")
    sql(f'DROP DATABASE IF EXISTS {database} WITH (FORCE);')
    for role in (connector, tenant, system, runtime, owner, admin):
        sql(f'DROP ROLE IF EXISTS {role};')
print(f'PASS managed install: {len(checks)} checks', flush=True)
