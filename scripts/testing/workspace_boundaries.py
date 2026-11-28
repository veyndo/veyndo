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
               'veyndo-studio': ROOT / 'components/studio',
               'veyndo-app-platform': ROOT / 'components/apps'})
OWNERS.update({'veyndo': ROOT, 'veyndo-port-contracts': ROOT / 'crates/port-contracts', 'veyndo-migration': ROOT / 'extensions/apps/migration',
               'veyndo-marketplaces': ROOT / 'extensions/apps/marketplaces',
               'veyndo-app-storage': ROOT / 'extensions/services/app-storage',
               'veyndo-app-model': ROOT / 'extensions/services/app-storage/model',
               'veyndo-app-runtime': ROOT / 'extensions/services/app-storage/runtime',
               'veyndo-app-authority': ROOT / 'extensions/services/app-storage/authority',
               'veyndo-connectors': ROOT / 'extensions/services/connectors',
               'veyndo-paypal': ROOT / 'extensions/apps/paypal',
               'veyndo-site-builder': ROOT / 'extensions/apps/site-builder',
               'veyndo-site-forms': ROOT / 'extensions/apps/site-forms',
               'veyndo-connector-runtime': ROOT / 'extensions/services/connector-runtime',
               'veyndo-catalog-import': ROOT / 'extensions/apps/catalog-import',
               'veyndo-provider-email': ROOT / 'extensions/apps/email',
               'veyndo-provider-gmail': ROOT / 'extensions/apps/gmail',
               'veyndo-provider-google-analytics': ROOT / 'extensions/apps/google-analytics',
               'veyndo-provider-slack': ROOT / 'extensions/apps/slack',
               'veyndo-provider-algolia': ROOT / 'extensions/apps/algolia',
               'veyndo-provider-contentful': ROOT / 'extensions/apps/contentful',
               'veyndo-provider-hubspot': ROOT / 'extensions/apps/hubspot',
               'veyndo-provider-klaviyo': ROOT / 'extensions/apps/klaviyo',
               'veyndo-provider-vercel': ROOT / 'extensions/apps/vercel',
               # Generated external client: speaks HTTP only, never links a workspace crate.
               'veyndo-sdk': ROOT / 'sdks/rust'})
