#!/usr/bin/env python3
"""Generate standalone component mirrors from the monorepo, check that each builds and tests alone, and record the released commit.

Development tooling only (root AGENTS.md: Python never serves a commerce request).

A mirror is a sparse copy of the monorepo: the component's crates, every crate they
reach through path dependencies, the files their sources include from outside those
crates, the checked `.sqlx` query cache, `Cargo.lock` and a generated workspace
manifest that names only the component's own crates. Every copied source keeps its
monorepo-relative path and its exact bytes, so path dependencies and `include_str!`
paths stay valid without editing a source file, and a mirror can be compared byte for
byte with the commit it was generated from. The server crate (`src/`) is never part of
a mirror: a component's ports are satisfied in the mirror only by implementations
inside the component itself, normally its test adapters.

    python3 scripts/component_mirrors.py                 generate every mirror under target/mirrors/
    python3 scripts/component_mirrors.py --check         generate, then cargo check + cargo test each mirror alone
    python3 scripts/component_mirrors.py --record        record HEAD as the released commit in components.release.json,
                                                         with new component versions and changelogs
    python3 scripts/component_mirrors.py --versions      show the versions --record would assign now
    python3 scripts/component_mirrors.py --publish --dry-run
                                                         print what would be pushed where; never pushes

See docs/component-mirrors.md for the release flow.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import time
import tomllib

import component_releases as releases

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'target/mirrors'
TARGET = ROOT / 'target/mirror-build'
RELEASE = ROOT / 'components.release.json'
IGNORED = {'target', 'node_modules', '__pycache__', '.git'}
INCLUDE = re.compile(r'include(?:_str|_bytes)?!\s*\(\s*"([^"]+)"')
# Files a test reads at run time from outside its crate (through CARGO_MANIFEST_DIR), which
# no include! names. Each entry is a monorepo path copied into the mirror unchanged.
RUNTIME_DATA = {'veyndo-app-platform': ['extensions/apps/assistant-examples']}
# Components without a crate of their own are not mirrored; the reason is part of the report.
NOT_MIRRORED = {
    'host': 'the server binary itself (src/); it composes the components and is released as the Veyndo image',
    'protocols': 'MCP/UCP/capability adapters still live in the server crate (src/mcp, src/ucp); no crate of their own yet',
    'connectors': 'separate connector service (extensions/services/connectors), released with its own image, not a component crate',
}
# A mirror whose primary crate is imported from its own upstream repository is checked
# here but not published from here: that repository is the source, the monorepo the copy.
VENDORED = {'automation': 'crates/automation is imported from github.com/veyndo/automation at the commit in components.lock.json'}


def components():
    """Component name -> its crate directories (monorepo-relative), from components.json."""
    data = json.loads((ROOT / 'components.json').read_text())['components']
    # The server is released as the image, never as a mirror, even though shared test crates
    # such as `crates/port-contracts` are listed among its sources.
    return {name: [] if name == 'host' else [s for s in row['sources'] if (ROOT / s / 'Cargo.toml').is_file()]
            for name, row in data.items()}


def manifest(crate):
    return tomllib.loads((ROOT / crate / 'Cargo.toml').read_text())


def crate_name(crate):
    return manifest(crate)['package']['name']


def path_dependencies(crate):
    """Monorepo-relative directories of every path dependency (normal, dev, build, per target)."""
    data = manifest(crate)
    tables = [data.get(k, {}) for k in ('dependencies', 'dev-dependencies', 'build-dependencies')]
    for target in data.get('target', {}).values():
        tables += [target.get(k, {}) for k in ('dependencies', 'dev-dependencies', 'build-dependencies')]
    found = []
    for table in tables:
        for spec in table.values():
            if isinstance(spec, dict) and 'path' in spec:
                found.append(rel((ROOT / crate / spec['path']).resolve()))
    return found


def rel(path):
    relative = Path(path).resolve().relative_to(ROOT)
    assert '..' not in relative.parts, f'{path} escapes the monorepo'
    return relative.as_posix()


def closure(crates):
    """The component's crates plus every crate reachable through path dependencies."""
    seen, todo = [], list(crates)
    while todo:
        crate = todo.pop(0)
        if crate in seen:
            continue
        assert crate != '.', 'A component depends on the server crate; it cannot stand alone'
        seen.append(crate)
        todo += path_dependencies(crate)
    return seen


