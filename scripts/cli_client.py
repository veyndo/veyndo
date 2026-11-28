#!/usr/bin/env python3
"""Run the `veyndo` command-line client (sdks/cli) against the suite's running server: sign-in, shops, sandbox import and release, apps, operations, MCP and exit codes.

Registered HTTP suite: `scripts/verify_integration.py --only cli_client`. Build the client first with
`cargo build --manifest-path sdks/cli/Cargo.toml` (its own workspace: its executable is named
`veyndo` like the server). Every account, key and shop here is synthetic; the client gets a private
configuration directory, and no secret may appear in any output.
"""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time
from urllib.error import HTTPError
from urllib.request import Request, urlopen

ROOT = Path(__file__).resolve().parents[1]
BINARY = ROOT / 'sdks/cli/target/debug/veyndo'
BASE = os.environ.get('BASE_URL')
assert BASE, 'Run through scripts/verify_integration.py --only cli_client'
sources = [p for folder in ('sdks/cli/src', 'sdks/rust/src') for p in (ROOT / folder).rglob('*.rs')]
assert BINARY.exists() and BINARY.stat().st_mtime >= max(p.stat().st_mtime for p in sources), \
    'Build the client first: cargo build --manifest-path sdks/cli/Cargo.toml'

CONFIG = Path(tempfile.mkdtemp(prefix='veyndo-cli-suite-'))
WORK = Path(tempfile.mkdtemp(prefix='veyndo-cli-work-'))
SECRETS = []
ENV = {k: v for k, v in os.environ.items() if not k.startswith('VEYNDO_')}
ENV.update({'VEYNDO_CONFIG_DIR': str(CONFIG), 'VEYNDO_SERVER': BASE, 'NO_COLOR': '1'})


def http(path, body=None, token=None, tenant=None, status=200):
    headers = {'Content-Type': 'application/json'}
    if token:
        headers['Authorization'] = 'Bearer ' + token
    if tenant:
        headers['x-tenant'] = tenant
    request = Request(BASE + path, data=None if body is None else json.dumps(body).encode(), headers=headers)
    try:
        response = urlopen(request, timeout=30)
    except HTTPError as error:
        response = error
    with response:
        payload = response.read()
        assert response.status == status, (path, response.status, payload[:400])
        return json.loads(payload) if payload else None


def cli(*args, code=0, stdin=None, env=None, cwd=None):
    """Runs the client and checks its exit code; no stored secret may ever be printed."""
    done = subprocess.run([str(BINARY), *args], input=stdin, capture_output=True, text=True,
                          env={**ENV, **(env or {})}, cwd=cwd or WORK, timeout=120)
    for secret in SECRETS:
        assert secret not in done.stdout and secret not in done.stderr, ('secret printed by', args)
    assert done.returncode == code, (args, done.returncode, done.stdout[-2000:], done.stderr[-2000:])
    return done


def cli_json(*args, **kwargs):
    return json.loads(cli('--json', *args, **kwargs).stdout)


suffix = f'{time.time_ns():x}'
email, password = f'cli-{suffix}@example.test', 'Synthetic-cli-2026!'
SECRETS.append(password)
account = http('/api/auth/register', {'email': email, 'name': 'CLI suite', 'password': password,
                                      'workspaceId': f'cli-{suffix}'})
shop = account['workspace']
SECRETS.append(account['token'])

# Signed out: status still answers, protected commands exit 3, usage errors exit 2.
assert cli_json('status')['signedIn'] is False
cli('products', 'list', '--shop', shop, code=3)
cli('products', code=2)
cli('--server', 'http://example.test', 'status', code=2)

# Personal sign-in: the password comes from stdin and the profile file is owner-only.
login = cli_json('login', '--email', email, '--password-stdin', stdin=password + '\n')
assert login == {'signedIn': True, 'kind': 'session', 'email': email, 'shop': shop, 'server': BASE}, login
credentials = CONFIG / 'credentials.json'
assert credentials.stat().st_mode & 0o777 == 0o600
SECRETS.append(json.loads(credentials.read_text())['credential']['secret'])
status = cli_json('status')
assert status['signedIn'] and status['personalSession'] and status['email'] == email, status
assert 'secret' not in json.dumps(status)
cli('login', '--email', email, '--password-stdin', stdin='wrong-password\n', code=3)
shops = cli_json('shop', 'list')['shops']
assert [s['id'] for s in shops] == [shop] and shops[0]['selected'], shops
cli('shop', 'use', 'no-such-shop', code=5)
second = f'cli2-{suffix}'
assert cli_json('shop', 'create', second, '--name', 'CLI second')['shop'] == second
assert {s['id'] for s in cli_json('shop', 'list')['shops']} == {shop, second}
SECRETS.append(json.loads(credentials.read_text())['credential']['secret'])
cli('shop', 'use', shop)
assert cli_json('status')['shop'] == shop

