"""Derive semantic component versions from contract changes, write per-component changelogs from git history and plan (never perform) a publish.

Development tooling only; used by scripts/component_mirrors.py (`--versions`, `--record`,
`--publish --dry-run`). A component's version moves when it is recorded:

* **major** when its contract (`contracts/http/<component>.openapi.json`) breaks a caller: a
  route, method or tool removed, a permission changed, a tool input that newly requires or
  rejects something (required property added, property or enum value removed, type narrowed,
  limit tightened, unknown keys newly refused) or a declared output that drops or retypes a field;
* **minor** when the contract only grows (new route, tool or optional input, wider limits);
* **patch** when the contract is unchanged apart from descriptions or source locations, or only
  the component's own source files changed;
* unchanged otherwise.

The previous contract is read from git at the commit the previous version was recorded at, so the
classification compares real files, not digests alone. The combined contract versions the SDKs.
"""
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts/sdks'))
import contracts  # noqa: E402

INITIAL = '1.0.0'
CHANGELOGS = ROOT / 'contracts/changelogs'
LEVELS = {None: 0, 'patch': 1, 'minor': 2, 'major': 3}
# Changelog paths of the SDKs, and (SDK_SOURCES) the inputs a patch release follows: the generators and the
# hand-written examples and smoke tests. Generated SDK files are left out: they follow from the contract and
# the generators, and a recorded release rewrites them with the new version.
SDK_PATHS = ['sdks', 'scripts/sdks', 'contracts/http/veyndo.openapi.json']
SDK_SOURCES = ['scripts/sdks', 'sdks/typescript/examples', 'sdks/typescript/tests', 'sdks/python/examples',
               'sdks/python/tests', 'sdks/rust/examples', 'sdks/rust/smoke']


# Version arithmetic ----------------------------------------------------------------------
def bump(version, level):
    major, minor, patch = (int(part) for part in version.split('.'))
    if level == 'major':
        return f'{major + 1}.0.0'
    if level == 'minor':
        return f'{major}.{minor + 1}.0'
    if level == 'patch':
        return f'{major}.{minor}.{patch + 1}'
    return version


def is_semver(value):
    return isinstance(value, str) and re.fullmatch(r'(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)', value) is not None


# Contract comparison ---------------------------------------------------------------------
def types(schema):
    kind = schema.get('type')
    return set(kind) if isinstance(kind, list) else ({kind} if kind else set())


TIGHTER_IF_SMALLER = ('maxLength', 'maxItems', 'maximum')
TIGHTER_IF_LARGER = ('minLength', 'minItems', 'minimum')


def input_changes(old, new, where, out):
    """Breaking/additive differences of an input schema: what a caller may send."""
    if not isinstance(old, dict) or not isinstance(new, dict):
        return
    if types(old) and types(new) and not types(old) <= types(new):
        out.append(('major', f'{where}: type {sorted(types(old))} -> {sorted(types(new))}'))
    elif types(new) > types(old):
        out.append(('minor', f'{where}: also accepts {sorted(types(new) - types(old))}'))
    if 'enum' in old or 'enum' in new:
        removed = set(map(json.dumps, old.get('enum', []))) - set(map(json.dumps, new.get('enum', [])))
        added = set(map(json.dumps, new.get('enum', []))) - set(map(json.dumps, old.get('enum', [])))
        if 'enum' in new and (removed or 'enum' not in old):
            out.append(('major', f'{where}: values {sorted(removed) or "restricted"} no longer accepted'))
        elif added or 'enum' not in new:
            out.append(('minor', f'{where}: more values accepted'))
    for key in TIGHTER_IF_SMALLER + TIGHTER_IF_LARGER:
        if old.get(key) != new.get(key):
            tighter = key in new and (key not in old or (new[key] < old[key]) == (key in TIGHTER_IF_SMALLER))
            out.append(('major' if tighter else 'minor', f'{where}: {key} {old.get(key)} -> {new.get(key)}'))
    if old.get('additionalProperties') is not False and new.get('additionalProperties') is False:
        out.append(('major', f'{where}: unknown fields now rejected'))
    old_props, new_props = old.get('properties', {}), new.get('properties', {})
    for name in sorted(set(old_props) - set(new_props)):
        out.append(('major', f'{where}.{name}: removed'))
    for name in sorted(set(new_props) - set(old_props)):
        out.append(('major' if name in new.get('required', []) else 'minor', f'{where}.{name}: added'))
    for name in sorted(set(new.get('required', [])) - set(old.get('required', [])) - (set(new_props) - set(old_props))):
        out.append(('major', f'{where}.{name}: now required'))
    for name in sorted(set(old_props) & set(new_props)):
        input_changes(old_props[name], new_props[name], f'{where}.{name}', out)
    input_changes(old.get('items'), new.get('items'), f'{where}[]', out)


