#!/usr/bin/env python3
"""Versioned outbox event contracts: generation, the compatibility gate and its negative controls.

`contracts/events/<kind>.v<N>.json` are generated from the typed payloads (`veyndo_core::event!`)
by `veyndo --event-contracts`; `contracts/events/published.json` records the signature of every
published version (its properties and required fields).

    python3 scripts/event_contracts.py --write     # regenerate from target/debug/veyndo, then --publish
    python3 scripts/event_contracts.py --publish   # record new versions and additive fields
    python3 scripts/event_contracts.py             # gate (no build needed) plus negative controls

The gate fails when a published version changes incompatibly: a field is removed or renamed,
its type changes, a field becomes required, or a guaranteed (required) field becomes optional.
Such a change needs a new version (`version = N + 1` in `event!`): the old file stays as the
frozen contract and the new one is published next to it. A published kind may only disappear
when a higher version of it is generated. New optional fields and new versions are compatible
but must be recorded with `--publish`. It also refuses components that write to the outbox
without the typed `veyndo_core::events::emit*` functions, and SQL triggers that write kinds the
registry does not declare as `sql-trigger` producers.
"""
import argparse
import json
import re
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CONTRACTS = ROOT / 'contracts/events'
LOCK = 'published.json'
INDEX = 'index.json'
# Vendored code is documented as external in the index and never edited here.
VENDORED = ('extensions/apps/marketplaces/', 'extensions/services/app-storage/', 'crates/automation/')
# The only Rust files that may write outbox rows directly.
WRITERS = {'components/core/src/store/outbox.rs', 'components/core/src/events.rs'}
RAW_WRITE = re.compile(r'outbox::publish|\bpublish_on\s*\(|\bpublish_id_on\s*\(|INSERT INTO outbox\s*\(')
TRIGGER_KIND = re.compile(r"INSERT INTO outbox\s*\(tenant,kind,data\)\s*VALUES\s*\(NEW\.tenant,'([^']+)'")


def signature(schema):
    """What consumers may rely on in one contract file."""
    if 'properties' not in schema:
        return {'appDefined': True}
    return {'properties': schema['properties'], 'required': sorted(schema.get('required', []))}


def split(name):
    """`order.placed.v1.json` -> ('order.placed', 1)."""
    match = re.fullmatch(r'(.+)\.v(\d+)\.json', name)
    if not match:
        raise ValueError(f'not a contract file name: {name}')
    return match.group(1), int(match.group(2))


def breaking(old, new):
    """Every incompatible difference between two signatures of the same version."""
    problems = []
    if old.get('appDefined') or new.get('appDefined'):
        return [] if old == new else ['payload changed between app-defined and typed']
    for field, schema in old['properties'].items():
        if field not in new['properties']:
            problems.append(f'field {field} removed or renamed')
        elif new['properties'][field] != schema:
            problems.append(f'field {field} changed type {json.dumps(schema)} -> {json.dumps(new["properties"][field])}')
    for field in sorted(set(new['required']) - set(old['required'])):
        problems.append(f'field {field} became required')
    for field in sorted(set(old['required']) - set(new['required'])):
        if field in new['properties']:
            problems.append(f'field {field} is no longer guaranteed')
    return problems


def compare(published, current):
    """(breaking problems, unrecorded changes) between the lock and the current contract files."""
    problems, unrecorded = [], []
    current_versions = {}
    for name in current:
        kind, version = split(name)
        current_versions[kind] = max(version, current_versions.get(kind, 0))
    for name, old in sorted(published.items()):
        kind, version = split(name)
        if name not in current:
            if current_versions.get(kind, 0) <= version:
                problems.append(f'{name}: published version removed without a newer version of {kind}')
            continue
        for problem in breaking(old, current[name]):
            problems.append(f'{name}: {problem}; publish it as {kind}.v{version + 1}.json instead')
        if not breaking(old, current[name]) and old != current[name]:
            unrecorded.append(f'{name}: additive change not recorded')
    for name in sorted(set(current) - set(published)):
        unrecorded.append(f'{name}: version not published yet')
    return problems, unrecorded


def read_contracts(folder):
    """The signatures of every contract file in a folder (generated or frozen)."""
    return {path.name: signature(json.loads(path.read_text()))
            for path in sorted(folder.glob('*.v*.json'))}


def raw_writers(root):
    """Rust sources outside the event contract that write outbox rows directly."""
    found = []
    for path in sorted(root.glob('**/*.rs')):
        relative = path.relative_to(root).as_posix()
        if (relative in WRITERS or relative.startswith(VENDORED) or relative.startswith(('target/', 'tests/', 'reference/'))
                or '/tests/' in relative):
            continue
        text = re.sub(r'(?m)^\s*//.*$', '', path.read_text())
        if RAW_WRITE.search(text):
            found.append(relative)
    return found


def undeclared_triggers(root, index):
    """Outbox kinds written by baseline triggers that the registry does not declare as SQL-trigger payloads."""
    declared = {e['kind'] for e in index['events'] if 'sql-trigger' in e.get('sources', [])}
    found = []
    for path in sorted(root.glob('components/*/migrations/*.sql')):
        for kind in TRIGGER_KIND.findall(path.read_text()):
            if kind not in declared:
                found.append(f'{path.relative_to(root).as_posix()}: {kind}')
    return found


