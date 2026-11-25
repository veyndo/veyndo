#!/usr/bin/env python3
"""Store tests per component against an isolated PostgreSQL database under forced row security.

Two parts, both on a dedicated non-owner login (NOSUPERUSER NOBYPASSRLS, no table ownership):

1. `tests/store_isolation.rs` calls component store functions: it writes under shop A and
   proves that under shop B the same functions read nothing of it and change nothing.
2. A sweep over every tenant table each component owns (`components.json`): under shop B no
   row of another shop is visible, an update or delete aimed at shop A touches zero rows,
   and under shop A a row cannot be moved to another shop (row security WITH CHECK). Shop A is
   the seeded shop where it holds rows of the table, otherwise any shop that does.

A negative control repeats one probe on a login with BYPASSRLS and requires it to see A's
rows, so the sweep cannot pass merely because it queried nothing.
"""
import json
import os
import subprocess
import sys
import urllib.parse
import uuid
from collections import defaultdict

from testing.database import psql
from testing.runtime import ROOT

env = dict(os.environ)
url = urllib.parse.urlsplit(env['DATABASE_URL'])
owner = urllib.parse.unquote(url.username or 'commerce')
database = env['TEST_DATABASE']
container = env['TEST_DB_CONTAINER']
run_id = uuid.uuid4().hex[:12]
role = 'isolation_' + run_id
bypass = 'isolation_bypass_' + run_id
password = uuid.uuid4().hex
shop_a = env.get('STORE_ISOLATION_SHOP', 'atelier')
shop_b = 'isolation-sweep-' + run_id


def sql(text, check=True):
    result = subprocess.run(psql(container, owner, database, '-qAt', '-v', 'ON_ERROR_STOP=1'),
                            input=text, text=True, capture_output=True)
    if check:
        assert result.returncode == 0, result.stderr
    return result


def literal(value):
    return "'" + value.replace("'", "''") + "'"


def scoped(tenant, statement, as_role=role):
    """One rolled-back transaction on the restricted login with only the shop scope set."""
    return (f"BEGIN; SET LOCAL ROLE {as_role}; SELECT set_config('rac.tenant',{literal(tenant)},true),"
            f"set_config('rac.system','off',true) \\g /dev/null\n{statement}\nROLLBACK;\n")


def tenant_tables():
    rows = sql("SELECT c.relname FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname='public' "
               "AND c.relkind='r' AND c.relforcerowsecurity AND EXISTS(SELECT 1 FROM pg_attribute a WHERE a.attrelid=c.oid "
               "AND a.attname='tenant' AND NOT a.attisdropped) ORDER BY 1;").stdout.split()
    return set(rows)


def unforced_tables():
    return sql("SELECT c.relname FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname='public' "
               "AND c.relkind='r' AND NOT c.relforcerowsecurity AND EXISTS(SELECT 1 FROM pg_attribute a WHERE "
               "a.attrelid=c.oid AND a.attname='tenant' AND NOT a.attisdropped) ORDER BY 1;").stdout.split()


def probe(table):
    """Visible foreign rows, updated and deleted rows of a holding shop under B; that shop's row count as the owner
    sees it; the move result under the holding shop. The holding shop is shop A when it has rows, else any shop that does."""
    t = '"' + table + '"'
    holder = sql(f"SELECT tenant FROM {t} WHERE tenant IS NOT NULL ORDER BY tenant={literal(shop_a)} DESC LIMIT 1;").stdout.strip() or shop_a
    a, b = literal(holder), literal(shop_b)
    owned = int(sql(f"SELECT count(*) FROM {t} WHERE tenant={a};").stdout.strip())
    under_b = sql(scoped(shop_b,
        f"SELECT count(*) FROM {t} WHERE tenant IS DISTINCT FROM {b};\n"
        f"WITH u AS (UPDATE {t} SET tenant=tenant WHERE tenant={a} RETURNING 1) SELECT count(*) FROM u;\n"
        f"WITH d AS (DELETE FROM {t} WHERE tenant={a} RETURNING 1) SELECT count(*) FROM d;"), check=False)
    if under_b.returncode != 0:
        return {'error': under_b.stderr.strip()[:300], 'owned': owned}
    visible, updated, deleted = (int(v) for v in under_b.stdout.split())
    moved = sql(scoped(holder, f"""DO $probe$ DECLARE n bigint; BEGIN
  UPDATE {t} SET tenant={b} WHERE ctid=(SELECT ctid FROM {t} WHERE tenant={a} LIMIT 1);
  GET DIAGNOSTICS n = ROW_COUNT;
  RAISE NOTICE 'probe:%', CASE WHEN n=0 THEN 'empty' ELSE 'moved' END;
EXCEPTION WHEN insufficient_privilege THEN RAISE NOTICE 'probe:refused';
  WHEN others THEN RAISE NOTICE 'probe:other-%', SQLSTATE;
END $probe$;"""), check=False)
    outcome = next((line.split('probe:', 1)[1] for line in moved.stderr.splitlines() if 'probe:' in line),
                   'error:' + moved.stderr.strip()[:200])
    return {'owned': owned, 'visible': visible, 'updated': updated, 'deleted': deleted, 'move': outcome}


