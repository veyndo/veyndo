"""Enforce reviewed Cargo owners and dependency capabilities, including aliases and unnamed helper crates."""
import json
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
OWNERS = {name: ROOT / 'crates' / name.removeprefix('veyndo-') for name in (
    'veyndo-kernel', 'veyndo-flow-graph', 'veyndo-runtime', 'veyndo-apps', 'veyndo-cognition',
    'veyndo-automation')}
# Complete components live under components/<name>; the plan in docs/plans moves the root crate there.
OWNERS.update({'veyndo-core': ROOT / 'components/core', 'veyndo-pricing': ROOT / 'components/pricing',
               'veyndo-identity': ROOT / 'components/identity',
               'veyndo-catalog': ROOT / 'components/catalog',
               'veyndo-inventory': ROOT / 'components/inventory',
               'veyndo-shipping': ROOT / 'components/shipping',
               'veyndo-customers': ROOT / 'components/customers',
               'veyndo-shop': ROOT / 'components/shop',
               'veyndo-marketing': ROOT / 'components/marketing',
               'veyndo-orders': ROOT / 'components/orders',
               'veyndo-payments': ROOT / 'components/payments',
               'veyndo-b2b': ROOT / 'components/b2b',
               'veyndo-pos': ROOT / 'components/pos',
               'veyndo-checkout': ROOT / 'components/checkout',
               'veyndo-intelligence': ROOT / 'components/intelligence',
               'veyndo-studio': ROOT / 'components/studio'})
OWNERS.update({'veyndo': ROOT, 'veyndo-migration': ROOT / 'extensions/apps/migration',
               'veyndo-marketplaces': ROOT / 'extensions/apps/marketplaces',
               'veyndo-app-storage': ROOT / 'extensions/services/app-storage',
               'veyndo-app-model': ROOT / 'extensions/services/app-storage/model',
               'veyndo-app-runtime': ROOT / 'extensions/services/app-storage/runtime',
               'veyndo-app-authority': ROOT / 'extensions/services/app-storage/authority'})