def crate_files(crate):
    folder = ROOT / crate
    for file in sorted(folder.rglob('*')):
        parts = file.relative_to(folder).parts
        if file.is_file() and not set(parts) & IGNORED and not file.is_symlink():
            # A nested crate with a [workspace] of its own is a separate project, not this crate.
            nested = next((folder.joinpath(*parts[:i]) for i in range(1, len(parts))
                           if (folder.joinpath(*parts[:i]) / 'Cargo.toml').is_file()), None)
            if nested and '[workspace]' in (nested / 'Cargo.toml').read_text():
                continue
            yield rel(file)


def outside_files(crates):
    """Files the crates name outside themselves: include!/include_str!/include_bytes!, license-file, readme."""
    inside = lambda path: any(path == c or path.startswith(c + '/') for c in crates)
    extra = set()
    for crate in crates:
        data = manifest(crate)['package']
        for key in ('license-file', 'readme'):
            if isinstance(data.get(key), str):
                extra.add(rel((ROOT / crate / data[key]).resolve()))
        for file in crate_files(crate):
            if file.endswith('.rs'):
                text = (ROOT / file).read_text()
                for target in INCLUDE.findall(text):
                    extra.add(rel((ROOT / file).parent / target))
        for path in RUNTIME_DATA.get(crate_name(crate), []):
            extra.add(path)
    return sorted(p for p in extra if not inside(p))


def plan(name, own):
    """What a mirror of this component contains, before anything is written."""
    crates = closure(own)
    files = [f for c in crates for f in crate_files(c)]
    for extra in outside_files(crates):
        source = ROOT / extra
        assert source.exists(), f'{name}: included file {extra} does not exist'
        files += [rel(f) for f in sorted(source.rglob('*')) if f.is_file()] if source.is_dir() else [extra]
    files += [rel(f) for f in sorted((ROOT / '.sqlx').glob('*.json'))]
    files += ['Cargo.lock', 'LICENSE', 'clippy.toml']
    primary = next((c for c in own if c == f'components/{name}'), own[0])
    return {'crates': own, 'dependencies': [c for c in crates if c not in own],
            'primary': crate_name(primary), 'files': sorted(set(files))}


def workspace_manifest(own):
    root = (ROOT / 'Cargo.toml').read_text()
    shared = root[root.index('[workspace.package]'):root.index('\n[package]')].rstrip()
    members = ', '.join(json.dumps(c) for c in own)
    return ('# Generated by scripts/component_mirrors.py from the Veyndo monorepo; do not edit here.\n'
            '# Only this component\'s crates are members; its dependency crates join as path dependencies.\n'
            f'[workspace]\nresolver = "3"\nmembers = [{members}]\ndefault-members = [{members}]\n\n'
            f'{shared}\n\n[profile.release]\nlto = "thin"\ncodegen-units = 1\n')


MONOREPO = 'https://github.com/veyndo/veyndo'


def intro(name):
    """The first paragraph of the component's own README, its relative links pointed at the monorepo."""
    path = ROOT / 'components' / name / 'README.md'
    if not path.exists():
        return ''
    body = path.read_text().split('\n\n')
    paragraph = next((block for block in body[1:] if block.strip() and not block.lstrip().startswith(('#', '|', '```'))), '')

    def absolute(match):
        target = match.group(2)
        if re.match(r'^(https?:|#|mailto:)', target):
            return match.group(0)
        resolved = os.path.normpath(os.path.join('components', name, target))
        return f'{match.group(1)}({MONOREPO}/blob/main/{resolved})'
    return re.sub(r'(\[[^\]]*\])\(([^)]+)\)', absolute, paragraph.strip())


