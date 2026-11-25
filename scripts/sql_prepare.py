#!/usr/bin/env python3
"""Prepare every static SQL statement of the Rust sources against the migrated database; a wrong column, table or parameter type fails here instead of at run time."""
import argparse
import os
from pathlib import Path
import re
import subprocess
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
from testing.database import psql  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]
FOLDERS = ('src', 'crates', 'extensions', 'components')
CALL = re.compile(r'\bsqlx::(?:query|query_as|query_scalar)\b')
# Checked macros (`query!`, `query_as!(Row, …)`, `query_file_scalar!("path", …)`) carry their SQL as the
# first literal; the compiler checks them from `.sqlx`, and preparing them here checks the same schema again.
MACRO = re.compile(r'\bsqlx::query(_file)?(_as|_scalar)?!\s*\(')
# PostgreSQL prepares only these statement kinds; everything else is a utility command checked when it runs.
PREPARABLE = re.compile(r'\s*(?:--[^\n]*\n\s*)*\(*\s*(SELECT|INSERT|UPDATE|DELETE|MERGE|VALUES|WITH|TABLE)\b', re.I)
ESCAPES = {'n': '\n', 't': '\t', 'r': '\r', '0': '\0', '\\': '\\', '"': '"', "'": "'"}


def literal(text, at):
    """Read one Rust string literal starting at `at`; returns (value, end) or None."""
    raw = re.compile(r'r(#*)"').match(text, at)
    if raw:
        close = '"' + raw.group(1)
        end = text.find(close, raw.end())
        return (text[raw.end():end], end + len(close)) if end >= 0 else None
    if text[at:at + 1] != '"':
        return None
    out, i = [], at + 1
    while i < len(text):
        c = text[i]
        if c == '"':
            return ''.join(out), i + 1
        if c != '\\':
            out.append(c)
            i += 1
            continue
        following = text[i + 1]
        if following == '\n':
            # A trailing backslash joins the lines and drops the next line's indentation.
            i += 2
            while text[i] in ' \t\n\r':
                i += 1
        elif following == 'u':
            close = text.index('}', i)
            out.append(chr(int(text[i + 3:close], 16)))
            i = close + 1
        elif following == 'x':
            out.append(chr(int(text[i + 2:i + 4], 16)))
            i += 4
        else:
            out.append(ESCAPES.get(following, following))
            i += 2
    return None


def skip_space(text, at):
    while at < len(text) and text[at] in ' \t\n\r':
        at += 1
    return at


def constant(text, name):
    found = re.search(r'\b(?:const|static)\s+' + re.escape(name) + r'\s*:\s*&(?:\'static\s+)?str\s*=\s*', text)
    return literal(text, found.end()) if found else None


def argument(path, text, at):
    """The SQL of one call: ('static', sql) or ('dynamic', reason)."""
    at = skip_space(text, at)
    read = literal(text, at)
    if read:
        sql, end = read
        return ('static', sql) if text[skip_space(text, end):][:1] in (')', ',') else ('dynamic', 'expression')
    include = re.compile(r'include_str!\(\s*').match(text, at)
    if include:
        name = literal(text, include.end())
        file = (path.parent / name[0]) if name else None
        return ('static', file.read_text()) if file and file.exists() else ('dynamic', 'include')
    name = re.compile(r'([A-Z][A-Z0-9_]*)\s*\)').match(text, at)
    if name:
        value = constant(text, name.group(1))
        return ('static', value[0]) if value else ('dynamic', 'constant elsewhere')
    return ('dynamic', 'built at run time')


def crate_root(path):
    """The directory of the nearest Cargo.toml, against which `query_file!` paths resolve."""
    for folder in path.parents:
        if (folder / 'Cargo.toml').exists():
            return folder
    return ROOT


def macro(path, text, call):
    """The SQL of one checked macro: ('static', sql) or ('dynamic', reason)."""
    at = call.end()
    if call.group(2) == '_as':
        typed = re.compile(r'\s*[\w:<>]+\s*,').match(text, at)
        if not typed:
            return ('dynamic', 'macro without row type')
        at = typed.end()
    read = literal(text, skip_space(text, at))
    if not read:
        return ('dynamic', 'macro without literal')
    if call.group(1):
        file = crate_root(path) / read[0]
        return ('static', file.read_text()) if file.exists() else ('dynamic', 'missing query file')
    return ('static', read[0])


def statements(root=ROOT):
    """Every query call as (file, line, kind, sql-or-reason)."""
    found = []
    for folder in FOLDERS:
        for path in sorted((root / folder).rglob('*.rs')):
            # Test cases run against fixtures they create themselves, not against the commerce schema.
            if {'target', 'node_modules', 'tests'} & set(path.parts):
                continue
            text = path.read_text()
            for call in MACRO.finditer(text):
                found.append((str(path.relative_to(root)), text.count('\n', 0, call.start()) + 1,
                              *macro(path, text, call)))
            for call in CALL.finditer(text):
                at = call.end()
                generic = re.compile(r'\s*::\s*<').match(text, at)
                if generic:
                    depth, at = 1, generic.end()
                    while depth and at < len(text):
                        # `->` inside a type is not a closing bracket.
                        depth += (text[at] == '<') - (text[at] == '>' and text[at - 1] != '-')
                        at += 1
                at = skip_space(text, at)
                if text[at:at + 1] != '(':
                    continue
                kind, value = argument(path, text, at + 1)
                found.append((str(path.relative_to(root)), text.count('\n', 0, call.start()) + 1, kind, value))
    return found


