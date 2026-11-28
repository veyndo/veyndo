#!/usr/bin/env python3
"""Keep the developer templates (templates/) consistent: index, template descriptors, shared files and their tests.

Development tooling only. `veyndo app new <id>` (the CLI) scaffolds exactly the folders listed in
templates/index.json. Every app template carries byte-identical copies of templates/shared/ (the sandbox
deploy script and the generic Manifest test); `--write` refreshes them, the default check fails on drift.

    python3 scripts/templates.py            check index, descriptors and shared copies (fast, no Node)
    python3 scripts/templates.py --write    copy templates/shared/ into every app template
    python3 scripts/templates.py --test     also run each template's own `node --test`
"""
import argparse
import json
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
TEMPLATES = ROOT / 'templates'
SHARED = TEMPLATES / 'shared'
LANGUAGES = {'en', 'de', 'es', 'fr'}
KINDS = {'app', 'storefront'}


def pointer(document, path):
    value = document
    for part in path.strip('/').split('/'):
        value = value[part]
    return value


def problems(write=False):
    errors = []
    index = json.loads((TEMPLATES / 'index.json').read_text())
    listed = [row['id'] for row in index['templates']]
    folders = sorted(p.name for p in TEMPLATES.iterdir() if p.is_dir() and p != SHARED)
    if sorted(listed) != folders or len(listed) != len(set(listed)):
        errors.append(f'templates/index.json lists {sorted(listed)}, folders are {folders}')
    shared = sorted(p.relative_to(SHARED) for p in SHARED.rglob('*') if p.is_file())
    for row in index['templates']:
        folder = TEMPLATES / row['id']
        if not (folder / 'template.json').is_file():
            errors.append(f'{row["id"]}: missing template.json')
            continue
        descriptor = json.loads((folder / 'template.json').read_text())
        if descriptor['id'] != row['id'] or descriptor['kind'] != row['kind'] or row['kind'] not in KINDS:
            errors.append(f'{row["id"]}: index and template.json disagree on id or kind')
        if row['path'] != f'templates/{row["id"]}' or descriptor['title'] != row['title'] or descriptor['summary'] != row['summary']:
            errors.append(f'{row["id"]}: index path, title or summary differs from template.json')
        for key in ('title', 'summary'):
            if set(row[key]) != LANGUAGES or not all(row[key].values()):
                errors.append(f'{row["id"]}: {key} needs en, de, es and fr')
        for name in ('README.md', 'package.json'):
            if not (folder / name).is_file():
                errors.append(f'{row["id"]}: missing {name}')
        if not list((folder / 'tests').glob('*.test.mjs')):
            errors.append(f'{row["id"]}: needs tests/*.test.mjs')
        package = json.loads((folder / 'package.json').read_text())
        if package.get('name') != row['id'] or package.get('scripts', {}).get('test') != 'node --test':
            errors.append(f'{row["id"]}: package.json name/test script')
        for rename in descriptor['rename']:
            target = json.loads((folder / rename['file']).read_text())
            expected = row['id'] if rename.get('value') == 'kebab' else descriptor['appId']
            if pointer(target, rename['pointer']) != expected:
                errors.append(f'{row["id"]}: {rename["file"]}{rename["pointer"]} is not {expected}')
        if row['kind'] != 'app':
            continue
        manifest = json.loads((folder / 'manifest.json').read_text())
        if not re.fullmatch(r'[a-z][a-z0-9_]{2,40}', descriptor['appId'] or '') or manifest['id'] != descriptor['appId']:
            errors.append(f'{row["id"]}: appId must match manifest id')
        for name in shared:
            source, copy = SHARED / name, folder / name
            if not copy.is_file() or copy.read_bytes() != source.read_bytes():
                if write:
                    copy.parent.mkdir(parents=True, exist_ok=True)
                    copy.write_bytes(source.read_bytes())
                else:
                    errors.append(f'{row["id"]}: {name} differs from templates/shared (run scripts/templates.py --write)')
    return index, errors


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--write', action='store_true')
    parser.add_argument('--test', action='store_true')
    args = parser.parse_args()
    index, errors = problems(args.write)
    if errors:
        sys.exit('Template drift:\n- ' + '\n- '.join(errors))
    if args.test:
        for row in index['templates']:
            subprocess.run(['node', '--test'], cwd=TEMPLATES / row['id'], check=True)
    print(f'PASS {len(index["templates"])} templates: index, descriptors, languages and shared files'
          + (', tests' if args.test else ''))


if __name__ == '__main__':
    main()