def output_changes(old, new, where, out):
    """Breaking/additive differences of a declared output: what a caller may rely on."""
    if not isinstance(old, dict) or not isinstance(new, dict):
        return
    if types(old) and types(new) and not types(new) <= types(old):
        out.append(('major', f'{where}: may now be {sorted(types(new) - types(old))}'))
    old_props, new_props = old.get('properties', {}), new.get('properties', {})
    for name in sorted(set(old_props) - set(new_props)):
        out.append(('major', f'{where}.{name}: removed'))
    for name in sorted(set(old.get('required', [])) - set(new.get('required', []))):
        if name in new_props:
            out.append(('major', f'{where}.{name}: no longer always present'))
    for name in sorted(set(new_props) - set(old_props)):
        out.append(('minor', f'{where}.{name}: added'))
    for name in sorted(set(old_props) & set(new_props)):
        output_changes(old_props[name], new_props[name], f'{where}.{name}', out)
    output_changes(old.get('items'), new.get('items'), f'{where}[]', out)


def compare(old, new):
    """(level, reasons) between two contract documents; level None when they are equal."""
    if old is None:
        return 'minor', ['no previous contract to compare with']
    out = []
    old_ops = {(p, m): op for p, item in old.get('paths', {}).items() for m, op in item.items()}
    new_ops = {(p, m): op for p, item in new.get('paths', {}).items() for m, op in item.items()}
    for path, method in sorted(set(old_ops) - set(new_ops)):
        out.append(('major', f'{method.upper()} {path}: removed'))
    for path, method in sorted(set(new_ops) - set(old_ops)):
        out.append(('minor', f'{method.upper()} {path}: added'))
    for key in sorted(set(old_ops) & set(new_ops)):
        before, after = old_ops[key], new_ops[key]
        if before.get('x-veyndo-permission') != after.get('x-veyndo-permission'):
            out.append(('major', f'{key[1].upper()} {key[0]}: permission {before.get("x-veyndo-permission")} -> {after.get("x-veyndo-permission")}'))
    schemas_old = old.get('components', {}).get('schemas', {})
    schemas_new = new.get('components', {}).get('schemas', {})
    old_tools = {t['name']: t for t in old.get('x-veyndo-tools', [])}
    new_tools = {t['name']: t for t in new.get('x-veyndo-tools', [])}
    for name in sorted(set(old_tools) - set(new_tools)):
        out.append(('major', f'tool {name}: removed'))
    for name in sorted(set(new_tools) - set(old_tools)):
        out.append(('minor', f'tool {name}: added'))
    resolve = lambda schemas, ref: schemas.get(ref['$ref'].rsplit('/', 1)[1], {}) if ref else None
    for name in sorted(set(old_tools) & set(new_tools)):
        before, after = old_tools[name], new_tools[name]
        facts_changes(before, after, name, out)
        input_changes(resolve(schemas_old, before['input']), resolve(schemas_new, after['input']), f'tool {name} input', out)
        if 'output' in before:
            if 'output' not in after:
                out.append(('major', f'tool {name}: output contract removed'))
            else:
                output_changes(resolve(schemas_old, before['output']), resolve(schemas_new, after['output']), f'tool {name} output', out)
        elif 'output' in after:
            out.append(('minor', f'tool {name}: output contract declared'))
    for schema in ('Problem', 'ErrorEnvelope'):
        output_changes(schemas_old.get(schema), schemas_new.get(schema), schema, out)
    if not out and contracts.contract_digest(old) != contracts.contract_digest(new):
        out.append(('patch', 'descriptions or source locations changed'))
    level = max((level for level, _ in out), key=LEVELS.get, default=None)
    return level, [reason for _, reason in out]


