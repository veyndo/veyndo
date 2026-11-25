#!/usr/bin/env python3
"""Publish separate all-source coverage totals, untested files and enforce reviewed minimums.

The --require-full audit rejects gaps and unmeasured runtime scopes. Normal CI
uses the checked-in regression floors; passing those floors never means 100%.
Each component also has its own line floors (Rust sources and component UI), mapped
through components.json; --ratchet raises them to the measured values, never lowers them.
"""
import argparse
import json
import math
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
POLICY = ROOT / 'scripts/testing/coverage-policy.json'
UNMEASURED = [
    'Browser guest SDK/example-app JavaScript is contract-tested, not included in V8 percentages.',
    'CSS layouts require browser checks; statement coverage cannot measure visual correctness.',
    'Wasm examples have contract tests; no instruction/branch coverage collector is configured.',
    'Real provider credentials, external model behavior and production deployment are not covered by synthetic fixtures.',
]


def type_only(path):
    """True when a formatted TypeScript module declares only types: no top-level runtime statement."""
    text = re.sub(r'/\*.*?\*/', '', path.read_text(), flags=re.S)
    runtime = re.compile(r'^(export\s+(default|const|let|var|function|async|class|enum|\{|\*)|(const|let|var|function|async|class|enum)\s|import\s+(?!type\s))')
    return not any(runtime.match(line) for line in text.splitlines())


def read_frontend(root):
    frontend = json.loads((root / 'frontend/coverage/coverage-summary.json').read_text())
    metrics = {'frontend': {k: frontend['total'][k]['pct'] for k in ('lines', 'branches', 'functions', 'statements')}}
    modules = {}
    for filename, data in frontend.items():
        if filename == 'total':
            continue
        owner = next((prefix for prefix in ('frontend/src/', 'extensions/apps/') if prefix in filename), None)
        # A component's UI folder (components/<name>/ui) is frontend source too.
        component_ui = re.search(r'components/[a-z0-9_-]+/ui/', filename)
        if owner is None and component_ui is None:
            raise ValueError('Coverage source has no frontend/app owner: ' + filename)
        relative = filename[filename.index(owner) if owner else component_ui.start():]
        modules[relative] = {k: data[k]['pct'] for k in ('lines', 'branches', 'functions', 'statements')}
    roots = [root / 'frontend/src', root / 'extensions/apps', *sorted((root / 'components').glob('*/ui'))]
    expected_frontend = {p.relative_to(root).as_posix() for directory in roots for p in directory.rglob('*') if p.suffix in {'.ts', '.tsx'} and 'tests' not in p.relative_to(root).parts}
    # V8 reports no entry for a module outside frontend/ that has no runtime code
    # (only types); such a module has nothing to cover. Every other gap still fails.
    omitted = {name for name in expected_frontend - modules.keys() if not type_only(root / name)}
    if omitted:
        raise ValueError('Coverage report omits source files: ' + ', '.join(sorted(omitted)))
    return metrics, modules, [name for name, data in modules.items() if data['lines'] == 0]



def rust_gaps(root, files):
    """Keep extracted libraries and app-owned Rust visible in unmeasured gaps."""
    emitted = {Path(f['filename']).resolve() for f in files}
    return sorted(p.relative_to(root).as_posix()
                  for directory in ('src', 'crates', 'components', 'extensions')
                  for p in (root / directory).rglob('*.rs')
                  if p.resolve() not in emitted)


def read_reports(root):
    metrics, modules, missing_frontend = read_frontend(root)
    rust = json.loads((root / 'artifacts/coverage/rust.json').read_text())['data'][0]
    python = json.loads((root / 'artifacts/coverage/python.json').read_text())
    metrics.update({
        'rust': {k: rust['totals'][k]['percent'] for k in ('lines', 'functions', 'regions')},
        'python': {'lines': python['totals']['covered_lines'] / max(1, python['totals']['num_statements']) * 100,
                   'branches': python['totals']['covered_branches'] / max(1, python['totals']['num_branches']) * 100},
    })
    expected_python = {p.relative_to(root).as_posix() for directory in ('extensions/services', 'extensions/apps', 'scripts') for p in (root / directory).rglob('*.py') if '__pycache__' not in p.parts}
    omitted = expected_python - python['files'].keys()
    if omitted:
        raise ValueError('Coverage report omits source files: ' + ', '.join(sorted(omitted)))
    missing = {'frontend': missing_frontend,
               'rust': [f['filename'].split('/src/', 1)[-1] for f in rust['files'] if f['summary']['lines']['covered'] == 0],
               'python': [name for name, data in python['files'].items() if data['summary']['covered_lines'] == 0]}
    return metrics, modules, missing