def readme(name, layout, commit):
    title = json.loads((ROOT / 'components.json').read_text())['components'][name]['title']
    deps = '\n'.join(f'- `{c}` ({crate_name(c)})' for c in layout['dependencies']) or '- none'
    own = '\n'.join(f'- `{c}` ({crate_name(c)})' for c in layout['crates'])
    about = intro(name)
    about = f'\n{about}\n' if about else ''
    return f"""<p align="center">
  <img src="{MONOREPO}/raw/main/docs/brand/veyndo-readme.svg" alt="Veyndo Commerce: commerce that thinks and acts." width="100%">
</p>

# Veyndo {name}: {title[0].lower() + title[1:] if title[1:2].islower() else title}

**One of the 22 components of [Veyndo Commerce]({MONOREPO}), the AI-native commerce
platform built in Rust.** Shoppers, merchants and their agents work on the same
functions; one core decides price, stock and order.
{about}
| | |
| --- | --- |
| **Contract** | [`contracts/http/{name}.openapi.json`]({MONOREPO}/blob/main/contracts/http/{name}.openapi.json): its HTTP routes, permissions and MCP tools |
| **Events** | [Versioned event contracts]({MONOREPO}/tree/main/contracts/events) shared by every component |
| **Clients** | [Generated TypeScript, Python and Rust SDKs]({MONOREPO}/blob/main/docs/sdks.md) |
| **Platform** | [Architecture]({MONOREPO}/blob/main/docs/architecture.md) · [Components]({MONOREPO}/blob/main/docs/commerce-components.md) · [Status]({MONOREPO}/blob/main/docs/status.md) · [Website](https://www.veyndo.ai/) |

## Build and test alone

```bash
cargo check --all-targets --offline
```

```bash
cargo test --offline
```

Database queries compile from the checked `.sqlx` cache (`SQLX_OFFLINE=true` in
`.cargo/config.toml`); no database is needed to build or test. The component's ports
are implemented here by its test adapters; in production the Veyndo server implements
them. A `ui/` folder holds the component's Studio and storefront screens; they build
inside the Veyndo frontend, not on their own.

## Crates of this component

{own}

## Crates it builds against

{deps}

## Where changes go

This repository is a generated mirror of the Veyndo monorepo (commit `{commit}`).
Do not change it here: changes go to [the monorepo]({MONOREPO}), which regenerates this
mirror with `scripts/component_mirrors.py`. Every file keeps its monorepo path and bytes;
`MIRROR.json` lists the source commit. Licensed under the
[Veyndo Sustainable Use License 1.0]({MONOREPO}/blob/main/LICENSE).
"""


def generate(name, own, commit):
    layout = plan(name, own)
    out = OUT / name
    if out.exists():
        shutil.rmtree(out)
    for file in layout['files']:
        target = out / file
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT / file, target)  # keep mtimes, so a shared target dir reuses builds
    (out / 'Cargo.toml').write_text(workspace_manifest(own))
    (out / '.cargo').mkdir(exist_ok=True)
    (out / '.cargo/config.toml').write_text('# Query macros compile from the checked .sqlx cache.\n[env]\nSQLX_OFFLINE = "true"\n')
    (out / 'README.md').write_text(readme(name, layout, commit))
    changelog = releases.CHANGELOGS / f'{name}.md'
    if changelog.exists():
        shutil.copy2(changelog, out / 'CHANGELOG.md')
    layout['digest'] = digest(out)
    version = json.loads(RELEASE.read_text()).get('versions', {}).get(name, {}).get('version') if RELEASE.exists() else None
    (out / 'MIRROR.json').write_text(json.dumps({
        'component': name, 'version': version, 'monorepo': 'https://github.com/veyndo/veyndo', 'commit': commit,
        'crates': layout['crates'], 'dependencies': layout['dependencies'],
        'digest': layout['digest']}, indent=2) + '\n')
    return layout


def digest(out):
    """SHA-256 over the sorted (path, content hash) list of a mirror, MIRROR.json excluded."""
    lines = []
    for file in sorted(out.rglob('*')):
        if file.is_file() and file.name != 'MIRROR.json':
            lines.append(f'{file.relative_to(out).as_posix()} {hashlib.sha256(file.read_bytes()).hexdigest()}')
    return hashlib.sha256('\n'.join(lines).encode()).hexdigest()


def port_traits(crate):
    """Public traits a crate declares in src/ports.rs or src/ports/."""
    files = [ROOT / crate / 'src/ports.rs'] + sorted((ROOT / crate / 'src/ports').glob('**/*.rs'))
    return sorted({t for f in files if f.is_file() for t in re.findall(r'pub trait (\w+)', f.read_text())})


