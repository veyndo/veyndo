#!/usr/bin/env python3
"""Real-browser journeys through Studio and storefront: purchase, refund, product edit while orders arrive, app installation and a non-English main language.

The suite owns its server (the built `target/debug/veyndo` and `frontend/dist`), a
local payment-provider fixture and one disposable shop per journey, registered
through the ordinary sign-up. Fixture setup goes through the HTTP API; every
journey step under test goes through the browser (`scripts/testing/browser_journeys.mjs`).

Browser driver: the pinned `playwright` package from `scripts/testing/requirements.txt`
(`python3 -m playwright install chromium-headless-shell`), whose bundled Node runs
the journeys. `BROWSER_NODE` and `BROWSER_MODULE` select another Node and Playwright
module. Synthetic data only; the server listens on loopback.
"""
import copy
import json
import os
import pathlib
import secrets
import socket
import subprocess
import tempfile
import threading
import time
import urllib.error
import urllib.request
import uuid
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

from testing.app_approval import consent
from testing.runtime import ROOT, artifact_path, serve, stop

PROVIDER_SECRET = 'synthetic-journey-provider-credential-' + secrets.token_hex(8)
provider_calls = []


class Provider(BaseHTTPRequestHandler):
    """Local wire fixture of the generic payment-provider contract: approves, captures and refunds."""

    def log_message(self, *_):
        pass

    def do_POST(self):
        assert self.headers['Authorization'] == 'Bearer ' + PROVIDER_SECRET
        body = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
        provider_calls.append(body['operation'])
        keys = ['apiVersion', 'provider', 'adapterVersion', 'tenant', 'environment']
        if self.path == '/v1/onboarding':
            value = {**{k: body[k] for k in keys + ['channel']}, 'accountRef': 'merchant-' + body['tenant'],
                     'ready': body['operation'] != 'disconnect', 'status': 'ready', 'methods': ['wallet'],
                     'capabilities': ['capture', 'authorize', 'void', 'refund']}
        else:
            value = {**{k: body[k] for k in keys + ['attemptId', 'orderId', 'accountRef', 'amountMinor', 'currency']},
                     'reference': body.get('reference') or 'REF-' + body['attemptId'], 'status': 'ready',
                     'approvalUrl': 'https://provider.example.test/pay', 'confirmed': True}
            operation = body['operation']
            if operation == 'reconcile':
                value['status'] = 'approved'
            if operation == 'capture':
                value.update(status='captured', captureId='CAP-' + body['attemptId'], settledAmountMinor=body['amountMinor'])
            if operation == 'refund':
                value.update(status='refunded', refund={'id': 'REFUND-' + body['requestKey'],
                                                        'amountMinor': body['input']['amountMinor'], 'currency': body['currency']})
        data = json.dumps(value).encode()
        self.send_response(200)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Content-Length', str(len(data)))
        self.end_headers()
        self.wfile.write(data)


def free_port():
    with socket.socket() as probe:
        probe.bind(('127.0.0.1', 0))
        return probe.getsockname()[1]


def browser_driver():
    """The Node binary and Playwright module that run the journeys."""
    node, module = os.environ.get('BROWSER_NODE'), os.environ.get('BROWSER_MODULE')
    if node and module:
        return node, module
    try:
        import playwright
    except ImportError:
        raise SystemExit('Browser journeys need the pinned driver: python3 -m pip install -r scripts/testing/requirements.txt '
                         'and python3 -m playwright install chromium-headless-shell')
    driver = pathlib.Path(playwright.__file__).parent / 'driver'
    return str(driver / 'node'), str(driver / 'package' / 'index.mjs')


fixture = ThreadingHTTPServer(('127.0.0.1', 0), Provider)
threading.Thread(target=fixture.serve_forever, daemon=True).start()
port = free_port()
base = f'http://127.0.0.1:{port}'
provider = {'url': f'http://127.0.0.1:{fixture.server_address[1]}', 'token': PROVIDER_SECRET,
            'environment': 'contract-fixture', 'approvalOrigins': ['https://provider.example.test']}
env = {**os.environ, 'BIND_ADDR': f'127.0.0.1:{port}', 'BASE_URL': base, 'DEMO_CATALOG': 'fashion',
       'PAYMENT_SERVICES': json.dumps({'example_payments': {'1.0.0': provider}}),
       'PAYPAL_ACCOUNTS': '{}', 'PAYPAL_SANDBOX_ACCOUNTS': '{}'}


def call(path, body=None, headers=None, method=None, expected=200):
    request = urllib.request.Request(base + path, data=None if body is None else json.dumps(body).encode(),
                                     headers={'Content-Type': 'application/json', **(headers or {})},
                                     method=method or ('GET' if body is None else 'POST'))
    try:
        with urllib.request.urlopen(request, timeout=45) as response:
            code, data = response.status, json.load(response)
    except urllib.error.HTTPError as error:
        code, data = error.code, json.load(error)
    assert code == expected, (path, code, expected, data)
    return data