# Integration keys are created by a personal administrator; the CLI signs in with one from stdin.
session = json.loads(credentials.read_text())['credential']['secret']
permissions = ['catalog.read', 'catalog.write', 'environments.read', 'environments.manage', 'apps.read',
               'apps.manage', 'team.manage', 'orders.read', 'customers.pii']
key = http('/api/workspace/integrations', {'name': 'cli-suite', 'permissions': permissions, 'expiresInDays': 1},
           token=session, tenant=shop)['key']
narrow = http('/api/workspace/integrations', {'name': 'cli-read', 'permissions': ['catalog.read'], 'expiresInDays': 1},
              token=session, tenant=shop)['key']
SECRETS.extend([key, narrow])
assert cli_json('logout')['removed'] is True
assert not credentials.exists()
cli('login', '--with-key', stdin=key + '\n', code=2)  # a key needs its shop
keyed = cli_json('login', '--with-key', '--shop', shop, stdin=key + '\n')
assert keyed['kind'] == 'integration-key' and 'apps.manage' in keyed['permissions'], keyed
assert cli_json('status')['personalSession'] is False
cli('shop', 'list', code=4)  # listing shops needs a personal session
cli('shop', 'use', second, code=4)  # a key stays bound to its shop
# The environment key wins over the profile, and an unknown key is refused.
assert cli_json('status', env={'VEYNDO_API_KEY': narrow})['credentialSource'] == 'VEYNDO_API_KEY'
cli('sandbox', 'create', env={'VEYNDO_API_KEY': narrow}, code=4)
cli('products', 'list', env={'VEYNDO_API_KEY': 'not-a-key'}, code=3)

# Catalogue reads.
products = cli_json('products', 'list', '--limit', '3')['elements']
assert 0 < len(products) <= 3, products
paged = cli_json('products', 'list', '--limit', '200')['elements']
assert len({p['id'] for p in paged}) == len(paged) >= len(products)
first = cli_json('products', 'get', products[0]['id'])
assert first['id'] == products[0]['id']
cli('products', 'get', 'no-such-product', code=5)
table = cli('products', 'list', '--limit', '2').stdout
assert table.splitlines()[0].split()[:3] == ['ID', 'NUMBER', 'NAME'], table
assert isinstance(cli_json('orders', 'list')['elements'], list)

# CSV import goes into a new private sandbox, replays idempotently and is released explicitly.
catalog = WORK / 'catalog.csv'
catalog.write_text('name,price,description,productNumber\n'
                   f'CLI Lamp {suffix},49.5,"A lamp, imported",CLI-{suffix}-1\n'
                   f'CLI Desk {suffix},199,A desk,CLI-{suffix}-2\n')
imported = cli_json('products', 'import', str(catalog), '--tax-rate', '19')
sandbox = imported['sandbox']
assert [p['status'] for p in imported['products']] == ['created', 'created'], imported
again = cli_json('products', 'import', str(catalog), '--sandbox', sandbox, '--tax-rate', '19')
assert [p['status'] for p in again['products']] == ['unchanged', 'unchanged'], again
names = {p['name'] for p in cli_json('products', 'list', '--sandbox', sandbox, '--limit', '200', '--search', 'CLI')['elements']}
assert {f'CLI Lamp {suffix}', f'CLI Desk {suffix}'} <= names, names
live_names = {p['name'] for p in cli_json('products', 'list', '--limit', '200', '--search', 'CLI')['elements']}
assert not live_names & names, 'the import must not touch the live shop'
(WORK / 'bad.csv').write_text('title,cost\nx,1\n')
cli('products', 'import', str(WORK / 'bad.csv'), '--sandbox', sandbox, '--tax-rate', '19', code=6)
cli('products', 'import', str(WORK / 'missing.csv'), code=6)
cli('products', 'import', str(catalog), '--sandbox', 'not-a-sandbox', code=5)
assert sandbox in {s['id'] for s in cli_json('sandbox', 'list')['environments']}
changes = cli_json('sandbox', 'diff', sandbox)['changes']
product_keys = [c['key'] for c in changes if c['key'].startswith('product')]
assert len(product_keys) >= 2, changes
cli('sandbox', 'release', sandbox, code=2)  # no terminal and no --yes: nothing is published
cli('sandbox', 'release', sandbox, '--key', 'no-such-change', '--yes', code=5)
released = cli_json('sandbox', 'release', sandbox, *sum((['--key', k] for k in product_keys), []), '--yes')
assert released['releases'] and released['releases'][0]['published'], released
live_names = {p['name'] for p in cli_json('products', 'list', '--limit', '200', '--search', 'CLI')['elements']}
assert {f'CLI Lamp {suffix}', f'CLI Desk {suffix}'} <= live_names, live_names

