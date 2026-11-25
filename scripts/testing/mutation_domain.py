#!/usr/bin/env python3
"""On-demand mutation testing of the pure `domain/` modules: a surviving mutant is a missing test.

Runs `cargo-mutants` (installed into target/tools: `cargo install --locked --root target/tools
cargo-mutants`) for each crate's `src/domain/**` against that crate's own tests, in parallel
copies of the tree, one crate at a time. Every surviving (missed) mutant must be
listed with a reason in `scripts/testing/mutation-survivors.json`; an unlisted survivor fails,
and so does a listed one that no longer survives (the list only shrinks with tests).
Too slow for every run (hours for all crates); run it like the coverage audit:

    python3 scripts/testing/mutation_domain.py [--crates pricing inventory ...] [--check-only]
"""
import argparse
import collections
import json
import os
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CRATES = ['pricing', 'inventory', 'checkout', 'payments', 'orders']
SURVIVORS = ROOT / 'scripts/testing/mutation-survivors.json'
OUTPUT = ROOT / 'artifacts/mutants'
TOOL = ROOT / 'target/tools/bin/cargo-mutants'


def key(line):
    """A mutant without its line and column, so unrelated edits above it do not rename it."""
    return re.sub(r':\d+:\d+:', ':', line.strip())


def run(crate, jobs):
    out = OUTPUT / crate
    # Each job builds in its own copy of the tree; a shared CARGO_TARGET_DIR would serialize them.
    env = {k: v for k, v in os.environ.items() if k != 'CARGO_TARGET_DIR'}
    command = [str(TOOL), 'mutants', '--jobs', str(jobs), '--no-shuffle', '--package', f'veyndo-{crate}',
               '--file', f'components/{crate}/src/domain/**/*.rs', '--file', f'components/{crate}/src/domain.rs',
               '--exclude', '**/tests.rs', '--exclude', '**/*_tests.rs', '--output', str(out),
               '--timeout-multiplier', '3']
    print('RUN', ' '.join(command), flush=True)
    # cargo-mutants exits non-zero when mutants survive; the survivor list below decides.
    subprocess.run(command, cwd=ROOT, env=env, check=False)


def outcomes(crate):
    base = OUTPUT / crate / 'mutants.out'
    read = lambda name: [l for l in (base / name).read_text().splitlines() if l.strip()] if (base / name).exists() else []
    if not (base / 'outcomes.json').exists():
        raise SystemExit(f'No mutation outcomes for {crate}; run without --check-only')
    return {kind: read(f'{kind}.txt') for kind in ('caught', 'missed', 'timeout', 'unviable')}


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--crates', nargs='+', choices=CRATES, default=CRATES)
    parser.add_argument('--jobs', type=int, default=8, help='Parallel copies of the tree (each builds its own target)')
    parser.add_argument('--check-only', action='store_true', help='Evaluate existing outcomes without running mutants')
    args = parser.parse_args()
    if not args.check_only and not TOOL.exists():
        raise SystemExit(f'Install cargo-mutants first: cargo install --locked --root target/tools cargo-mutants')
    accepted = json.loads(SURVIVORS.read_text())['survivors']
    failures, rows = [], []
    for crate in args.crates:
        if not args.check_only:
            run(crate, args.jobs)
        result = outcomes(crate)
        # Multisets: two mutants of one line differ only in their column.
        listed = collections.Counter(key(k) for k in accepted.get(crate, {}))
        missed = collections.Counter(key(m) for m in result['missed'])
        failures += [f'{crate}: surviving mutant without a test or a listed reason: {m}' for m in sorted((missed - listed).elements())]
        failures += [f'{crate}: listed survivor is now caught or gone; remove it: {m}' for m in sorted((listed - missed).elements())]
        rows.append((crate, *(len(result[k]) for k in ('caught', 'missed', 'timeout', 'unviable'))))
    for crate, caught, missed, timeout, unviable in rows:
        print(f'{crate}: caught {caught}, survived {missed}, timeout {timeout}, unviable {unviable}')
    for failure in failures:
        print('FAIL', failure)
    if failures:
        sys.exit(f'{len(failures)} mutation failures')
    print('PASS mutation testing of domain/: every survivor is listed with a reason')


if __name__ == '__main__':
    main()