def facts_changes(before, after, name, out):
    """Typed operation facts: a newly required permission or approval breaks a caller; other fact changes do not."""
    old, new = before.get('facts'), after.get('facts')
    if old is None or new is None:
        if new is not None:
            out.append(('minor', f'tool {name}: typed facts declared'))
        return
    added = sorted(set(new.get('permissions', [])) - set(old.get('permissions', [])))
    if added:
        out.append(('major', f'tool {name}: newly requires permission {", ".join(added)}'))
    elif set(old.get('permissions', [])) != set(new.get('permissions', [])):
        out.append(('minor', f'tool {name}: requires fewer permissions'))
    if after.get('requiresApproval') and not before.get('requiresApproval'):
        out.append(('major', f'tool {name}: newly requires approval'))
    elif old != new or before.get('requiresApproval') != after.get('requiresApproval'):
        out.append(('patch', f'tool {name}: facts changed'))


# Git helpers -----------------------------------------------------------------------------
def git(*args, check=True):
    result = subprocess.run(['git', '-C', str(ROOT), *args], capture_output=True, text=True)
    if check and result.returncode:
        raise SystemExit(result.stderr.strip())
    return result


def contract_at(commit, name):
    """The committed contract of a component at a commit, or None when it did not exist yet."""
    if not commit:
        return None
    result = git('show', f'{commit}:contracts/http/{name}', check=False)
    return json.loads(result.stdout) if result.returncode == 0 else None


def commits(since, until, paths):
    """`<short hash> <subject>` of non-merge commits touching the paths, newest first."""
    if not paths:
        return []
    revision = f'{since}..{until}' if since else until
    result = git('log', '--no-merges', '--format=%h %s', revision, '--', *paths, check=False)
    return [line for line in result.stdout.splitlines() if line.strip()]


def files_digest(paths):
    """SHA-256 over the sorted (path, content hash) list of every file under the paths."""
    lines = []
    for path in paths:
        root = ROOT / path
        files = [root] if root.is_file() else sorted(p for p in root.rglob('*') if p.is_file()) if root.exists() else []
        for file in files:
            parts = file.relative_to(ROOT).parts
            if {'target', 'node_modules', '__pycache__'} & set(parts):
                continue
            lines.append(f'{file.relative_to(ROOT).as_posix()} {hashlib.sha256(file.read_bytes()).hexdigest()}')
    return hashlib.sha256('\n'.join(sorted(lines)).encode()).hexdigest()


# Release planning ------------------------------------------------------------------------
def component_paths():
    """Component -> the monorepo paths whose commits make up its changelog and source digest."""
    data = json.loads((ROOT / 'components.json').read_text())['components']
    paths = {name: list(row['sources']) for name, row in data.items()}
    paths['connectors'] = paths['connectors'] or ['extensions/services/connectors', 'extensions/services/connector-runtime']
    return paths


def plan(release, commit):
    """Per component and for the SDK: previous and next version, bump level and the reasons."""
    docs = contracts.build()
    versions = release.get('versions', {})
    paths = component_paths()
    rows = {}
    for name in contracts.component_names():
        doc = docs[f'{name}.openapi.json']
        previous = versions.get(name)
        source = files_digest(paths[name])
        digest = contracts.contract_digest(doc)
        if previous is None:
            level, reasons, version = 'initial', ['first versioned release'], INITIAL
        else:
            level, reasons = (None, []) if digest == previous['contract'] else compare(contract_at(previous['commit'], f'{name}.openapi.json'), doc)
            if level is None and source != previous.get('source'):
                level, reasons = 'patch', ['component sources changed']
            version = bump(previous['version'], level)
        rows[name] = {'previous': previous and previous['version'], 'version': version, 'level': level, 'reasons': reasons,
                      'contract': digest, 'source': source, 'paths': paths[name] + [f'contracts/http/{name}.openapi.json'],
                      'since': previous and previous['commit']}
    combined = docs['veyndo.openapi.json']
    previous = release.get('sdk')
    digest, source = contracts.contract_digest(combined), files_digest(SDK_SOURCES)
    if previous is None:
        level, reasons, version = 'initial', ['first versioned release'], INITIAL
    else:
        level, reasons = (None, []) if digest == previous['contract'] else compare(contract_at(previous['commit'], 'veyndo.openapi.json'), combined)
        if level is None and source != previous.get('source'):
            level, reasons = 'patch', ['SDK generators or hand-written SDK files changed']
        version = bump(previous['version'], level)
    rows['sdk'] = {'previous': previous and previous['version'], 'version': version, 'level': level, 'reasons': reasons,
                   'contract': digest, 'source': source, 'paths': SDK_PATHS, 'since': previous and previous['commit']}
    for row in rows.values():
        row['commits'] = commits(row['since'], commit, row['paths'])
    return rows


