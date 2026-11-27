"""Tooling suite: semantic version bumps follow contract changes (negative controls), and the publish only ever plans."""
import copy
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts'))
import component_releases as releases  # noqa: E402

BASE = json.loads((ROOT / 'contracts/http/catalog.openapi.json').read_text())
TOOL = 'catalog.search'
SCHEMA = 'CatalogSearchInput'


def level(change):
    new = copy.deepcopy(BASE)
    change(new)
    return releases.compare(BASE, new)[0]


def schema(doc):
    return doc['components']['schemas'][SCHEMA]


def first_route(doc):
    return next(iter(doc['paths']))


def remove_route(doc):
    del doc['paths'][first_route(doc)]


def add_route(doc):
    doc['paths']['/api/merchant/brand-new'] = {'get': copy.deepcopy(next(iter(doc['paths'].values())))[next(iter(next(iter(doc['paths'].values()))))]}


def change_permission(doc):
    path = next(p for p, item in doc['paths'].items() for op in item.values() if op.get('x-veyndo-permission'))
    for op in doc['paths'][path].values():
        op['x-veyndo-permission'] = 'catalog.write'


def require_new(doc):
    schema(doc)['properties']['channel'] = {'type': 'string'}
    schema(doc)['required'] = ['channel']


def optional_new(doc):
    schema(doc)['properties']['channel'] = {'type': 'string'}


def require_existing(doc):
    schema(doc)['required'] = ['query']


def narrow_limit(doc):
    schema(doc)['properties']['query']['maxLength'] = 20


def widen_limit(doc):
    schema(doc)['properties']['query']['maxLength'] = 400


def narrow_type(doc):
    schema(doc)['properties']['limit']['type'] = 'string'


def remove_tool(doc):
    doc['x-veyndo-tools'] = [t for t in doc['x-veyndo-tools'] if t['name'] != TOOL]


def describe(doc):
    doc['info']['description'] += ' More words.'


def remove_enum_value(doc):
    for name, value in doc['components']['schemas'].items():
        if not name.endswith('Input'):
            continue
        for prop in value.get('properties', {}).values():
            if 'enum' in prop and len(prop['enum']) > 1:
                prop['enum'] = prop['enum'][1:]
                return
    raise AssertionError('no enum in the catalog contract to narrow')


CASES = [
    (remove_route, 'major'), (change_permission, 'major'), (remove_tool, 'major'), (require_new, 'major'),
    (require_existing, 'major'), (narrow_limit, 'major'), (narrow_type, 'major'), (remove_enum_value, 'major'),
    (add_route, 'minor'), (optional_new, 'minor'), (widen_limit, 'minor'),
    (describe, 'patch'),
]
for change, expected in CASES:
    found = level(change)
    assert found == expected, (change.__name__, found, expected)
assert releases.compare(BASE, copy.deepcopy(BASE)) == (None, [])
assert releases.compare(None, BASE)[0] == 'minor'

# Declared outputs may grow but not shrink.
output = {'type': 'object', 'required': ['valid'], 'properties': {'valid': {'type': 'boolean'}, 'errors': {'type': 'array'}}}
out = []
releases.output_changes(output, {**output, 'properties': {'valid': {'type': 'boolean'}}}, 'o', out)
assert [lvl for lvl, _ in out] == ['major'], out
out = []
releases.output_changes(output, {**output, 'properties': {**output['properties'], 'more': {'type': 'string'}}}, 'o', out)
assert [lvl for lvl, _ in out] == ['minor'], out

assert [releases.bump('1.4.2', lvl) for lvl in ('major', 'minor', 'patch', None)] == ['2.0.0', '1.5.0', '1.4.3', '1.4.2']
assert releases.is_semver('1.0.0') and not releases.is_semver('1.0') and not releases.is_semver('01.0.0')

# Unchanged contract and sources keep the version; changed sources alone make a patch.
head = subprocess.check_output(['git', '-C', str(ROOT), 'rev-parse', 'HEAD'], text=True).strip()
recorded = json.loads((ROOT / 'components.release.json').read_text())
if recorded.get('versions'):
    rows = releases.plan(recorded, head)
    same = {**recorded, 'versions': {n: {**r, 'contract': rows[n]['contract'], 'source': rows[n]['source']} for n, r in recorded['versions'].items()}}
    assert all(row['level'] is None for name, row in releases.plan(same, head).items() if name != 'sdk')
    stale = copy.deepcopy(same)
    stale['versions']['catalog']['source'] = '0' * 64
    planned = releases.plan(stale, head)['catalog']
    assert planned['level'] == 'patch' and planned['version'] == releases.bump(recorded['versions']['catalog']['version'], 'patch')

# Publishing never happens from here: without --dry-run the command refuses before generating anything.
refused = subprocess.run([sys.executable, 'scripts/component_mirrors.py', '--publish'], cwd=ROOT, capture_output=True, text=True)
assert refused.returncode != 0 and 'not enabled' in refused.stderr, refused.stderr
print(f'PASS component releases: {len(CASES)} contract changes classified, output growth rules, version arithmetic, patch-on-source and publish refusal')