def check(root=ROOT):
    folder = root / 'contracts/events'
    current = read_contracts(folder)
    published = json.loads((folder / LOCK).read_text())['versions']
    index = json.loads((folder / INDEX).read_text())
    problems, unrecorded = compare(published, current)
    indexed = {e['file'] for e in index['events']}
    problems += [f'{name}: generated contract missing from index.json' for name in sorted(indexed - set(current))]
    problems += [f'{path}: writes to the outbox without veyndo_core::events::emit*' for path in raw_writers(root)]
    problems += [f'{entry}: trigger kind not declared as a sql-trigger event' for entry in undeclared_triggers(root, index)]
    problems += [f'{entry}; run python3 scripts/event_contracts.py --publish' for entry in unrecorded]
    return problems, len(indexed)


def publish(root=ROOT):
    folder = root / 'contracts/events'
    lock = folder / LOCK
    current = read_contracts(folder)
    published = json.loads(lock.read_text())['versions'] if lock.exists() else {}
    problems, _ = compare(published, current)
    if problems:
        raise SystemExit('Refusing to publish breaking event changes:\n' + '\n'.join(problems))
    versions = {**published, **current}
    lock.write_text(json.dumps({
        'description': 'Published event contract signatures; written by scripts/event_contracts.py --publish. '
                       'A breaking change to a listed version fails the gate; add a new version instead.',
        'versions': dict(sorted(versions.items())),
    }, indent=2) + '\n')
    return len(versions)


def write(root=ROOT):
    files = json.loads(subprocess.check_output([str(root / 'target/debug/veyndo'), '--event-contracts'], cwd=root))
    folder = root / 'contracts/events'
    folder.mkdir(parents=True, exist_ok=True)
    published = json.loads((folder / LOCK).read_text())['versions'] if (folder / LOCK).exists() else {}
    for path in folder.glob('*.v*.json'):
        # Frozen published versions stay; unpublished leftovers of the generator go.
        if path.name not in files and path.name not in published:
            path.unlink()
    for name, content in files.items():
        (folder / name).write_text(json.dumps(content, indent=2, sort_keys=True) + '\n')
    return len(files) - 1


def negative_controls():
    """The gate refuses each kind of breaking change and accepts the compatible ones."""
    base = {'properties': {'orderId': {'type': 'string'}, 'totalMinor': {'type': 'integer'}},
            'required': ['orderId', 'totalMinor']}
    published = {'order.placed.v1.json': base}

    def variant(**change):
        value = json.loads(json.dumps(base))
        for key, update in change.items():
            update(value) if callable(update) else value.__setitem__(key, update)
        return {'order.placed.v1.json': value}

    refused = {
        'removed field': variant(drop=lambda v: (v['properties'].pop('totalMinor'), v['required'].remove('totalMinor'))),
        'renamed field': variant(rename=lambda v: (v['properties'].__setitem__('total', v['properties'].pop('totalMinor')),
                                                   v.__setitem__('required', ['orderId', 'total']))),
        'type change': variant(retype=lambda v: v['properties'].__setitem__('totalMinor', {'type': 'string'})),
        'required added': variant(add=lambda v: (v['properties'].__setitem__('appId', {'type': 'string'}),
                                                 v.__setitem__('required', ['appId', 'orderId', 'totalMinor']))),
        'guarantee dropped': variant(optional=lambda v: v['required'].remove('totalMinor')),
        'version removed': {},
        'typed to app-defined': {'order.placed.v1.json': {'appDefined': True}},
    }
    for label, current in refused.items():
        problems, _ = compare(published, current)
        assert problems, f'gate accepted a breaking change: {label}'
    accepted = {
        'optional field added': variant(add=lambda v: v['properties'].__setitem__('appId', {'type': 'string'})),
        'new version next to the frozen one': {**published, 'order.placed.v2.json': {'properties': {}, 'required': []}},
        'old version retired by a newer one': {'order.placed.v2.json': {'properties': {}, 'required': []}},
    }
    for label, current in accepted.items():
        problems, _ = compare(published, current)
        assert not problems, f'gate refused a compatible change ({label}): {problems}'
    _, unrecorded = compare(published, accepted['optional field added'])
    assert unrecorded, 'an additive change must be recorded with --publish'
    with tempfile.TemporaryDirectory() as folder:
        root = Path(folder)
        (root / 'components/shop/src/store').mkdir(parents=True)
        (root / 'components/shop/migrations').mkdir(parents=True)
        (root / 'components/shop/src/store/bypass.rs').write_text(
            'async fn f() { veyndo_core::store::outbox::publish_on(db, t, "x.y", &data).await; }\n')
        (root / 'components/shop/src/store/typed.rs').write_text(
            '// publish_on( is mentioned in a comment only\nasync fn f() { veyndo_core::events::emit(db, t, &e).await; }\n')
        (root / 'components/shop/migrations/0001-baseline.sql').write_text(
            "INSERT INTO outbox(tenant,kind,data) VALUES(NEW.tenant,'shadow.kind',jsonb_build_object('a',1));\n")
        assert raw_writers(root) == ['components/shop/src/store/bypass.rs'], raw_writers(root)
        index = {'events': [{'kind': 'cache.invalidate', 'sources': ['sql-trigger']}]}
        assert undeclared_triggers(root, index) == ['components/shop/migrations/0001-baseline.sql: shadow.kind']
    return len(refused) + len(accepted) + 2


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--write', action='store_true', help='regenerate contracts from target/debug/veyndo and publish')
    parser.add_argument('--publish', action='store_true', help='record new versions and additive fields')
    args = parser.parse_args()
    if args.write:
        print(f'wrote {write()} event contracts')
    if args.write or args.publish:
        print(f'published {publish()} event contract versions')
    controls = negative_controls()
    problems, count = check()
    if problems:
        sys.exit('FAIL event contracts:\n' + '\n'.join(problems))
    print(f'PASS event contracts: {count} published kind versions compatible, no untyped outbox writers, '
          f'{controls} negative controls')


if __name__ == '__main__':
    main()