def changelog_section(name, row, commit, date):
    lines = [f'## {row["version"]} ({date}, monorepo `{commit[:12]}`)', '']
    if row['level'] == 'initial':
        lines.append('First versioned release of this contract. Earlier history is in the monorepo.')
    else:
        lines.append(f'{row["level"].capitalize()} release.')
    if row['reasons'] and row['level'] != 'initial':
        lines += ['', '### Contract', ''] + [f'- {reason}' for reason in row['reasons'][:40]]
        if len(row['reasons']) > 40:
            lines.append(f'- … and {len(row["reasons"]) - 40} more contract differences')
    shown = row['commits'][:60]
    if shown:
        lines += ['', '### Commits', ''] + [f'- {line}' for line in shown]
        if len(row['commits']) > 60:
            lines.append(f'- … and {len(row["commits"]) - 60} older commits')
    return '\n'.join(lines) + '\n'


def write_changelogs(rows, commit, date):
    """Prepend the new version's section to contracts/changelogs/<component>.md for every changed component."""
    CHANGELOGS.mkdir(parents=True, exist_ok=True)
    for name, row in rows.items():
        if row['level'] is None:
            continue
        path = CHANGELOGS / f'{name}.md'
        title = f'# Changelog: {"generated SDKs" if name == "sdk" else name}\n\nGenerated by `scripts/component_mirrors.py --record`; one section per recorded version.\n\n'
        existing = path.read_text().split('\n\n', 2)[2] if path.exists() else ''
        path.write_text(title + changelog_section(name, row, commit, date) + ('\n' + existing if existing else ''))


def apply(release, rows, commit):
    """Write the planned versions into the release record."""
    versions = release.setdefault('versions', {})
    for name, row in rows.items():
        entry = {'version': row['version'], 'contract': row['contract'], 'source': row['source'], 'commit': commit}
        if name == 'sdk':
            release['sdk'] = {**entry, 'packages': {'npm': '@veyndo/commerce-client', 'pypi': 'veyndo-commerce-client', 'crates.io': 'veyndo-sdk'}}
        elif row['level'] is not None or name not in versions:
            versions[name] = entry
    release['versions'] = dict(sorted(versions.items()))


def verify(release):
    """Structural problems of the version part of the release record."""
    problems = []
    versions = release.get('versions')
    if versions is None:
        return ['components.release.json has no versions; run --record']
    if set(versions) != set(contracts.component_names()):
        problems.append(f'versions list {sorted(versions)}, components are {sorted(contracts.component_names())}')
    for name, row in {**versions, 'sdk': release.get('sdk') or {}}.items():
        if not is_semver(row.get('version')):
            problems.append(f'{name}: semantic version required')
        if not re.fullmatch('[0-9a-f]{64}', row.get('contract', '')) or not re.fullmatch('[0-9a-f]{40}', row.get('commit', '')):
            problems.append(f'{name}: contract digest and exact commit required')
    return problems


def print_plan(rows):
    for name, row in rows.items():
        change = f'{row["previous"]} -> {row["version"]}' if row['previous'] else f'new {row["version"]}'
        level = row['level'] or 'unchanged'
        print(f'{name:13} {change:18} {level:8} {len(row["commits"]):4} commits  {"; ".join(row["reasons"][:2])}')