ALLOWED = {
    'veyndo-port-contracts': {'veyndo-core', 'veyndo-shop', 'veyndo-pricing', 'veyndo-shipping', 'veyndo-payments', 'veyndo-intelligence', 'veyndo-app-platform', 'veyndo-app-model'},
    'veyndo-app-model': set(),
    'veyndo-sdk': set(),
    'veyndo-app-runtime': {'veyndo-app-model'},
    'veyndo-app-storage': {'veyndo-app-model'},
    'veyndo-marketplaces': set(),
    'veyndo-kernel': set(),
    'veyndo-flow-graph': set(),
    'veyndo-automation': {'veyndo-kernel'},
    'veyndo-runtime': {'veyndo-app-authority'},
    'veyndo-app-authority': set(),
    # Standard app services: provider code never runs inside commerce request workers.
    'veyndo-connectors': {'veyndo-kernel', 'veyndo-pricing', 'veyndo-runtime', 'veyndo-connector-runtime', 'veyndo-provider-email', 'veyndo-provider-gmail', 'veyndo-provider-google-analytics', 'veyndo-provider-slack', 'veyndo-provider-algolia', 'veyndo-provider-contentful', 'veyndo-provider-hubspot', 'veyndo-provider-klaviyo', 'veyndo-provider-vercel'},
    # Standard app services: runtime, provider crates per app, service on top.
    'veyndo-connector-runtime': {'veyndo-runtime'},
    'veyndo-catalog-import': {'veyndo-core', 'veyndo-catalog', 'veyndo-pricing', 'veyndo-shop'},
    'veyndo-provider-email': {'veyndo-connector-runtime', 'veyndo-pricing', 'veyndo-runtime'},
    'veyndo-provider-gmail': {'veyndo-connector-runtime'},
    'veyndo-provider-google-analytics': {'veyndo-connector-runtime'},
    'veyndo-provider-slack': {'veyndo-connector-runtime', 'veyndo-provider-email'},
    'veyndo-provider-algolia': {'veyndo-connector-runtime'},
    'veyndo-provider-contentful': {'veyndo-connector-runtime'},
    'veyndo-provider-hubspot': {'veyndo-connector-runtime', 'veyndo-provider-email'},
    'veyndo-provider-klaviyo': {'veyndo-connector-runtime', 'veyndo-provider-email'},
    'veyndo-provider-vercel': {'veyndo-connector-runtime'},
    # The bundled PayPal adapter works over the payments ledger and its ports only.
    'veyndo-paypal': {'veyndo-core', 'veyndo-payments', 'veyndo-shop'},
    # Bundled site apps: component reads plus the SiteHost port for the server-owned steps.
    'veyndo-site-builder': {'veyndo-core', 'veyndo-identity', 'veyndo-shop', 'veyndo-catalog', 'veyndo-app-model', 'veyndo-app-platform', 'veyndo-intelligence', 'veyndo-studio'},
    'veyndo-site-forms': {'veyndo-core', 'veyndo-app-model'},
    # The optional `service` feature adds the shared error contract; the domain alone needs only the kernel.
    'veyndo-pricing': {'veyndo-kernel', 'veyndo-core', 'veyndo-identity'},
    'veyndo-apps': {'veyndo-kernel', 'veyndo-app-runtime'},
    'veyndo-app-platform': {'veyndo-core', 'veyndo-kernel', 'veyndo-app-model', 'veyndo-app-storage', 'veyndo-apps', 'veyndo-runtime', 'veyndo-identity', 'veyndo-pricing', 'veyndo-catalog', 'veyndo-orders', 'veyndo-checkout', 'veyndo-inventory', 'veyndo-shipping', 'veyndo-shop', 'veyndo-cognition', 'veyndo-b2b', 'veyndo-marketing', 'veyndo-intelligence'},
    'veyndo-cognition': {'veyndo-runtime', 'veyndo-kernel', 'veyndo-app-authority', 'veyndo-core'},
    'veyndo-core': {'veyndo-kernel', 'veyndo-runtime', 'veyndo-app-model'},
    'veyndo-identity': {'veyndo-core', 'veyndo-kernel', 'veyndo-app-model', 'veyndo-cognition', 'veyndo-runtime'},
    'veyndo-catalog': {'veyndo-core', 'veyndo-identity', 'veyndo-pricing'},
    'veyndo-inventory': {'veyndo-core', 'veyndo-catalog', 'veyndo-pricing'},
    'veyndo-shipping': {'veyndo-core', 'veyndo-identity', 'veyndo-pricing'},
    'veyndo-customers': {'veyndo-core', 'veyndo-pricing', 'veyndo-identity', 'veyndo-shipping', 'veyndo-shop', 'veyndo-runtime', 'veyndo-marketing'},
    'veyndo-marketing': {'veyndo-core', 'veyndo-pricing', 'veyndo-catalog', 'veyndo-shop', 'veyndo-app-model', 'veyndo-automation', 'veyndo-cognition', 'veyndo-flow-graph', 'veyndo-identity', 'veyndo-b2b', 'veyndo-kernel', 'veyndo-runtime'},
    'veyndo-orders': {'veyndo-core', 'veyndo-kernel', 'veyndo-b2b', 'veyndo-inventory', 'veyndo-catalog', 'veyndo-identity', 'veyndo-pricing', 'veyndo-shop', 'veyndo-app-model', 'veyndo-runtime', 'veyndo-shipping', 'veyndo-customers'},
    'veyndo-payments': {'veyndo-core', 'veyndo-pricing', 'veyndo-shop', 'veyndo-inventory', 'veyndo-orders', 'veyndo-kernel', 'veyndo-app-model', 'veyndo-identity'},
    'veyndo-b2b': {'veyndo-core', 'veyndo-pricing', 'veyndo-shop', 'veyndo-identity', 'veyndo-kernel', 'veyndo-app-model'},
    'veyndo-pos': {'veyndo-core', 'veyndo-pricing', 'veyndo-shop', 'veyndo-checkout', 'veyndo-customers', 'veyndo-identity', 'veyndo-marketing', 'veyndo-payments', 'veyndo-intelligence', 'veyndo-orders', 'veyndo-catalog', 'veyndo-app-model'},
    'veyndo-checkout': {'veyndo-core', 'veyndo-pricing', 'veyndo-shop', 'veyndo-shipping', 'veyndo-catalog', 'veyndo-b2b', 'veyndo-identity', 'veyndo-runtime', 'veyndo-marketing', 'veyndo-customers', 'veyndo-inventory', 'veyndo-orders', 'veyndo-payments'},
    'veyndo-intelligence': {'veyndo-core', 'veyndo-pricing', 'veyndo-runtime', 'veyndo-catalog', 'veyndo-kernel', 'veyndo-shipping', 'veyndo-identity', 'veyndo-shop', 'veyndo-marketing', 'veyndo-checkout', 'veyndo-inventory', 'veyndo-cognition', 'veyndo-app-model', 'veyndo-customers', 'veyndo-catalog-import'},
    'veyndo-studio': {'veyndo-core', 'veyndo-customers', 'veyndo-cognition', 'veyndo-b2b', 'veyndo-pos', 'veyndo-app-model', 'veyndo-identity', 'veyndo-app-platform', 'veyndo-orders', 'veyndo-pricing', 'veyndo-shop', 'veyndo-catalog', 'veyndo-intelligence', 'veyndo-marketing', 'veyndo-runtime', 'veyndo-checkout'},
    'veyndo-shop': {'veyndo-core', 'veyndo-pricing', 'veyndo-inventory', 'veyndo-shipping', 'veyndo-app-model', 'veyndo-catalog', 'veyndo-identity', 'veyndo-kernel'},
}
# Reviewed direct external capabilities. Cargo reports actual package names even
# when a manifest renames sqlx as something innocuous. New dependencies need an
# explicit architecture decision, including build/dev/optional/target variants.
EXTERNAL = {
    'veyndo-port-contracts': {'serde_json', 'reqwest', 'sqlx', 'tokio'},
    'veyndo-app-model': {'serde', 'serde_json', 'chrono', 'cron', 'sha2', 'semver', 'url'},
    'veyndo-sdk': {'reqwest', 'serde', 'serde_json', 'tokio'},
    'veyndo-app-runtime': {'wasmtime', 'wat', 'sha2', 'serde_json'},
    'veyndo-app-storage': {'sqlx', 'serde_json', 'sha2', 'tokio', 'chrono', 'base64', 'ring', 'zeroize'},
    'veyndo-paypal': {'axum', 'reqwest', 'serde_json', 'sqlx'},
    'veyndo-site-builder': {'axum', 'chrono', 'reqwest', 'serde', 'serde_json', 'sqlx'},
    'veyndo-site-forms': {'serde', 'serde_json', 'sqlx'},
    'veyndo-connectors': {'axum', 'hmac', 'regex', 'serde_json', 'sha2', 'sqlx', 'tokio'},
    'veyndo-catalog-import': {'reqwest', 'serde', 'serde_json'},
    'veyndo-connector-runtime': {'aes-gcm', 'axum', 'base64', 'chrono', 'regex', 'reqwest', 'serde_json', 'sha2', 'sqlx', 'uuid'},
    'veyndo-provider-email': {'base64', 'lettre', 'regex', 'rustls-pemfile', 'serde', 'serde_json', 'tokio', 'tokio-rustls', 'webpki-roots'},
    'veyndo-provider-gmail': {'base64', 'regex', 'reqwest', 'scraper', 'serde_json'},
    'veyndo-provider-google-analytics': {'serde_json'},
    'veyndo-provider-slack': {'regex', 'serde_json'},
    'veyndo-provider-algolia': {'serde_json'},
    'veyndo-provider-contentful': {'regex', 'reqwest', 'serde_json'},
    'veyndo-provider-hubspot': {'serde_json'},
    'veyndo-provider-klaviyo': {'serde_json'},
    'veyndo-provider-vercel': {'regex', 'reqwest', 'serde_json'},
    'veyndo-app-authority': {'zeroize', 'hmac', 'sha2', 'uuid', 'sqlx', 'reqwest', 'serde', 'serde_json', 'aes-gcm', 'base64'},
    'veyndo-marketplaces': {'axum', 'tokio', 'serde', 'serde_json', 'sha2', 'hmac', 'ring', 'chrono', 'reqwest'},
    'veyndo-kernel': set(),
    'veyndo-flow-graph': set(),
    'veyndo-automation': {'serde_json', 'chrono', 'chrono-tz'},
    'veyndo-pricing': {'serde', 'serde_json', 'chrono', 'regex', 'reqwest', 'tokio', 'axum', 'sqlx'},
    'veyndo-apps': set(),
    'veyndo-app-platform': {'semver', 'json-patch', 'serde', 'serde_json', 'reqwest', 'base64', 'ring', 'sqlx', 'chrono', 'tokio', 'cron', 'axum', 'hmac', 'sha2', 'tracing', 'uuid', 'veyndo-b2b', 'veyndo-marketing', 'veyndo-intelligence'},
    'veyndo-identity': {'serde', 'serde_json', 'axum', 'argon2', 'hmac', 'ring', 'sha2', 'tokio', 'uuid', 'sqlx', 'reqwest', 'base64'},
    'veyndo-catalog': {'serde', 'serde_json', 'sqlx', 'reqwest', 'axum', 'chrono', 'sha2', 'base64'},
    'veyndo-inventory': {'serde', 'serde_json', 'sqlx'},
    'veyndo-shipping': {'serde', 'serde_json', 'sqlx', 'axum'},
    'veyndo-customers': {'serde', 'serde_json', 'sqlx', 'axum', 'reqwest'},
    'veyndo-marketing': {'serde', 'serde_json', 'sqlx', 'axum', 'reqwest'},
    'veyndo-orders': {'serde', 'serde_json', 'sqlx', 'axum', 'chrono', 'reqwest', 'sha2', 'uuid', 'pdf-extract', 'image', 'base64', 'tokio'},
    'veyndo-payments': {'serde', 'serde_json', 'sqlx', 'chrono', 'reqwest', 'hmac', 'sha2', 'axum'},
    'veyndo-b2b': {'serde', 'serde_json', 'sqlx', 'chrono'},
    'veyndo-pos': {'serde', 'serde_json', 'sqlx'},
    'veyndo-checkout': {'serde', 'serde_json', 'chrono', 'sqlx', 'uuid', 'axum', 'base64', 'reqwest', 'ring', 'sha2'},
    'veyndo-intelligence': {'serde', 'serde_json', 'sqlx', 'reqwest', 'sha2', 'tokio', 'uuid', 'libc', 'pdf-extract', 'chrono', 'ring', 'base64', 'tracing', 'axum', 'image', 'scraper'},
    'veyndo-studio': {'axum', 'serde', 'serde_json', 'chrono', 'chrono-tz', 'sqlx', 'reqwest', 'uuid', 'tokio', 'base64', 'sha2'},
    'veyndo-shop': {'serde', 'serde_json', 'chrono', 'reqwest', 'sqlx', 'axum', 'tokio', 'futures-util'},
    'veyndo-core': {'axum', 'serde', 'serde_json', 'sha2', 'sqlx', 'uuid', 'tokio', 'tracing', 'reqwest'},
    'veyndo-runtime': {'hmac', 'sha2', 'zeroize', 'sqlx', 'tokio', 'futures-util',
                       'serde_json', 'reqwest', 'async-stream', 'uuid'},
    'veyndo-cognition': {'serde', 'serde_json', 'tokio', 'reqwest', 'aes-gcm',
                         'base64', 'sha2', 'sqlx', 'tracing', 'image'},
}

# Reviewed test-only crates: admitted as dev-dependencies only, never in a production build.
TEST_ONLY = {'proptest'}


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
            if dependency.get('kind') == 'dev' and dep in TEST_ONLY:
                continue
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
