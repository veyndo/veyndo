#!/usr/bin/env python3
"""Differential driver: every proved rule, compiled Rust against its Lean version, with a negative control.

The compiled production policies (`target/debug/verified_kernel`, built by
`cargo build --workspace --bins`) and the executable built from the extracted
Lean model (`proof/.lake/build/bin/conformance`) answer the same generated
inputs: every Boolean assignment crossed with numeric edges up to u64::MAX plus
seeded random cases. Every rule registered in `proof/manifest.json` must be in
that population and every answer must agree.

Negative control: the comparison must catch an altered rule. For each control,
one extracted rule is changed in a temporary copy of the Lean model, the Lean
runner is interpreted against it, and the comparison has to report a mismatch
on that rule. A driver that accepted the altered rule would fail this suite.
This is bridge evidence between the compiled Rust and the Lean model; the
universal statements are the kernel-checked theorems.
"""
import json
import os
import pathlib
import shutil
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts/formal'))
from conformance import cases, evaluate, mismatches  # noqa: E402
from extract import Parser, generated, kernel_source  # noqa: E402
from mutations import mutated  # noqa: E402
from runners import lean_runner  # noqa: E402

# One altered rule per control: a dropped guard, a weakened bound, a flipped decision, a wrong
# product and a truncating instead of rounding quotient.
CONTROLS = [
    ('reservation_admissible', 'requested <= ordered.saturating_sub(held)', 'requested <= ordered'),
    ('order_transition_admissible', '&& !source_terminal', '&& true'),
    ('payment_transition_admissible', 'next > current', 'true'),
    ('stock_admissible', 'quantity <= stock', 'quantity >= stock'),
    ('discount_cap', 'total.min(requested)', 'requested'),
    ('line_total', 'unit.saturating_mul(quantity)', 'unit.saturating_add(quantity)'),
    ('rounded_quotient', 'denominator.max(1) / 2', '0'),
]


def lean_tool(*args):
    """A tool of the pinned Lean toolchain, through elan from PATH or its default install."""
    elan = shutil.which('elan') or str(pathlib.Path.home() / '.elan/bin/elan')
    toolchain = (ROOT / 'proof/lean-toolchain').read_text().strip()
    return [elan, 'run', toolchain, *args]


def comparison(functions):
    """Compiled Rust and the built Lean runner on the complete generated population."""
    rust_bin = ROOT / 'target/debug/verified_kernel'
    if not rust_bin.exists():
        raise SystemExit('Build the binaries first: cargo build --workspace --bins')
    subprocess.run(lean_tool('lake', 'build', 'conformance'), cwd=ROOT / 'proof', check=True,
                   capture_output=True, text=True)
    population = cases(functions)
    rust = evaluate([str(rust_bin)], population)
    lean = evaluate([str(ROOT / 'proof/.lake/build/bin/conformance')], population)
    wrong = mismatches(population, rust, lean)
    if wrong:
        raise SystemExit(f'Rust/Lean mismatch: {wrong[0]}')
    per_rule = {}
    for case in population:
        per_rule[case['function']] = per_rule.get(case['function'], 0) + 1
    return population, rust, per_rule


def negative_control(source, population, rust):
    """Each altered Lean rule must disagree with the compiled Rust on its own cases."""
    caught = {}
    with tempfile.TemporaryDirectory(prefix='formal-differential-') as temp:
        path = pathlib.Path(temp)
        (path / 'Commerce').mkdir()
        env = {**os.environ, 'LEAN_PATH': str(path)}
        for name, old, new in CONTROLS:
            altered = Parser(mutated(source, name, old, new)).parse()
            (path / 'Commerce/Generated.lean').write_text(generated(altered))
            (path / 'Main.lean').write_text(lean_runner(altered))
            subprocess.run(lean_tool('lean', 'Commerce/Generated.lean', '-o', 'Commerce/Generated.olean'),
                           cwd=path, env=env, check=True, capture_output=True, text=True)
            selected = [(c, r) for c, r in zip(population, rust) if c['function'] == name]
            answers = evaluate(lean_tool('lean', '--run', 'Main.lean'), [c for c, _ in selected],
                               cwd=path, env=env, timeout=600)
            wrong = mismatches([c for c, _ in selected], [r for _, r in selected], answers)
            if not wrong:
                raise SystemExit(f'Negative control not caught: {name}: {old} -> {new}')
            caught[name] = len(wrong)
    return caught


def main():
    manifest = json.loads((ROOT / 'proof/manifest.json').read_text())
    source = kernel_source(ROOT)
    functions = Parser(source).parse()
    names = {f.name for f in functions}
    if names != set(manifest['policies']):
        raise SystemExit('Proved rules and extracted rules differ: ' + str(names ^ set(manifest['policies'])))
    population, rust, per_rule = comparison(functions)
    if set(per_rule) != names:
        raise SystemExit('Rule without generated inputs: ' + str(names - set(per_rule)))
    caught = negative_control(source, population, rust)
    report = {'rules': len(names), 'cases': len(population), 'mismatches': 0,
              'minimumCasesPerRule': min(per_rule.values()),
              'negativeControlsCaught': caught}
    out = ROOT / 'artifacts/formal-differential.json'
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report, indent=2))
    print(f'PASS formal differential: {len(names)} rules, {len(population)} cases, '
          f'{len(caught)} altered rules caught')


if __name__ == '__main__':
    main()