def separate_schemas(root=ROOT):
    """Tables that an independently hosted service creates in its own database, outside the commerce migrations."""
    names = set()
    for folder in FOLDERS:
        for path in (root / folder).rglob('*'):
            if path.suffix not in ('.sql', '.rs') or {'target', 'node_modules', 'migrations'} & set(path.parts):
                continue
            names.update(name.lower() for name in re.findall(
                r'(?i)CREATE TABLE (?:IF NOT EXISTS )?(?:public\.)?"?([a-z_][a-z0-9_]*)', path.read_text()))
    return names


def uncovered(error, own):
    """Why a refused statement is outside this check rather than wrong, or None when it is a real failure."""
    relation = re.search(r'relation "(?:public\.)?([^"]+)" does not exist', error)
    if relation and relation.group(1).lower() in own:
        return 'table of a separately hosted service'
    # sqlx sends the type of each bound value; a bare PREPARE has to infer it from the statement alone.
    if 'could not determine data type of parameter' in error or ('operator is not unique' in error and 'unknown' in error):
        return 'parameter type comes from the bound value'
    return None


def classify(sql):
    body = sql.strip().rstrip(';')
    if ';' in re.sub(r"'(?:[^']|'')*'", '', body):
        return 'several statements'
    if not PREPARABLE.match(body):
        return 'utility command'
    return None


def prepare(rows, command):
    """Run PREPARE for each statement in one session; returns {index: error}."""
    # Markers go to standard error with \\warn, the stream the database errors use, so their order is exact.
    script = ['\\set ON_ERROR_STOP off', "SELECT set_config('rac.system','on',false);"]
    for index, (_, _, _, sql) in enumerate(rows):
        script += [f'\\warn @@{index}', 'PREPARE veyndo_check AS ' + sql.strip().rstrip(';') + ';', 'DEALLOCATE ALL;']
    script.append('\\warn @@end')
    failures, current = {}, None
    merged = subprocess.run(command, input='\n'.join(script), text=True, stdout=subprocess.DEVNULL,
                            stderr=subprocess.PIPE).stderr
    assert '@@end' in merged, 'The database session ended early: ' + merged[-400:]
    for line in merged.splitlines():
        marker = re.fullmatch(r'@@(\d+|end)', line.strip())
        if marker:
            current = None if marker.group(1) == 'end' else int(marker.group(1))
        elif current is not None and 'ERROR:' in line and current not in failures:
            failures[current] = line.split('ERROR:', 1)[1].strip()
    return failures


def schema_tables(command):
    result = subprocess.run(command, input="SELECT tablename FROM pg_tables WHERE schemaname='public';", text=True,
                            capture_output=True, check=True)
    return set(result.stdout.split())


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--list', action='store_true', help='print the statements that cannot be checked')
    args = parser.parse_args(argv)
    env = os.environ
    container = env.get('DB_CONTAINER') or env.get('TEST_DB_CONTAINER', 'veyndo-postgres-1')
    command = psql(container, 'commerce', env['TEST_DATABASE'], '-X', '-q', '-At')
    found = statements()
    static = [row for row in found if row[2] == 'static']
    dynamic = [row for row in found if row[2] == 'dynamic']
    checkable = [row for row in static if not classify(row[3])]
    utility = [row for row in static if classify(row[3])]
    # Most statements are checked query macros now (verified from the .sqlx cache below), so only the
    # remaining runtime strings are extracted here; their number falls as they become macros.
    assert found and len(checkable) > 0.8 * len(found), 'The extractor lost most statements'
    refused = prepare(checkable, command)
    own = separate_schemas() - schema_tables(command)
    outside = {index: uncovered(error, own) for index, error in refused.items() if uncovered(error, own)}
    failures = {index: error for index, error in refused.items() if index not in outside}
    # Statements on a separately hosted service's own tables are a fixed set; the ratio bounds the rest.
    escaped = [reason for reason in outside.values() if reason != 'table of a separately hosted service']
    assert len(escaped) < 0.03 * len(checkable), 'Too many statements escaped the check'
    for index, error in sorted(failures.items()):
        file, line, _, sql = checkable[index]
        print(f'FAIL {file}:{line}: {error}\n     {" ".join(sql.split())[:200]}')
    if args.list:
        for index, reason in sorted(outside.items()):
            print(f'UNCHECKED {checkable[index][0]}:{checkable[index][1]}: {reason}')
        for file, line, _, reason in dynamic:
            print(f'UNCHECKED {file}:{line}: {reason}')
        for file, line, _, sql in utility:
            print(f'UNCHECKED {file}:{line}: {classify(sql)}')
    # Negative controls: an unknown column and an unknown table must be rejected by the same path.
    controls = [('control', 0, 'static', 'SELECT no_such_column FROM tenants WHERE id=$1'),
                ('control', 0, 'static', 'SELECT 1 FROM no_such_table_for_control'),
                ('control', 0, 'static', 'SELECT id FROM tenants WHERE id=$1')]
    rejected = prepare(controls, command)
    assert set(rejected) == {0, 1}, 'Negative controls: ' + str(rejected)
    if failures:
        raise SystemExit(f'{len(failures)} of {len(checkable)} static SQL statements do not match the schema')
    assert uncovered('relation "orders_typo" does not exist', own) is None, 'An unknown commerce table must stay a failure'
    print(f'PASS {len(checkable) - len(outside)} static SQL statements prepared against the migrated schema; not covered: '
          f'{len(utility)} utility or multi-statement, {len(dynamic)} built at run time, {len(outside)} with bound-value '
          'types or a separate service schema; '
          'negative controls rejected')
    # Statements written as checked query macros are verified at compile time from the committed
    # .sqlx cache; regenerate it against this migrated schema and require it to be current.
    subprocess.run([sys.executable, str(ROOT / 'scripts/sqlx_cache.py'), '--check'], cwd=ROOT, check=True)
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