# Apps: scaffold from a Studio pattern, validate offline and on the server, deploy into a sandbox.
assert {t['name'] for t in cli_json('app', 'new')['templates']} == {'advisor', 'returns', 'erp', 'edi', 'furniture'}
app_id = f'cli_advisor_{suffix}'[:60]
cli('app', 'new', 'advisor', 'advisor-app', '--id', app_id)
cli('app', 'new', 'advisor', 'advisor-app', '--id', app_id, code=7)
cli('app', 'new', 'nope', code=2)
assert json.loads((WORK / 'advisor-app/manifest.json').read_text())['id'] == app_id
assert cli_json('app', 'validate', 'advisor-app', '--offline') == {'valid': True, 'reviewed': False}
reviewed = cli_json('app', 'validate', 'advisor-app')
assert reviewed['app'] == app_id and len(reviewed['digest']) > 20 and reviewed['previousVersion'] is None, reviewed
broken = WORK / 'broken'
broken.mkdir()
(broken / 'manifest.json').write_text(json.dumps({'id': 'Bad-Id', 'version': '1', 'name': {}, 'permissions': []}))
assert len(cli_json('app', 'validate', str(broken), code=10)['findings']) == 3
(broken / 'manifest.json').write_text(json.dumps({'id': 'cli_broken', 'version': '1.0.0', 'name': {'en': 'Broken'},
                                                  'permissions': ['data.read'], 'entities': 'not-a-list'}))
cli('app', 'validate', str(broken), code=10)  # the server compiler refuses it
deployed = cli_json('app', 'deploy', 'advisor-app')
app_sandbox = deployed['sandbox']
assert deployed['review']['digest'] == reviewed['digest'], deployed
installed = http('/api/apps', token=key, tenant=app_sandbox)
assert any(p.get('id') == app_id or p.get('manifest', {}).get('id') == app_id for p in installed['packages']), installed
assert app_id not in json.dumps(http('/api/apps', token=key, tenant=shop)), 'deploy must not touch the live shop'
logs = cli_json('app', 'logs', app_id, '--sandbox', app_sandbox)
assert isinstance(logs['calls'], list) and isinstance(logs['deliveries'], list), logs
cli('app', 'logs', 'no_such_app', '--sandbox', app_sandbox, code=5)

# Operations graph and planner check.
operations = cli_json('ops', 'list')['operations']
assert any(o['name'] == 'merchant.products' for o in operations)
assert all('facts' in o and 'requiresApproval' in o for o in operations)
assert [o['name'] for o in cli_json('ops', 'list', '--name', 'merchant.products')['operations']] == ['merchant.products']
(WORK / 'plan.json').write_text(json.dumps({'steps': [{'name': 'merchant.products', 'arguments': {'limit': 5}}]}))
checked = cli_json('ops', 'check', str(WORK / 'plan.json'))
assert checked['executable'] and checked['steps'][0]['verdict'] == 'allowed', checked
(WORK / 'blocked.json').write_text(json.dumps([{'name': 'no.such.operation'}]))
assert cli_json('ops', 'check', str(WORK / 'blocked.json'), code=10)['steps'][0]['verdict'] == 'unknown'
assert cli_json('ops', 'check', '-', stdin=(WORK / 'plan.json').read_text())['executable']
(WORK / 'invalid.json').write_text('{"steps": 3}')
cli('ops', 'check', str(WORK / 'invalid.json'), code=6)

# MCP: configuration references the key, the stdio bridge forwards JSON-RPC with the stored credential.
mcp = cli_json('mcp')
assert mcp['endpoint'] == BASE + '/mcp' and mcp['shop'] == shop and set(mcp['configs']) == {'ClaudeCode', 'ClaudeDesktop', 'Cursor'}
assert 'VEYNDO_API_KEY' in cli('mcp', '--client', 'cursor').stdout
bridge = cli('mcp', 'serve', stdin='\n'.join([
    json.dumps({'jsonrpc': '2.0', 'id': 1, 'method': 'initialize', 'params': {'protocolVersion': '2025-06-18', 'capabilities': {}, 'clientInfo': {'name': 'cli-suite', 'version': '1'}}}),
    json.dumps({'jsonrpc': '2.0', 'method': 'notifications/initialized'}),
    json.dumps({'jsonrpc': '2.0', 'id': 2, 'method': 'tools/list'}),
    'not json',
]) + '\n').stdout.splitlines()
replies = [json.loads(line) for line in bridge]
assert [r.get('id') for r in replies] == [1, 2, None], replies
assert any(t['name'] == 'merchant.products' for t in replies[1]['result']['tools'])
assert replies[2]['error']['code'] == -32700

# Shell completions and help.
for shell in ('bash', 'zsh', 'fish'):
    assert 'sandbox' in cli('completions', shell).stdout
assert 'Exit codes' in cli('--help').stdout
cli('agent', 'eval', '--repo', str(WORK), code=2)
assert cli_json('logout')['removed'] is True
print('PASS veyndo CLI: personal and key sign-in, shops, catalogue and orders, sandbox import and release, '
      'app validate/deploy/logs, operations check, MCP bridge, exit codes, no secret in output')