def register(label):
    """A disposable shop with its owner; the password is synthetic and never printed."""
    shop = f'journey-{label}-{uuid.uuid4().hex[:8]}'
    email, password = f'{shop}@example.test', secrets.token_urlsafe(18)
    session = call('/api/auth/register', {'name': 'Journey Owner', 'email': email, 'password': password,
                                          'workspaceId': shop, 'workspaceName': f'Journey {label}'})
    return {'id': shop, 'email': email, 'password': password, 'token': session['token']}


def merchant(shop):
    return {'Authorization': 'Bearer ' + shop['token'], 'x-tenant': shop['id']}


def update_commerce(shop, change):
    config = call('/api/merchant/commerce', headers=merchant(shop))
    change(config['data'])
    call('/api/merchant/commerce', {'data': config['data'], 'revision': config['revision']}, merchant(shop), 'PUT')


def provider_order(shop):
    """A captured order paid through the provider fixture, ready to be refunded in Studio."""
    manifest = json.loads((ROOT / 'extensions/apps/payment-provider/manifest.json').read_text())
    call('/api/apps', consent({'manifest': copy.deepcopy(manifest)}), merchant(shop))
    call(f"/api/payment-providers/{manifest['id']}/onboarding",
         {'operation': 'start', 'channel': 'default', 'country': 'DE', 'requestKey': uuid.uuid4().hex, 'approve': True},
         merchant(shop))

    def activate(data):
        next(method for method in data['payments'] if method['id'] == 'example-payments-wallet')['active'] = True
    update_commerce(shop, activate)
    headers = {'x-tenant': shop['id']}
    cart = call('/store-api/checkout/cart', {'session': uuid.uuid4().hex}, headers)
    headers['sw-context-token'] = cart['token']
    cart = call('/store-api/checkout/cart/line-item', {'items': [{'referencedId': 'bag', 'quantity': 1}]}, headers)
    selection = cart['checkout']
    selection.update(paymentMethodId='example-payments-wallet', customerEmail='refund-journey@example.test',
                     billingAddress={'name': 'Rita Refund', 'firstName': 'Rita', 'lastName': 'Refund',
                                     'street': 'Teststraße 2', 'postalCode': '10115', 'city': 'Berlin', 'country': 'DE'})
    call('/store-api/checkout/context', {'revision': cart['revision'], 'checkout': selection}, headers, 'PUT')
    order = call('/store-api/checkout/order', {}, {**headers, 'Idempotency-Key': uuid.uuid4().hex})
    attempt = order['payment']['attemptId']
    call(f'/store-api/payments/{attempt}/reconcile', {}, {**headers, 'Idempotency-Key': uuid.uuid4().hex})
    for _ in range(100):
        payment = call(f'/store-api/payments/{attempt}', headers=headers)
        if payment['state'] == 'captured':
            return {'number': order['orderNumber'], 'attempt': attempt, 'amountMinor': payment['amountMinor'],
                    'shopper': headers}
        time.sleep(.15)
    raise AssertionError(('payment not captured', payment))


def spanish_main_language(shop):
    """Spanish main language with a fifth content language (Italian)."""
    def change(data):
        data['mainLocale'] = 'es-ES'
        data['locales'] = [*data['locales'], 'it-IT']
    update_commerce(shop, change)


node, module = browser_driver()
log = artifact_path('browser-journeys/server.log').open('w')
server = serve(env, base, log)
try:
    shops = {label: register(label) for label in ('purchase', 'refund', 'edit', 'apps', 'language')}
    shops['refund']['order'] = provider_order(shops['refund'])
    spanish_main_language(shops['language'])
    with tempfile.NamedTemporaryFile('w', suffix='.json', delete=False) as handle:
        json.dump({'base': base, 'shops': shops}, handle)
        fixture_file = handle.name
    os.chmod(fixture_file, 0o600)
    try:
        subprocess.run([node, str(ROOT / 'scripts/testing/browser_journeys.mjs')], cwd=ROOT, check=True,
                       env={**env, 'BROWSER_MODULE': module, 'JOURNEY_FIXTURE': fixture_file,
                            'JOURNEY_ARTIFACTS': str(artifact_path('browser-journeys/screens').parent)})
    finally:
        os.unlink(fixture_file)
    assert 'refund' in provider_calls, 'The Studio refund never reached the provider'
    print('PASS browser journeys: purchase, refund, product edit while an order arrives, app installation, Spanish main language with Italian content')
finally:
    stop(server)
    fixture.shutdown()
