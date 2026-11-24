#!/usr/bin/env python3
"""Regenerate or check the `.sqlx` query cache of the `sqlx::query_as!` macros against a freshly migrated database.

The cache lets every build compile the checked queries offline (`.cargo/config.toml` sets SQLX_OFFLINE).
`--write` rewrites `.sqlx`; `--check` (the `sql_prepare` suite) regenerates into a temporary directory
and fails when a query, its columns, their types or nullability differ from the committed cache."""
import argparse
import os
import shutil
import subprocess
import sys
import tempfile
import urllib.parse
import uuid
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CACHE = ROOT / '.sqlx'


def base_url():
    url = os.environ.get('DATABASE_URL')
    if not url and (ROOT / '.env').exists():
        for line in (ROOT / '.env').read_text().splitlines():
            if line.startswith('DATABASE_URL='):
                url = line.split('=', 1)[1].strip().strip('"\'')
    if not url:
        raise SystemExit('DATABASE_URL (or .env) required for a migration database')
    return urllib.parse.urlsplit(url)


def sources_using_macros():
    """Crate roots whose sources contain a checked query macro; touching them re-runs the macros."""
    roots = set()
    for folder in ('src', 'components', 'crates', 'extensions'):
        for path in (ROOT / folder).rglob('*.rs'):
            if 'target' in path.parts:
                continue
            if 'sqlx::query_as!(' in path.read_text() or 'sqlx::query!(' in path.read_text() or 'sqlx::query_scalar!(' in path.read_text():
                crate = path.parent
                while not (crate / 'Cargo.toml').exists():
                    crate = crate.parent
                roots.add(crate)
    return sorted(roots)


def migration_binary():
    """A binary that applies the current migrations.

    The working tree builds once every query macro has cached data. A new macro has none yet, so the
    migrations then come from a HEAD build in a temporary worktree; that is exact only while the
    migrations equal HEAD, which is checked."""
    built = subprocess.run(['cargo', 'build', '--offline', '--bin', 'veyndo'], cwd=ROOT,
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    if built.returncode == 0:
        return ROOT / 'target/debug/veyndo', None
    changed = subprocess.run(['git', 'diff', '--quiet', 'HEAD', '--', 'migrations', 'src/migrations.rs', 'src/migrations'],
                             cwd=ROOT).returncode
    if changed:
        raise SystemExit('New query macros and changed migrations together: commit the migrations first, then regenerate')
    tree = Path(tempfile.mkdtemp(prefix='veyndo-sqlx-'))
    subprocess.run(['git', 'worktree', 'add', '--detach', str(tree), 'HEAD'], cwd=ROOT, check=True,
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    target = ROOT / 'target/sqlx-migrate'
    subprocess.run(['cargo', 'build', '--offline', '--bin', 'veyndo', '--target-dir', str(target)], cwd=tree, check=True)
    return target / 'debug/veyndo', tree


def generate(target, url):
    """Expand every query macro against `url`, writing one data file per query into `target`."""
    target.mkdir(parents=True, exist_ok=True)
    for crate in sources_using_macros():
        for lib in ('src/lib.rs', 'src/main.rs'):
            if (crate / lib).exists():
                (crate / lib).touch()
    env = {**os.environ, 'SQLX_OFFLINE': 'false', 'SQLX_OFFLINE_DIR': str(target), 'DATABASE_URL': url}
    subprocess.run(['cargo', 'check', '--offline', '--workspace', '--all-targets'], cwd=ROOT, env=env, check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument('--write', action='store_true')
    mode.add_argument('--check', action='store_true')
    args = parser.parse_args()
    url = base_url()
    admin = urllib.parse.urlunsplit(url)
    name = 'commerce_sqlx_' + uuid.uuid4().hex[:16]
    scratch = urllib.parse.urlunsplit(url._replace(path='/' + name))
    psql = ['psql', admin, '-X', '-q', '-v', 'ON_ERROR_STOP=1']
    if shutil.which('psql') is None:
        container = os.environ.get('TEST_DB_CONTAINER') or os.environ.get('DB_CONTAINER')
        if not container:
            raise SystemExit('psql or TEST_DB_CONTAINER required')
        psql = ['docker', 'exec', '-i', container, 'psql', '-U', url.username or 'commerce', '-d', url.path[1:] or 'postgres', '-X', '-q', '-v', 'ON_ERROR_STOP=1']
    subprocess.run(psql, input=f'CREATE DATABASE "{name}";', text=True, check=True)
    try:
        binary, tree = migration_binary()
        env = {**os.environ, 'BOOTSTRAP_MODE': 'migrate', 'DATABASE_URL': scratch, 'APP_SERVICES': '{}',
               'PLATFORM_SECRET_KEY': os.environ.get('PLATFORM_SECRET_KEY', '07' * 32)}
        try:
            subprocess.run([str(binary)], cwd=tree or ROOT, env=env, check=True, stdout=subprocess.DEVNULL)
        finally:
            if tree:
                subprocess.run(['git', 'worktree', 'remove', '--force', str(tree)], cwd=ROOT, check=False)
        if args.write:
            shutil.rmtree(CACHE, ignore_errors=True)
            generate(CACHE, scratch)
            print(f'PASS wrote {len(list(CACHE.glob("query-*.json")))} query descriptions to .sqlx')
            return
        with tempfile.TemporaryDirectory() as tmp:
            fresh = Path(tmp) / 'sqlx'
            generate(fresh, scratch)
            committed = {p.name: p.read_text() for p in CACHE.glob('query-*.json')}
            current = {p.name: p.read_text() for p in fresh.glob('query-*.json')}
            if committed != current:
                missing, stale = sorted(set(current) - set(committed)), sorted(set(committed) - set(current))
                changed = sorted(k for k in set(current) & set(committed) if current[k] != committed[k])
                raise SystemExit(f'.sqlx is out of date (missing {missing}, stale {stale}, changed {changed}); run scripts/sqlx_cache.py --write')
            print(f'PASS {len(current)} checked query macros match the migrated schema and the committed .sqlx cache')
    finally:
        subprocess.run(psql, input=f'DROP DATABASE IF EXISTS "{name}" WITH (FORCE);', text=True, check=False)


if __name__ == '__main__':
    sys.exit(main())