def port_adapters(own):
    """Port trait -> where the mirror implements it ('test' or 'component'), or None when only the server does."""
    sources = [(f, (ROOT / f).read_text()) for c in own for f in crate_files(c) if f.endswith('.rs')]
    result = {}
    for crate in own:
        for trait in port_traits(crate):
            impl = re.compile(rf'impl(?:<[^>]*>)?\s+(?:[\w:]+::)?{trait}(?:<[^>]*>)?\s+for\b')
            kinds = set()
            for file, text in sources:
                for match in impl.finditer(text):
                    test_at = text.find('#[cfg(test)]')
                    in_test = '/tests/' in f'/{file}' or (0 <= test_at < match.start())
                    kinds.add('test' if in_test else 'component')
            result[f'{crate_name(crate)}::{trait}'] = 'test' if 'test' in kinds else ('component' if kinds else None)
    return result


def cargo(out, *args, log):
    env = dict(os.environ, CARGO_TARGET_DIR=str(TARGET), SQLX_OFFLINE='true', CARGO_NET_OFFLINE='true')
    env.pop('DATABASE_URL', None)
    started = time.monotonic()
    with open(log, 'a') as handle:
        handle.write(f'$ cargo {" ".join(args)}\n')
        handle.flush()
        code = subprocess.run(['cargo', *args, '--offline'], cwd=out, env=env,
                              stdout=handle, stderr=subprocess.STDOUT).returncode
    return code, time.monotonic() - started


def head():
    return subprocess.check_output(['git', '-C', str(ROOT), 'rev-parse', 'HEAD'], text=True).strip()


def clean():
    return not subprocess.check_output(['git', '-C', str(ROOT), 'status', '--porcelain'], text=True).strip()


def selected(names):
    all_components = components()
    for name in names or []:
        assert name in all_components, f'Unknown component {name}'
    return {n: c for n, c in all_components.items() if c and (not names or n in names)}


def verify_release(mirrored):
    """The release record names exactly the mirrored components, each with an exact, existing commit."""
    if not RELEASE.exists():
        return ['components.release.json is missing; run --record on a clean checkout']
    release = json.loads(RELEASE.read_text())
    problems = []
    if release.get('version') != 1:
        problems.append('components.release.json: unknown version')
    rows = release.get('components', {})
    if set(rows) != set(mirrored):
        problems.append(f'components.release.json lists {sorted(rows)}, mirrors are {sorted(mirrored)}')
    if set(release.get('not_mirrored', {})) != {n for n, c in components().items() if not c}:
        problems.append('components.release.json: not_mirrored differs from the components without a crate')
    for name, row in rows.items():
        if not re.fullmatch('[0-9a-f]{40}', row.get('commit', '')):
            problems.append(f'{name}: exact commit required')
        elif subprocess.run(['git', '-C', str(ROOT), 'cat-file', '-e', row['commit'] + '^{commit}'],
                            capture_output=True).returncode:
            problems.append(f'{name}: recorded commit {row["commit"]} is not in this repository')
        if not re.fullmatch('[0-9a-f]{64}', row.get('digest', '')):
            problems.append(f'{name}: mirror digest required')
    return problems + releases.verify(release)


def record(chosen, commit):
    release = json.loads(RELEASE.read_text()) if RELEASE.exists() else {'version': 1, 'components': {}}
    # Versions first: the mirrors carry the new version and changelog.
    rows = releases.plan(release, commit)
    date = subprocess.check_output(['git', '-C', str(ROOT), 'show', '-s', '--format=%cs', commit], text=True).strip()
    releases.write_changelogs(rows, commit, date)
    releases.apply(release, rows, commit)
    RELEASE.write_text(json.dumps(release, indent=2) + '\n')
    # Contracts and SDKs state the recorded versions, and the SDK crate's version is in Cargo.lock,
    # which every mirror copies: regenerate both before the mirrors, so they match the release commit
    # that follows this record.
    subprocess.run([sys.executable, str(ROOT / 'scripts/sdks/generate.py')], cwd=ROOT, check=True)
    subprocess.run(['cargo', 'metadata', '--format-version', '1', '--offline'], cwd=ROOT, check=True, stdout=subprocess.DEVNULL)
    release['version'] = 1
    release['generator'] = 'scripts/component_mirrors.py'
    release['not_mirrored'] = {n: NOT_MIRRORED[n] for n, c in components().items() if not c}
    for name, own in chosen.items():
        layout = generate(name, own, commit)
        row = {'repository': 'https://github.com/veyndo/' + layout['primary'].removeprefix('veyndo-'),
               'commit': commit, 'digest': layout['digest'], 'crates': layout['crates'],
               'dependencies': layout['dependencies'], 'published': None}
        if name in VENDORED:
            row['publish'] = False
            row['reason'] = VENDORED[name]
        release['components'][name] = row
    release['components'] = dict(sorted(release['components'].items()))
    RELEASE.write_text(json.dumps(release, indent=2) + '\n')
    releases.print_plan(rows)
    print(f'Recorded {len(chosen)} mirrors and {len(rows) - 1} component versions at {commit} in {RELEASE.relative_to(ROOT)}; '
          'commit components.release.json, contracts/, sdks/ and Cargo.lock')


