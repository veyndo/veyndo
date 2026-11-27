#!/usr/bin/env python3
"""Upgrade from the previous release: its database must survive the forward migrations intact, and both
releases must serve the migrated database at the same time (expand/contract).

The previous release is the commit before the newest forward migration file was added (or `UPGRADE_FROM`).
The suite builds that commit's server in a temporary worktree under `target/schema-upgrade/` (the binary is
kept per commit), installs it fresh in its own database, writes a shop, products, a product edit, a cart, an
order and its payment through the previous release's HTTP API, then runs the new binary's
`BOOTSTRAP_MODE=migrate`. It checks that the ledger recorded every new migration, that the upgraded schema
equals `fixtures/schema/reference.sql` (the fresh-install schema), that the new binary serves the same
products, order and payment, that the new behaviour works (exact product prices), and finally serves one
request round from the old and the new binary together against the migrated database.
"""
import argparse
import json
import os
import re
import shutil
import socket
import subprocess
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid
from decimal import Decimal
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from schema_baseline import REFERENCE, normalize_schema  # noqa: E402
from testing.runtime import ROOT, artifact_path, stop  # noqa: E402

FOLDERS = ['components/*/migrations', 'extensions/services/connectors/migrations']
WORK = ROOT / 'target/schema-upgrade'
checks = []


def check(name):
    checks.append(name)
    print('PASS', name, flush=True)


def git(*args, check_result=True):
    return subprocess.run(['git', *args], cwd=ROOT, capture_output=True, text=True, check=check_result).stdout.strip()


def previous_release():
    """The commit before the newest forward migration file; HEAD while that file is not committed yet."""
    if os.environ.get('UPGRADE_FROM'):
        return git('rev-parse', '--verify', os.environ['UPGRADE_FROM'] + '^{commit}')
    forward = sorted((path for folder in FOLDERS for path in ROOT.glob(folder + '/*.sql')
                      if re.match(r'^[0-9]{4}-', path.name) and int(path.name[:4]) >= 2),
                     key=lambda path: (int(path.name[:4]), str(path)))
    if not forward:
        raise SystemExit('No forward migration exists yet; set UPGRADE_FROM to the release to upgrade from')
    added = [git('log', '--diff-filter=A', '-1', '--format=%H', '--', str(path.relative_to(ROOT))) for path in forward]
    if not all(added):
        return git('rev-parse', 'HEAD')
    newest = max(added, key=lambda sha: int(git('show', '-s', '--format=%ct', sha)))
    return git('rev-parse', newest + '^')


def build(commit):
    """The previous release's server binary, built once per commit in a temporary worktree under target/."""
    binary = WORK / f'veyndo-{commit[:12]}'
    if binary.exists():
        return binary
    tree = WORK / f'tree-{commit[:12]}'
    WORK.mkdir(parents=True, exist_ok=True)
    if tree.exists():
        git('worktree', 'remove', '--force', str(tree), check_result=False)
        shutil.rmtree(tree, ignore_errors=True)
    git('worktree', 'add', '--detach', str(tree), commit)
    try:
        env = {**os.environ, 'CARGO_TARGET_DIR': str(WORK / 'target'), 'SQLX_OFFLINE': 'true'}
        with artifact_path('schema-upgrade/build.log').open('w') as log:
            subprocess.run(['cargo', 'build', '--offline', '--bin', 'veyndo'], cwd=tree, env=env,
                           stdout=log, stderr=log, check=True)
        shutil.copy2(WORK / 'target/debug/veyndo', binary)
    finally:
        git('worktree', 'remove', '--force', str(tree), check_result=False)
    return binary


def free_port():
    with socket.socket() as probe:
        probe.bind(('127.0.0.1', 0))
        return probe.getsockname()[1]