ALLOWED = {
    'veyndo-app-model': set(),
    'veyndo-app-runtime': {'veyndo-app-model'},
    'veyndo-app-storage': {'veyndo-app-model'},
    'veyndo-marketplaces': set(),
    'veyndo-kernel': set(),
    'veyndo-flow-graph': set(),
    'veyndo-automation': {'veyndo-kernel'},
    'veyndo-runtime': {'veyndo-app-authority'},
    'veyndo-app-authority': set(),
    # The optional `service` feature adds the shared error contract; the domain alone needs only the kernel.
    'veyndo-pricing': {'veyndo-kernel', 'veyndo-core', 'veyndo-identity'},
    'veyndo-apps': {'veyndo-kernel', 'veyndo-app-runtime'},
    'veyndo-cognition': {'veyndo-runtime', 'veyndo-kernel', 'veyndo-app-authority', 'veyndo-core'},
    'veyndo-core': {'veyndo-kernel', 'veyndo-runtime'},
    'veyndo-identity': {'veyndo-core', 'veyndo-kernel', 'veyndo-app-model'},
    'veyndo-catalog': {'veyndo-core', 'veyndo-identity', 'veyndo-pricing'},
    'veyndo-inventory': {'veyndo-core', 'veyndo-catalog', 'veyndo-pricing'},
    'veyndo-shipping': {'veyndo-core', 'veyndo-identity', 'veyndo-pricing'},
    'veyndo-customers': {'veyndo-core', 'veyndo-pricing'},
    'veyndo-marketing': {'veyndo-core', 'veyndo-pricing', 'veyndo-catalog', 'veyndo-shop', 'veyndo-app-model', 'veyndo-automation', 'veyndo-cognition', 'veyndo-flow-graph', 'veyndo-identity'},
    'veyndo-orders': {'veyndo-core', 'veyndo-identity', 'veyndo-pricing', 'veyndo-shop', 'veyndo-app-model', 'veyndo-runtime', 'veyndo-shipping'},
    'veyndo-payments': {'veyndo-core', 'veyndo-pricing', 'veyndo-shop', 'veyndo-inventory', 'veyndo-orders', 'veyndo-kernel'},
    'veyndo-b2b': {'veyndo-core', 'veyndo-pricing', 'veyndo-shop', 'veyndo-identity', 'veyndo-kernel', 'veyndo-app-model'},
    'veyndo-pos': {'veyndo-core', 'veyndo-pricing', 'veyndo-shop'},
    'veyndo-checkout': {'veyndo-core', 'veyndo-pricing', 'veyndo-shop', 'veyndo-shipping'},
    'veyndo-intelligence': {'veyndo-core', 'veyndo-pricing', 'veyndo-runtime'},
    'veyndo-studio': {'veyndo-core', 'veyndo-app-model'},
    'veyndo-shop': {'veyndo-core', 'veyndo-pricing', 'veyndo-inventory', 'veyndo-shipping', 'veyndo-app-model', 'veyndo-catalog'},
}
# Reviewed direct external capabilities. Cargo reports actual package names even
# when a manifest renames sqlx as something innocuous. New dependencies need an
# explicit architecture decision, including build/dev/optional/target variants.
EXTERNAL = {
    'veyndo-app-model': {'serde', 'serde_json', 'chrono', 'cron', 'sha2', 'semver', 'url'},
    'veyndo-app-runtime': {'wasmtime', 'wat', 'sha2', 'serde_json'},
    'veyndo-app-storage': {'sqlx', 'serde_json', 'sha2', 'tokio', 'chrono', 'base64', 'ring', 'zeroize'},
    'veyndo-app-authority': {'zeroize', 'hmac', 'sha2', 'uuid', 'sqlx', 'reqwest', 'serde', 'serde_json', 'aes-gcm', 'base64'},
    'veyndo-marketplaces': {'axum', 'tokio', 'serde', 'serde_json', 'sha2', 'hmac', 'ring', 'chrono', 'reqwest'},
    'veyndo-kernel': set(),
    'veyndo-flow-graph': set(),
    'veyndo-automation': {'serde_json', 'chrono', 'chrono-tz'},
    'veyndo-pricing': {'serde', 'serde_json', 'chrono', 'regex', 'reqwest', 'tokio', 'axum', 'sqlx'},
    'veyndo-apps': set(),
    'veyndo-identity': {'serde_json', 'axum', 'argon2', 'hmac', 'ring', 'sha2', 'tokio', 'uuid'},
    'veyndo-catalog': {'serde', 'serde_json', 'sqlx', 'reqwest', 'axum', 'chrono', 'sha2', 'base64'},
    'veyndo-inventory': {'serde', 'serde_json', 'sqlx'},
    'veyndo-shipping': {'serde', 'serde_json', 'sqlx', 'axum'},
    'veyndo-customers': {'serde', 'serde_json', 'sqlx'},
    'veyndo-marketing': {'serde', 'serde_json', 'sqlx'},
    'veyndo-orders': {'serde', 'serde_json', 'sqlx', 'axum', 'chrono', 'reqwest', 'sha2', 'uuid', 'pdf-extract'},
    'veyndo-payments': {'serde', 'serde_json', 'sqlx', 'chrono', 'reqwest', 'hmac', 'sha2'},
    'veyndo-b2b': {'serde', 'serde_json', 'sqlx'},
    'veyndo-pos': {'serde', 'serde_json'},
    'veyndo-checkout': {'serde', 'serde_json', 'chrono', 'sqlx'},
    'veyndo-intelligence': {'serde', 'serde_json', 'sqlx', 'reqwest', 'sha2', 'tokio', 'uuid'},
    'veyndo-studio': {'serde', 'serde_json', 'chrono', 'chrono-tz'},
    'veyndo-shop': {'serde', 'serde_json', 'chrono', 'reqwest', 'sqlx'},
    'veyndo-core': {'axum', 'serde', 'serde_json', 'sha2', 'sqlx', 'uuid', 'tokio', 'tracing', 'reqwest'},
    'veyndo-runtime': {'hmac', 'sha2', 'zeroize', 'sqlx', 'tokio', 'futures-util',
                       'serde_json', 'reqwest', 'async-stream', 'uuid'},
    'veyndo-cognition': {'serde', 'serde_json', 'tokio', 'reqwest', 'aes-gcm',
                         'base64', 'sha2', 'sqlx', 'tracing'},
}


def check(packages):
    names = {p['name'] for p in packages}
    assert names == set(OWNERS), ('Unclassified or missing workspace owner', names ^ set(OWNERS))
    for package in packages:
        name = package['name']
        owner = OWNERS[name].resolve()
        assert Path(package['manifest_path']).resolve() == owner / 'Cargo.toml', (name, 'Wrong owner directory')
        for target in package['targets']:
            assert Path(target['src_path']).resolve().is_relative_to(owner), (name, 'Target escapes its owner')
        if name not in ALLOWED:
            continue
        allowed = ALLOWED[name] | EXTERNAL[name]
        for dependency in package['dependencies']:
            dep = dependency['name']
            assert dep in allowed, (name, 'Unreviewed dependency capability', dep)
            if dep == 'veyndo-app-authority':
                if name == 'veyndo-cognition':
                    assert not dependency.get('uses_default_features', True) and set(dependency.get('features', [])) == {'vault'}, 'Cognition imports vault only, without database/OAuth features'
                elif name == 'veyndo-runtime':
                    assert not set(dependency.get('features', [])) - {'database'}, 'Runtime imports the existing signed database protocol only'
            if dependency.get('path'):
                assert dep in ALLOWED[name] and Path(dependency['path']).resolve() == OWNERS[dep].resolve(), (name, 'Path dependency escapes its owner', dep)
            elif dep in ALLOWED[name]:
                raise AssertionError((name, 'Domain dependency must use its reviewed local owner', dep))
        assert not any('custom-build' in t['kind'] for t in package['targets']), (name, 'Unreviewed domain build script')


def main():
    data = json.loads(subprocess.check_output(
        ['cargo', 'metadata', '--locked', '--no-deps', '--format-version', '1'], cwd=ROOT))
    check(data['packages'])
    from workspace_boundary_tests import negative_controls
    count = negative_controls(data['packages'])
    print(f'PASS Cargo owner/dependency boundaries; {count} bypass negative controls')


if __name__ == '__main__':
    main()
