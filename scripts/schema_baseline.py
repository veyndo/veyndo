#!/usr/bin/env python3
"""Fresh-install schema equality: a database set up by the server from the component baselines must
match the committed reference schema and starting rows exactly.

The reference (`fixtures/schema/reference.sql`) was taken from a database that the last release with
incremental migrations (commit ce4285d7) set up from scratch, so a pass proves the baselines produce
the same schema. A deliberate schema change regenerates it with `--write` and shows up in review.

The suite creates its own database in the selected PostgreSQL container, runs the prebuilt server in
`BOOTSTRAP_MODE=migrate` without demo data, and compares a normalized `pg_dump --schema-only` (every
statement, sorted, owners left out) plus the rows that setup creates. Generated values (timestamps,
random UUIDs and the per-database scope key) are masked, everything else must be identical.
"""
import argparse
import difflib
import os
import re
import subprocess
import sys
import urllib.parse
import uuid
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from testing.runtime import ROOT, artifact_path  # noqa: E402

REFERENCE = ROOT / 'fixtures/schema/reference.sql'
DATA_MASKS = (
    (re.compile(r'\b\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}(\.\d+)?\+00\b'), '<timestamp>'),
    (re.compile(r'\b[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\b'), '<uuid>'),
)
# The authority key is generated per database; its row is compared without id and secret.
DATA_EXCLUDE = ('public.commerce_migrations', 'veyndo_authority.keys')


def statements(text):
    """Split SQL into statements, keeping dollar-quoted bodies and string literals intact."""
    out, current, i, n, quote = [], [], 0, len(text), None
    while i < n:
        if quote:
            end = text.find(quote, i)
            end = n if end < 0 else end + len(quote)
            current.append(text[i:end])
            i, quote = end, None
            continue
        c = text[i]
        dollar = re.compile(r'\$[A-Za-z_]*\$').match(text, i) if c == '$' else None
        if dollar:
            quote = dollar.group(0)
            current.append(quote)
            i = dollar.end()
        elif c == "'":
            quote = "'"
            current.append(c)
            i += 1
            # A doubled quote inside a literal is consumed as part of the literal by the next search.
            while True:
                end = text.find("'", i)
                if end < 0:
                    current.append(text[i:])
                    i = n
                    break
                if text.startswith("''", end):
                    current.append(text[i:end + 2])
                    i = end + 2
                    continue
                current.append(text[i:end + 1])
                i = end + 1
                break
            quote = None
        elif c == ';':
            out.append(''.join(current).strip())
            current = []
            i += 1
        else:
            current.append(c)
            i += 1
    if ''.join(current).strip():
        out.append(''.join(current).strip())
    return [s for s in out if s]


def strip_meta(text):
    """Drop pg_dump comments, session settings and psql meta-commands outside of statement bodies."""
    keep, quote = [], None
    for line in text.splitlines():
        if quote is None and (line.startswith(('--', 'SET ', '\\')) or line.startswith('SELECT pg_catalog.set_config')):
            continue
        keep.append(line)
        for tag in re.findall(r'\$[A-Za-z_]*\$', line):
            quote = tag if quote is None else (None if tag == quote else quote)
    return '\n'.join(keep)


def normalize_schema(dump):
    return sorted(statements(strip_meta(dump)))


def normalize_data(dump):
    blocks, lines, table = [], dump.splitlines(), None
    rows = []
    for line in lines:
        if line.startswith('COPY '):
            table, rows = line, []
        elif line == '\\.' and table:
            masked = []
            for row in rows:
                for pattern, label in DATA_MASKS:
                    row = pattern.sub(label, row)
                masked.append(row)
            blocks.append(table + '\n' + '\n'.join(sorted(masked)))
            table = None
        elif table:
            rows.append(line)
    return sorted(blocks)


def render(schema, data):
    return '-- schema\n' + ';\n\n'.join(schema) + ';\n\n-- rows\n' + '\n\n'.join(data) + '\n'


def dump(container, user, database):
    common = ['docker', 'exec', container, 'pg_dump', '-U', user, '-d', database, '--no-owner']
    schema = subprocess.run(common + ['--schema-only'], capture_output=True, text=True, check=True).stdout
    data = subprocess.run(common + ['--data-only'] + [f'--exclude-table-data={t}' for t in DATA_EXCLUDE],
                          capture_output=True, text=True, check=True).stdout
    keys = subprocess.run(['docker', 'exec', container, 'psql', '-U', user, '-d', database, '-Atc',
                           "SELECT count(*) FILTER (WHERE active AND octet_length(secret)=32 AND id ~ '^[0-9a-f]{32}$') || '/' || count(*) FROM veyndo_authority.keys"],
                          capture_output=True, text=True, check=True).stdout.strip()
    return render(normalize_schema(schema), normalize_data(data) + [f'veyndo_authority.keys: {keys} active random 32-byte keys'])


def setup(env, container, user, binary):
    """A new database set up by the server binary alone, as a fresh installation does it."""
    name = 'schema_baseline_' + uuid.uuid4().hex
    psql = ['docker', 'exec', '-i', container, 'psql', '-U', user, '-d', 'postgres', '-qv', 'ON_ERROR_STOP=1']
    subprocess.run(psql, input=f'CREATE DATABASE "{name}";', text=True, check=True)
    url = urllib.parse.urlsplit(env['DATABASE_URL'])
    child = {**env, 'DATABASE_URL': urllib.parse.urlunsplit(url._replace(path='/' + name)),
             'BOOTSTRAP_MODE': 'migrate', 'SEED_DEMO': 'false'}
    for key in ('DATABASE_RUNTIME_URL', 'DATABASE_TENANT_URL', 'DATABASE_LISTENER_URL'):
        child.pop(key, None)
    with artifact_path('schema-baseline/migrate.log').open('w') as log:
        subprocess.run([str(binary)], cwd=ROOT, env=child, stdout=log, stderr=log, check=True)
    return name, lambda: subprocess.run(psql, input=f'DROP DATABASE "{name}" WITH (FORCE);', text=True, check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__.split('\n\n')[0])
    parser.add_argument('--write', action='store_true', help='Replace the committed reference with this setup')
    parser.add_argument('--binary', default=str(ROOT / 'target/debug/veyndo'))
    args = parser.parse_args()
    env = dict(os.environ)
    container = env.get('TEST_DB_CONTAINER') or env.get('DB_CONTAINER') or 'veyndo-postgres-1'
    user = urllib.parse.urlsplit(env['DATABASE_URL']).username or 'commerce'
    env.setdefault('MERCHANT_TOKEN', 'schema-baseline-fixture-credential')
    name, drop = setup(env, container, user, args.binary)
    try:
        actual = dump(container, user, name)
    finally:
        drop()
    if args.write:
        REFERENCE.parent.mkdir(parents=True, exist_ok=True)
        REFERENCE.write_text(actual)
        print(f'Wrote {REFERENCE.relative_to(ROOT)}')
        return
    expected = REFERENCE.read_text()
    if actual != expected:
        diff = ''.join(difflib.unified_diff(expected.splitlines(True), actual.splitlines(True), 'reference', 'fresh setup', n=2))
        artifact_path('schema-baseline/difference.diff').write_text(diff)
        print(diff[:20000])
        raise SystemExit('FAIL fresh-install schema differs from fixtures/schema/reference.sql')
    print(f'PASS fresh-install schema and starting rows equal the reference ({expected.count(";")} statements)')


if __name__ == '__main__':
    main()
