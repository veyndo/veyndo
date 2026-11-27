#!/usr/bin/env python3
"""Generate the component contracts and the TypeScript, Python and Rust SDKs from them; `--check` fails on any drift.

Development tooling only (root AGENTS.md: Python never serves a commerce request).

    python3 scripts/sdks/generate.py            write contracts/http and the generated SDK files
    python3 scripts/sdks/generate.py --check    fail when a committed file differs from its generator

Generated: `contracts/http/*.openapi.json`, `sdks/*/src` (TypeScript, Rust), `sdks/python/veyndo_sdk`
and the package manifests. Hand-written and checked elsewhere: `sdks/*/examples`, `sdks/*/tests`
and the READMEs. See docs/sdks.md.
"""
import argparse
from pathlib import Path
import subprocess
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
import contracts  # noqa: E402  (also exposes ROOT)
import model  # noqa: E402
import python  # noqa: E402
import rust  # noqa: E402
import typescript  # noqa: E402

ROOT = contracts.ROOT
# Directories whose every file is generated: a file here that no generator writes is stale.
OWNED = ['sdks/typescript/src', 'sdks/python/veyndo_sdk', 'sdks/rust/src/components']


def rustfmt(files):
    """Format generated Rust exactly as `cargo fmt` would, so the workspace format check agrees."""
    for name, text in files.items():
        if name.endswith('.rs'):
            files[name] = subprocess.run(['rustfmt', '--edition', '2024', '--emit', 'stdout', '--quiet'],
                                         input=text, capture_output=True, text=True, check=True, cwd=ROOT).stdout
    return files


def sdk_outputs():
    contract = model.load()
    files = {}
    files.update(typescript.render(contract))
    files.update(python.render(contract))
    files.update(rustfmt(rust.render(contract)))
    return files


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    if args.check:
        subprocess.run([sys.executable, str(Path(__file__).with_name('contracts.py')), '--check'], check=True)
    else:
        subprocess.run([sys.executable, str(Path(__file__).with_name('contracts.py'))], check=True)
    files = sdk_outputs()
    drift = [name for name, text in files.items() if not (ROOT / name).exists() or (ROOT / name).read_text() != text]
    stale = sorted(path.relative_to(ROOT).as_posix() for folder in OWNED for path in (ROOT / folder).rglob('*')
                   if path.is_file() and '__pycache__' not in path.parts and path.relative_to(ROOT).as_posix() not in files)
    if args.check:
        if drift or stale:
            sys.exit('SDK drift: ' + ', '.join(drift + [s + ' (stale)' for s in stale]) + ' (run python3 scripts/sdks/generate.py)')
    else:
        for name in drift:
            (ROOT / name).parent.mkdir(parents=True, exist_ok=True)
            (ROOT / name).write_text(files[name])
        for name in stale:
            (ROOT / name).unlink()
    languages = {name.split('/')[1] for name in files}
    print(f'PASS SDKs: {len(files)} generated files for {", ".join(sorted(languages))}' + ('' if args.check else f' ({len(drift)} written, {len(stale)} removed)'))


if __name__ == '__main__':
    main()