def owners(root):
    """(source prefix, component) pairs from components.json, longest prefix first."""
    mapping = json.loads((root / 'components.json').read_text())['components']
    pairs = [(source.rstrip('/'), name) for name, row in mapping.items() for source in row['sources']]
    return sorted(pairs, key=lambda pair: -len(pair[0]))


def component_of(relative, pairs):
    """The component a repository-relative source file belongs to, or None."""
    ui = re.match(r'components/([a-z0-9_-]+)/ui/', relative)
    if ui:
        return ui.group(1)
    return next((name for prefix, name in pairs if relative == prefix or relative.startswith(prefix + '/')), None)


def relative_to_root(filename, root):
    """A report path relative to the repository: reports may come from another checkout or CI runner."""
    marker = next((m for m in ('/components/', '/crates/', '/src/', '/frontend/', '/extensions/') if m in filename), None)
    if filename.startswith(str(root) + '/'):
        return filename[len(str(root)) + 1:]
    return filename[filename.index(marker) + 1:] if marker else filename


def component_coverage(root, rust_files=(), frontend=None):
    """Line coverage per component and scope: {component: {'rust': pct, 'frontend': pct}}."""
    pairs = owners(root)
    totals = {}
    rows = [('rust', f['filename'], f['summary']['lines']['covered'], f['summary']['lines']['count']) for f in rust_files]
    rows += [('frontend', name, data['lines']['covered'], data['lines']['total'])
             for name, data in (frontend or {}).items() if name != 'total']
    for scope, filename, covered, count in rows:
        component = component_of(relative_to_root(filename, root), pairs)
        if component is None or not count:
            continue
        seen = totals.setdefault(component, {}).setdefault(scope, [0, 0])
        seen[0] += covered
        seen[1] += count
    return {c: {scope: covered / count * 100 for scope, (covered, count) in scopes.items()}
            for c, scopes in totals.items()}


def component_violations(measured, policy, scopes):
    """A measured component without a floor, a floor without a measurement, or a value below its floor."""
    floors = policy.get('components', {})
    errors = []
    for component, values in sorted(measured.items()):
        for scope in scopes:
            if scope in values and scope not in floors.get(component, {}):
                errors.append(f'component {component} {scope} lines: measured {values[scope]:.2f}% but no floor; run --ratchet')
    for component, values in sorted(floors.items()):
        for scope, minimum in values.items():
            if scope not in scopes:
                continue
            value = measured.get(component, {}).get(scope)
            if value is None:
                errors.append(f'component {component} {scope} lines: floor {minimum}% but nothing measured')
            elif value + 0.001 < minimum:
                errors.append(f'component {component} {scope} lines: {value:.2f}% < {minimum}%')
    return errors


def ratchet(measured, policy, scopes):
    """Floors raised to the measured values (rounded down to 0.01); a floor never falls."""
    floors = {c: dict(v) for c, v in policy.get('components', {}).items()}
    for component, values in measured.items():
        for scope in scopes:
            if scope in values:
                value = math.floor(values[scope] * 100) / 100
                current = floors.setdefault(component, {}).get(scope, 0)
                floors[component][scope] = max(current, value)
    return {**policy, 'components': {c: dict(sorted(v.items())) for c, v in sorted(floors.items())}}