class Server:
    """One release's server process on its own port against the shared upgraded database."""

    def __init__(self, binary, env, name):
        self.binary, self.name = binary, name
        port = free_port()
        self.base = f'http://127.0.0.1:{port}'
        self.env = {**env, 'BIND_ADDR': f'127.0.0.1:{port}', 'BASE_URL': self.base}
        self.process = None

    def migrate(self):
        with artifact_path(f'schema-upgrade/{self.name}-migrate.log').open('w') as log:
            subprocess.run([str(self.binary)], cwd=ROOT, env={**self.env, 'BOOTSTRAP_MODE': 'migrate'},
                           stdout=log, stderr=log, check=True)

    def start(self):
        log = artifact_path(f'schema-upgrade/{self.name}-serve.log').open('a')
        self.process = subprocess.Popen([str(self.binary)], cwd=ROOT, env={**self.env, 'BOOTSTRAP_MODE': 'serve'},
                                        stdout=log, stderr=log)
        for _ in range(300):
            if self.process.poll() is not None:
                raise RuntimeError(f'{self.name} server stopped during startup; see artifacts/schema-upgrade/{self.name}-serve.log')
            try:
                with urllib.request.urlopen(self.base + '/health', timeout=1):
                    return self
            except OSError:
                time.sleep(0.1)
        raise RuntimeError(f'{self.name} server did not become ready')

    def stop(self):
        if self.process:
            stop(self.process)
            self.process = None

    def call(self, path, body=None, h=None, method=None, status=200):
        request = urllib.request.Request(self.base + path, data=None if body is None else json.dumps(body).encode(),
                                         headers={'Content-Type': 'application/json', **(h or {})}, method=method)
        try:
            with urllib.request.urlopen(request, timeout=60) as response:
                code, value = response.status, json.load(response)
        except urllib.error.HTTPError as error:
            code, value = error.code, json.load(error)
        assert code == status, (self.name, path, code, status, value)
        return value


def psql(container, user, database, statement):
    return subprocess.run(['docker', 'exec', '-i', container, 'psql', '-U', user, '-d', database, '-XAtq', '-v', 'ON_ERROR_STOP=1'],
                          input=statement, capture_output=True, text=True, check=True).stdout.strip()


def product_draft(id, number, price, list_price):
    return {'id': id, 'revision': 0,
            'catalog': {'active': True, 'parentId': None, 'productNumber': number, 'options': {}, 'categoryIds': []},
            'commerce': {'price': price, 'taxRate': 19, 'stock': 50, 'minPurchase': 1, 'purchaseSteps': 1, 'maxPurchase': None,
                         'deliveryDays': 2, 'listPrice': list_price, 'advancedPrices': [], 'media': [], 'properties': {}},
            'extra': {}, 'translations': {'en': {'name': 'Upgrade ' + number, 'description': 'Synthetic upgrade fixture'}}}


def editable(product):
    return {k: json.loads(json.dumps(product[k])) for k in ['revision', 'contentRevision', 'catalog', 'commerce', 'extra', 'translations'] if k in product}


def purchase(server, tenant, product, tag):
    """A storefront cart and order of one product through the server's HTTP API."""
    public = {'x-tenant': tenant}
    cart = server.call('/store-api/checkout/cart', {}, public)
    buyer = {**public, 'sw-context-token': cart['token']}
    cart = server.call('/store-api/checkout/cart/line-item', {'items': [{'referencedId': product, 'quantity': 2}]}, buyer)
    address = {'name': 'Upgrade Buyer', 'firstName': 'Upgrade', 'lastName': 'Buyer', 'street': 'Fixture Street 1',
               'postalCode': '10115', 'city': 'Berlin', 'country': 'DE'}
    selection = {**cart['checkout'], 'customerEmail': f'upgrade-{tag}@example.test', 'address': address, 'billingAddress': address}
    cart = server.call('/store-api/checkout/context', {'revision': cart['revision'], 'checkout': selection}, buyer, 'PUT')
    total = Decimal(str(cart['price']['totalPrice']))
    order = server.call('/store-api/checkout/order', {}, {**buyer, 'Idempotency-Key': 'upgrade-' + tag,
                                                          'x-commerce-cart-revision': str(cart['revision']),
                                                          'x-commerce-total-minor': str(int(total * 100)),
                                                          'x-commerce-currency': 'EUR'})
    return order, buyer


def merchant_view(server, h, products, order_id):
    """What the merchant and the storefront see of the written data, compared across releases."""
    view = {'products': {}, 'order': None}
    for id in products:
        p = server.call('/api/merchant/products/' + id, h=h)
        view['products'][id] = {'commerce': p['commerce'], 'catalog': p['catalog'], 'revision': p['revision']}
    listing = server.call('/store-api/product', {}, {'x-tenant': h['x-tenant']})['elements']
    view['storefront'] = sorted((p['id'], p['price'], p.get('tax_rate')) for p in listing if p['id'] in products)
    view['order'] = server.call('/api/merchant/orders/' + order_id, h=h)
    return view


def schema_only(container, user, database):
    dump = subprocess.run(['docker', 'exec', container, 'pg_dump', '-U', user, '-d', database, '--no-owner', '--schema-only'],
                          capture_output=True, text=True, check=True).stdout
    return ';\n\n'.join(normalize_schema(dump)) + ';'