def check(chosen, commit, keep_going=True):
    logs = OUT / 'logs'
    logs.mkdir(parents=True, exist_ok=True)
    report, failed = {}, []
    for name, own in chosen.items():
        layout = generate(name, own, commit)
        log = logs / f'{name}.log'
        log.write_text('')
        row = {'crates': layout['crates'], 'dependencies': layout['dependencies'], 'files': len(layout['files'])}
        code, took = cargo(OUT / name, 'check', '--all-targets', log=log)
        row['check'] = 'pass' if code == 0 else 'fail'
        if code == 0:
            code, more = cargo(OUT / name, 'test', log=log)
            took += more
            row['test'] = 'pass' if code == 0 else 'fail'
        row['seconds'] = round(took, 1)
        adapters = port_adapters(own)
        row['ports'] = adapters
        row['ports_without_test_adapter'] = [p for p, kind in adapters.items() if kind is None]
        report[name] = row
        status = 'PASS' if row.get('test') == 'pass' else 'FAIL'
        if status == 'FAIL':
            failed.append(name)
        print(f'{status} {name}: check {row["check"]}, test {row.get("test", "not run")} '
              f'({len(layout["dependencies"])} dependency crates, {row["seconds"]}s)', flush=True)
        for port in row['ports_without_test_adapter']:
            print(f'  NEEDS test adapter for port {port} (only the server implements it)', flush=True)
        if status == 'FAIL':
            print(f'  log: {log.relative_to(ROOT)}', flush=True)
            if not keep_going:
                break
    (OUT / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    for name, reason in NOT_MIRRORED.items():
        if name in components() and not components()[name]:
            print(f'NOT MIRRORED {name}: {reason}')
    return failed, report


def publish_plan(commit):
    """What a publish would push where, from the recorded release; nothing is pushed or uploaded."""
    release = json.loads(RELEASE.read_text())
    versions, sdk = release.get('versions', {}), release.get('sdk') or {}
    steps, warnings = [], []
    for name, own in selected(None).items():
        row = release['components'][name]
        version = versions.get(name, {}).get('version')
        if row.get('publish') is False:
            steps.append({'component': name, 'action': 'skip', 'reason': row['reason']})
            continue
        layout = generate(name, own, row['commit'])
        current = layout['digest'] == row['digest']
        if not current:
            warnings.append(f'{name}: the tree generated from this checkout differs from the recorded digest; re-record before publishing')
        steps.append({'component': name, 'action': 'push', 'repository': row['repository'], 'branch': 'main', 'tag': f'v{version}',
                      'version': version, 'source_commit': row['commit'], 'files': len(layout['files']) + 4, 'digest': layout['digest'],
                      'matches_record': current, 'previously_published': row.get('published'),
                      'commands': [f'git -C target/mirrors/{name} init -b main',
                                   f'git -C target/mirrors/{name} add -A',
                                   f'git -C target/mirrors/{name} commit -m "Veyndo {name} {version} from monorepo {row["commit"][:12]}"',
                                   f'git -C target/mirrors/{name} tag v{version}',
                                   f'git -C target/mirrors/{name} push --force-with-lease {row["repository"]} main v{version}']})
    for name, reason in release.get('not_mirrored', {}).items():
        steps.append({'component': name, 'action': 'contract-only', 'version': versions.get(name, {}).get('version'), 'reason': reason})
    sdk_version = sdk.get('version')
    steps.append({'component': 'sdk', 'action': 'packages', 'version': sdk_version, 'commands': [
        f'npm --prefix sdks/typescript run build && npm publish sdks/typescript  # @veyndo/commerce-client@{sdk_version}',
        f'python3 -m build sdks/python && twine upload sdks/python/dist/*  # veyndo-commerce-client=={sdk_version}',
        f'cargo publish -p veyndo-sdk  # veyndo-sdk {sdk_version}']})
    (OUT / 'publish-plan.json').write_text(json.dumps({'commit': commit, 'steps': steps, 'warnings': warnings}, indent=2) + '\n')
    return steps, warnings


def print_publish(steps, warnings):
    for step in steps:
        if step['action'] == 'push':
            state = 'matches record' if step['matches_record'] else 'DIFFERS from record'
            print(f'WOULD PUSH {step["component"]} {step["version"]} -> {step["repository"]} (main, tag {step["tag"]}), '
                  f'{step["files"]} files from monorepo {step["source_commit"][:12]}, {state}')
        elif step['action'] == 'skip':
            print(f'SKIP {step["component"]}: {step["reason"]}')
        elif step['action'] == 'contract-only':
            print(f'CONTRACT ONLY {step["component"]} {step["version"]}: {step["reason"]}')
        else:
            print(f'WOULD PUBLISH SDK {step["version"]}:')
            for command in step['commands']:
                print('  ' + command)
    for warning in warnings:
        print('WARN ' + warning)
    print(f'Dry run only: nothing was pushed or uploaded. Plan: {(OUT / "publish-plan.json").relative_to(ROOT)}')


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--check', action='store_true', help='build and test every generated mirror alone')
    parser.add_argument('--record', action='store_true', help='record HEAD as released commit (clean checkout)')
    parser.add_argument('--component', nargs='+', help='limit to these components')
    parser.add_argument('--fail-fast', action='store_true')
    parser.add_argument('--versions', action='store_true', help='show the versions --record would assign now')
    parser.add_argument('--publish', action='store_true', help='plan the publish of the recorded release (requires --dry-run)')
    parser.add_argument('--dry-run', action='store_true', help='with --publish: print what would be pushed; never pushes')
    args = parser.parse_args()
    chosen = selected(args.component)
    commit = head()
    if args.versions:
        release = json.loads(RELEASE.read_text()) if RELEASE.exists() else {}
        releases.print_plan(releases.plan(release, commit))
        return
    if args.publish:
        # Publishing to component repositories and package registries is a release decision outside
        # this tool; it only plans the push.
        if not args.dry_run:
            sys.exit('Publishing is not enabled here; run with --dry-run to see what would be pushed')
        problems = verify_release(selected(None))
        for problem in problems:
            print('FAIL ' + problem)
        print_publish(*publish_plan(commit))
        sys.exit(1 if problems else 0)
    if args.record:
        assert clean(), 'Record a release only from a clean checkout, so the commit is the source of every mirror'
        record(chosen, commit)
        return
    if not args.check:
        for name, own in chosen.items():
            layout = generate(name, own, commit + ('' if clean() else '+dirty'))
            print(f'Generated {OUT.relative_to(ROOT)}/{name}: {len(layout["files"])} files, '
                  f'{len(layout["dependencies"])} dependency crates')
        problems = verify_release(selected(None))
        problems = [p for p in problems if not args.component or 'lists' not in p]
        for problem in problems:
            print('FAIL ' + problem)
        sys.exit(1 if problems else 0)
    failed, report = check(chosen, commit + ('' if clean() else '+dirty'), keep_going=not args.fail_fast)
    problems = verify_release(selected(None)) if not args.component else []
    for problem in problems:
        print('FAIL ' + problem)
    missing = sum(len(r['ports_without_test_adapter']) for r in report.values())
    print(f'{len(report) - len(failed)}/{len(report)} mirrors build and test alone; '
          f'{missing} ports have no test adapter in their mirror')
    if failed or problems:
        sys.exit('FAILED mirrors: ' + ', '.join(failed) if failed else 'FAILED release record')


if __name__ == '__main__':
    main()