def violations(metrics, modules, policy, require_full=False):
    errors = []
    for scope, values in metrics.items():
        for metric, value in values.items():
            minimum = 100 if require_full else policy['minimums'][scope][metric]
            if value + 0.001 < minimum:
                errors.append(f'{scope} {metric}: {value:.2f}% < {minimum}%')
    for path in policy['full_modules']:
        for metric in ('lines', 'branches', 'functions', 'statements'):
            if modules.get(path, {}).get(metric, 0) < 100:
                errors.append(f'{path} {metric}: requires 100%')
    if require_full:
        errors += ['Unmeasured scope: ' + scope for scope in UNMEASURED]
    return errors


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--require-full', action='store_true')
    parser.add_argument('--frontend-only', action='store_true', help='Enforce unchanged frontend floors without claiming Rust/Python measurements')
    parser.add_argument('--ratchet', action='store_true', help='Raise per-component floors to the measured values first; floors never fall')
    args = parser.parse_args()
    if args.frontend_only:
        if args.require_full:
            parser.error('--require-full cannot be combined with --frontend-only')
        metrics, modules, missing = read_frontend(ROOT)
        policy = json.loads(POLICY.read_text())
        summary = json.loads((ROOT / 'frontend/coverage/coverage-summary.json').read_text())
        measured = component_coverage(ROOT, frontend=summary)
        if args.ratchet:
            POLICY.write_text(json.dumps(ratchet(measured, policy, ['frontend']), indent=2) + '\n')
            policy = json.loads(POLICY.read_text())
        errors = violations(metrics, modules, policy) + component_violations(measured, policy, ['frontend'])
        output = ROOT / 'artifacts/coverage'
        output.mkdir(exist_ok=True, parents=True)
        (output / 'frontend-summary.json').write_text(json.dumps({'metrics': metrics,
            'zero_hit_modules': missing, 'errors': errors, 'measuredScopes': ['frontend'],
            'entireSystemFullyCovered': False}, indent=2) + '\n')
        print(json.dumps(metrics, indent=2))
        if errors:
            raise SystemExit('\n'.join(errors))
        print('PASS unchanged frontend coverage floors and full-module gates; Rust/Python not measured')
        return
    metrics, modules, missing = read_reports(ROOT)
    policy = json.loads(POLICY.read_text())
    rust_files = json.loads((ROOT / 'artifacts/coverage/rust.json').read_text())['data'][0]['files']
    measured = component_coverage(ROOT, rust_files, json.loads((ROOT / 'frontend/coverage/coverage-summary.json').read_text()))
    if args.ratchet:
        POLICY.write_text(json.dumps(ratchet(measured, policy, ['rust', 'frontend']), indent=2) + '\n')
        policy = json.loads(POLICY.read_text())
    errors = violations(metrics, modules, policy, args.require_full) + component_violations(measured, policy, ['rust', 'frontend'])
    output = ROOT / 'artifacts/coverage'
    output.mkdir(exist_ok=True)
    result = {'metrics': metrics, 'components': measured, 'zero_hit_modules': missing, 'unmeasured': UNMEASURED,
              'errors': errors, 'entireSystemFullyCovered': False}
    rust_report = json.loads((ROOT / 'artifacts/coverage/rust.json').read_text())['data'][0]
    result['rust_not_emitted_by_compiler'] = rust_gaps(ROOT, rust_report['files'])
    (output / 'summary.json').write_text(json.dumps(result, indent=2) + '\n')
    lines = ['# Measured coverage and open gaps', '',
             'These are separate source scopes, not a combined quality score or proof of bug freedom.', '',
             '| Scope | Metric | Coverage |', '|---|---|---|']
    for scope, values in metrics.items():
        lines += [f'| {scope} | {key} | {value:.2f}% |' for key, value in values.items()]
    lines += ['', '## Line coverage per component', '', '| Component | Rust | Frontend |', '|---|---|---|']
    lines += [f"| {c} | {v.get('rust', float('nan')):.2f}% | {v.get('frontend', float('nan')):.2f}% |" for c, v in sorted(measured.items())]
    lines += ['', '## Unmeasured scopes', ''] + ['- ' + s for s in UNMEASURED]
    lines += ['', '## Rust files without emitted coverage regions', '',
              'These include import registries and test-only modules. Their absence is reported, not treated as a covered module.']
    lines += ['- `' + p + '`' for p in result['rust_not_emitted_by_compiler']]
    for scope, paths in missing.items():
        lines += ['', '## Modules with no recorded hits: ' + scope, '']
        lines += ['- `' + p + '`' for p in paths] or ['None. This does not mean all branches were tested.']
    lines += ['', '## Gate result', ''] + (errors or ['Regression floors passed. Full-system 100% coverage is not achieved.'])
    (output / 'summary.md').write_text('\n'.join(lines) + '\n')
    print(json.dumps(metrics, indent=2))
    if errors:
        raise SystemExit('\n'.join(errors))
    print('PASS reviewed coverage floors and 100% transport/health modules; full-system gaps remain explicit')


if __name__ == '__main__':
    main()