def sweep():
    mapping = json.loads((ROOT / 'components.json').read_text())['components']
    present = tenant_tables()
    assert not unforced_tables(), ('tenant tables without FORCE ROW LEVEL SECURITY', unforced_tables())
    report, failures = {}, []
    covered = set()
    for component, row in sorted(mapping.items()):
        tables = sorted(set(row.get('tables', [])) & present)
        covered |= set(tables)
        counts = defaultdict(int)
        for table in tables:
            result = probe(table)
            counts['tables'] += 1
            counts['with_rows'] += result['owned'] > 0
            if 'error' in result:
                failures.append((component, table, result['error']))
                continue
            if result['visible'] or result['updated'] or result['deleted']:
                failures.append((component, table, 'shop B saw or changed foreign rows', result))
            if result['move'] == 'moved':
                failures.append((component, table, 'shop A moved a row to shop B', result))
            elif result['move'] == 'refused':
                counts['move_refused'] += 1
            elif result['move'].startswith('other'):
                counts['move_refused_by_constraint'] += 1
            elif result['move'] != 'empty':
                failures.append((component, table, result['move']))
        if tables:
            report[component] = dict(counts)
    unowned = present - covered
    assert not unowned, ('tenant tables no component declares', sorted(unowned))
    return report, failures


def negative_control():
    """The same visibility probe on a BYPASSRLS login must see shop A's rows."""
    sql(f"CREATE ROLE {bypass} NOLOGIN NOSUPERUSER BYPASSRLS; GRANT USAGE ON SCHEMA public TO {bypass}; "
        f"GRANT SELECT ON products TO {bypass};")
    seen = sql(scoped(shop_b, f"SELECT count(*) FROM products WHERE tenant={literal(shop_a)};", bypass)).stdout.split()
    restricted = sql(scoped(shop_b, f"SELECT count(*) FROM products WHERE tenant={literal(shop_a)};")).stdout.split()
    assert int(seen[-1]) > 0, 'negative control: shop A has no products, the sweep would be vacuous'
    assert int(restricted[-1]) == 0, 'restricted login saw shop A products'
    print(f'PASS negative control: a BYPASSRLS login sees {seen[-1]} products of {shop_a}, the runtime login none', flush=True)


try:
    sql(f"CREATE ROLE {role} LOGIN PASSWORD '{password}' NOSUPERUSER NOBYPASSRLS NOINHERIT; "
        f"GRANT CONNECT ON DATABASE \"{database}\" TO {role}; GRANT USAGE ON SCHEMA public TO {role}; "
        f"GRANT SELECT,INSERT,UPDATE,DELETE,REFERENCES ON ALL TABLES IN SCHEMA public TO {role}; "
        f"GRANT USAGE,SELECT ON ALL SEQUENCES IN SCHEMA public TO {role};")
    runtime_url = urllib.parse.urlunsplit(url._replace(netloc=f'{role}:{password}@{url.hostname}:{url.port}'))
    subprocess.run(['cargo', 'test', '--locked', '--test', 'store_isolation', '--', '--ignored', '--nocapture'],
                   cwd=ROOT, env={**env, 'STORE_ISOLATION_RUNTIME_URL': runtime_url, 'STORE_ISOLATION_SHOP': shop_a},
                   check=True)
    report, failures = sweep()
    for component, counts in report.items():
        print(f"PASS {component}: {counts['tables']} tables, {counts.get('with_rows', 0)} hold shop rows; "
              f"invisible and unchangeable to another shop; moves refused by row security "
              f"{counts.get('move_refused', 0)}, by a constraint {counts.get('move_refused_by_constraint', 0)}", flush=True)
    for failure in failures:
        print('FAIL', *failure, flush=True)
    assert not failures, f'{len(failures)} isolation failures'
    negative_control()
    total = sum(c['tables'] for c in report.values())
    print(f'PASS store isolation: {len(report)} components, {total} tenant tables under forced row security', flush=True)
finally:
    for name in (role, bypass):
        sql(f"DROP OWNED BY {name}; DROP ROLE {name};", check=False)
    sys.stdout.flush()