def main():
    parser = argparse.ArgumentParser(description=__doc__.split('\n\n')[0])
    parser.add_argument('--binary', default=str(ROOT / 'target/debug/veyndo'))
    args = parser.parse_args()
    env = dict(os.environ)
    container = env.get('TEST_DB_CONTAINER') or env.get('DB_CONTAINER') or 'veyndo-postgres-1'
    url = urllib.parse.urlsplit(env['DATABASE_URL'])
    user = url.username or 'commerce'
    commit = previous_release()
    print(f'Previous release: {commit[:12]} ({git("show", "-s", "--format=%s", commit)})', flush=True)
    old_binary, new_binary = build(commit), Path(args.binary)
    name = 'schema_upgrade_' + uuid.uuid4().hex
    psql(container, user, 'postgres', f'CREATE DATABASE "{name}";')
    database = {**env, 'DATABASE_URL': urllib.parse.urlunsplit(url._replace(path='/' + name)), 'SEED_DEMO': 'false',
                'MERCHANT_TOKEN': env.get('MERCHANT_TOKEN', 'schema-upgrade-synthetic-bootstrap-credential')}
    for key in ('DATABASE_RUNTIME_URL', 'DATABASE_TENANT_URL', 'DATABASE_LISTENER_URL'):
        database.pop(key, None)
    old, new = Server(old_binary, database, 'previous'), Server(new_binary, database, 'current')
    tag = uuid.uuid4().hex[:8]
    try:
        # 1. A fresh installation of the previous release, written to through its own HTTP API.
        old.migrate()
        old.start()
        session = old.call('/api/auth/register', {'workspaceId': 'upgrade-' + tag, 'workspaceName': 'Upgrade laboratory',
                                                  'name': 'Upgrade owner', 'email': f'upgrade-{tag}@example.test',
                                                  'password': 'Synthetic-upgrade-fixture-2026!'})
        tenant = session['workspace']
        h = {'x-tenant': tenant, 'Authorization': 'Bearer ' + session['token']}
        settings = old.call('/api/merchant/commerce', h=h)
        settings['data']['shipping'][0]['price'] = 4.9
        old.call('/api/merchant/commerce', {'data': settings['data'], 'revision': settings['revision']}, h, 'PUT')
        lamp, mug, fresh = str(uuid.uuid4()), str(uuid.uuid4()), str(uuid.uuid4())
        old.call('/api/merchant/products', product_draft(lamp, 'UP-LAMP-' + tag, 49.9, 59.9), h)
        old.call('/api/merchant/products', product_draft(mug, 'UP-MUG-' + tag, 12.5, None), h)
        edit = editable(old.call('/api/merchant/products/' + mug, h=h))
        edit['commerce']['price'] = 0.3
        old.call('/api/merchant/products/' + mug, edit, h, 'PUT')
        order, buyer = purchase(old, tenant, lamp, tag)
        placed = old.call('/api/merchant/orders/' + order['id'], h=h)
        # The shop's default manual payment method: the merchant records the payment through the order's payment transition.
        old.call('/api/merchant/orders/' + order['id'] + '/transition',
                 {'kind': 'payment', 'state': 'paid', 'revision': placed['revision']}, h)
        before = merchant_view(old, h, [lamp, mug], order['id'])
        artifact_path('schema-upgrade/order-before.json').write_text(json.dumps(before, indent=1, sort_keys=True))
        old.stop()
        assert before['products'][mug]['commerce']['price'] == 0.3 and before['products'][lamp]['commerce']['listPrice'] == 59.9
        assert before['order']['revision'] > placed['revision'], (placed, before['order'])
        check(f'Previous release {commit[:12]} installed fresh; shop settings, two products, a price edit, a cart, an '
              'order and its recorded payment written through its HTTP API')

        # 2. The new binary migrates the previous release's database.
        new.migrate()
        ledger = psql(container, user, name, 'SELECT version FROM commerce_migrations ORDER BY applied_at, version;').splitlines()
        assert 'catalog/0002-exact-product-prices' in ledger, ledger
        assert psql(container, user, name, 'SELECT count(*) FROM products WHERE price_decimal IS NULL OR tax_rate_decimal IS NULL '
                    'OR (list_price IS NOT NULL AND list_price_decimal IS NULL);') == '0'
        rows = {id: psql(container, user, name, f"SELECT price_decimal, tax_rate_decimal, list_price_decimal FROM products "
                         f"WHERE tenant='{tenant}' AND id='{id}';") for id in (lamp, mug)}
        assert rows == {lamp: '49.9|19|59.9', mug: '0.3|19|'}, rows
        reference = REFERENCE.read_text().split('\n\n-- rows\n', 1)[0].removeprefix('-- schema\n').strip()
        upgraded = schema_only(container, user, name)
        if upgraded != reference:
            import difflib
            diff = ''.join(difflib.unified_diff(reference.splitlines(True), upgraded.splitlines(True), 'fresh install', 'upgraded', n=2))
            artifact_path('schema-upgrade/schema.diff').write_text(diff)
            raise AssertionError('Upgraded schema differs from the fresh-install reference:\n' + diff[:8000])
        check('The new binary migrates it: ledger records the new migrations, every product price is backfilled to its '
              'exact decimal (0.3, not 0.29999999999999999), and the schema equals the fresh-install reference')

        # 3. The new binary serves the upgraded data unchanged, and its new behaviour works.
        new.start()
        after = merchant_view(new, h, [lamp, mug], order['id'])
        artifact_path('schema-upgrade/order-after.json').write_text(json.dumps(after, indent=1, sort_keys=True))
        assert after == before, 'see artifacts/schema-upgrade/order-before.json and order-after.json'
        check('The new release serves the same products, storefront prices and the complete merchant order document, '
              'including its payment')
        edit = editable(new.call('/api/merchant/products/' + lamp, h=h))
        edit['commerce']['price'] = 19.99
        new.call('/api/merchant/products/' + lamp, edit, h, 'PUT')
        stored = psql(container, user, name, f"SELECT price_decimal, price FROM products WHERE tenant='{tenant}' AND id='{lamp}';")
        assert stored == '19.99|19.99', stored
        new.call('/api/merchant/products', product_draft(fresh, 'UP-NEW-' + tag, 0.1, None), h)
        assert psql(container, user, name, f"SELECT price_decimal, price FROM products WHERE tenant='{tenant}' AND id='{fresh}';") == '0.1|0.1'
        second, _ = purchase(new, tenant, lamp, tag + '-new')
        assert Decimal(str(second['cart']['price']['totalPrice'])) >= Decimal('39.98'), second['cart']['price']
        check('New behaviour: the new release writes exact numeric prices (19.99, 0.1) and keeps the previous '
              "release's binary64 column in step; it prices and places an order from them")

        # 4. Expand/contract: both releases serve the migrated database at the same time for one request round.
        old.start()
        assert old.call('/api/merchant/products/' + lamp, h=h)['commerce']['price'] == 19.99
        edit = editable(old.call('/api/merchant/products/' + mug, h=h))
        edit['commerce']['price'] = 7.7
        old.call('/api/merchant/products/' + mug, edit, h, 'PUT')
        assert new.call('/api/merchant/products/' + mug, h=h)['commerce']['price'] == 7.7
        assert psql(container, user, name, f"SELECT price_decimal FROM products WHERE tenant='{tenant}' AND id='{mug}';") == '7.7'
        edit = editable(new.call('/api/merchant/products/' + lamp, h=h))
        edit['commerce']['price'] = 21.5
        new.call('/api/merchant/products/' + lamp, edit, h, 'PUT')
        assert old.call('/api/merchant/products/' + lamp, h=h)['commerce']['price'] == 21.5
        old_listing = sorted((p['id'], p['price']) for p in old.call('/store-api/product', {}, {'x-tenant': tenant})['elements'])
        new_listing = sorted((p['id'], p['price']) for p in new.call('/store-api/product', {}, {'x-tenant': tenant})['elements'])
        assert old_listing == new_listing, (old_listing, new_listing)
        third, _ = purchase(old, tenant, mug, tag + '-old')
        fourth, _ = purchase(new, tenant, mug, tag + '-both')
        assert third['cart']['price']['totalPrice'] == fourth['cart']['price']['totalPrice'], (third['cart']['price'], fourth['cart']['price'])
        assert old.call('/api/merchant/orders/' + fourth['id'], h=h)['id'] == fourth['id']
        assert new.call('/api/merchant/orders/' + third['id'], h=h)['id'] == third['id']
        check('Old and new release serve together: each reads the price the other wrote, both list the same storefront '
              'prices, price the same order equally and read each other\'s orders')
    finally:
        old.stop()
        new.stop()
        psql(container, user, 'postgres', f'DROP DATABASE IF EXISTS "{name}" WITH (FORCE);')
    print(f'PASS schema upgrade from {commit[:12]}: {len(checks)} checks')


if __name__ == '__main__':
    main()
