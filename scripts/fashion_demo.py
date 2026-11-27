#!/usr/bin/env python3
"""Actual default signup, fashion variants/media/localization, isolated checkout and restart; synthetic data only."""
from testing.database import psql
import hashlib
import json
import os
import socket
import subprocess
import urllib.error
import urllib.request
import uuid
from testing.runtime import ROOT, serve, stop

suffix = uuid.uuid4().hex[:10]
with socket.socket() as probe:
    probe.bind(('127.0.0.1', 0))
    port = probe.getsockname()[1]
base = f'http://127.0.0.1:{port}'
env = {**os.environ, 'BIND_ADDR': f'127.0.0.1:{port}', 'DEMO_CATALOG': 'fashion'}
checks = []

def request(path, body=None, headers=None, method=None, expected=200):
    req = urllib.request.Request(base + path, data=None if body is None else json.dumps(body).encode(),
        headers={'Content-Type': 'application/json', **(headers or {})}, method=method or ('GET' if body is None else 'POST'))
    try:
        with urllib.request.urlopen(req, timeout=45) as r:
            code, data = r.status, json.load(r)
    except urllib.error.HTTPError as e:
        code, data = e.code, json.load(e)
    assert code == expected, (path, code, expected, data)
    return data

def check(text):
    checks.append(text)
    print('PASS', text)

def register(label):
    return request('/api/auth/register', {'name': 'Fashion test', 'email': f'{label}-{suffix}@example.test',
        'password': 'Synthetic-fashion-2026!', 'workspaceId': f'{label}-{suffix}', 'workspaceName': 'Fashion demo'})

with (ROOT / 'artifacts/fashion-demo-server.log').open('w') as log:
    server = serve(env, base, log)
    try:
        one, two = register('fashion'), register('fashion-other')
        h = {'x-tenant': one['workspace']}
        other = {'x-tenant': two['workspace']}
        merchant = {**h, 'Authorization': 'Bearer ' + one['token']}
        items = request('/store-api/product', {'limit': 100}, h)['elements']
        assert len(items) == 12
        assert {p['id'] for p in items} == {'coat','knit','shirt','trousers','dress','blazer','tee','jeans','skirt','sneakers','bag','scarf'}
        assert all(p['media'] and p['extra']['demo']['synthetic'] for p in items)
        check('New merchant signup ships exactly 12 fashion products, with explicit generated-demo provenance')
        for locale, expected_name in [('de-DE','Harbor Wollmantel'),('en-GB','Harbor Wool Coat'),('fr-FR','Manteau en laine Harbor'),('es-ES','Abrigo de lana Harbor')]:
            detail = request('/store-api/product/coat', headers={**h, 'x-commerce-locale': locale})
            assert detail['product']['name'] == expected_name, detail
            assert len(detail['variants']) == 3
            assert {p['options']['size'] for p in detail['variants']} == {'S','M','L'}
        check('All four content languages and actual purchasable size variants are provisioned')
        nav = request('/store-api/navigation', headers={**h,'x-commerce-locale':'de-DE'})
        assert 'Jacken & Mäntel' in json.dumps(nav, ensure_ascii=False), nav
        for asset in json.loads((ROOT/'fixtures/fashion-artwork.json').read_text())['assets']:
            path = '/' + asset['path'].split('frontend/public/',1)[1]
            with urllib.request.urlopen(base + path) as r:
                content = r.read()
                assert r.headers['Content-Type'].startswith('image/webp')
            assert hashlib.sha256(content).hexdigest() == asset['sha256']
        check('Localized category navigation and all 12 real generated WebP assets are delivered over HTTP')
        editor = request('/api/merchant/products/coat', headers=merchant)
        editor['commerce']['price'] = 253
        saved = request('/api/merchant/products/coat', editor, merchant, method='PUT')
        assert saved['saved'] is True
        edited = request('/api/merchant/products/coat', headers=merchant)
        assert edited['commerce']['price'] == 253 and edited['extra']['demo']['synthetic']
        check('Fashion products remain fully editable with generated-demo provenance preserved')
        graph = request('/api/knowledge', headers=merchant)
        encoded = json.dumps(graph)
        assert 'coat' in encoded and 'scarf' in encoded and 'lamp' not in encoded
        check('Curated fashion knowledge links refer to the fashion catalog, with no furniture facts')
        cart = request('/store-api/checkout/cart', {'session': suffix}, h)
        ch = {**h, 'sw-context-token': cart['token']}
        cart = request('/store-api/checkout/cart/line-item', {'items':[{'referencedId':'coat-s','quantity':1}]}, ch)
        stock = request('/store-api/product/coat-s', headers=other)['product']['stock']
        order = request('/store-api/checkout/order', {}, {**ch,'Idempotency-Key':suffix})
        assert order['cart']['lineItems'][0]['referencedId'] == 'coat-s'
        assert request('/store-api/product/coat-s', headers=other)['product']['stock'] == stock
        assert request('/store-api/product/coat-s', headers=h)['product']['stock'] == stock-1
        assert request('/store-api/checkout/order', {}, {**ch,'Idempotency-Key':suffix})['id'] == order['id']
        check('A fashion size variant places one durable simulated order; another shop keeps its own stock')
        # Variant option values are merchant content: localized per language with main-language fallback.
        for locale, name in [('de-DE','Haferbeige'),('en-GB','Oatmeal'),('fr-FR','Avoine'),('es-ES','Avena')]:
            knit = request('/store-api/product/knit-s', headers={**h, 'x-commerce-locale': locale})
            assert knit['product']['extra']['optionLabels'] == {'color': name, 'size': 'S'}, knit['product']['extra']
            assert name in knit['product']['variantLabel'], knit['product']
        found = request('/store-api/product', {'limit': 24, 'search': 'haferbeige'}, {**h, 'x-commerce-locale': 'de-DE'})['elements']
        assert [p['id'] for p in found] == ['knit'], [p['id'] for p in found]
        assert not request('/store-api/product', {'limit': 24, 'search': 'haferbeige'}, {**h, 'x-commerce-locale': 'es-ES'})['elements']
        deh = {**h, 'x-commerce-locale': 'de-DE'}
        cart = request('/store-api/checkout/cart', {'session': suffix + '-de'}, deh)
        dch = {**deh, 'sw-context-token': cart['token']}
        cart = request('/store-api/checkout/cart/line-item', {'items':[{'referencedId':'knit-s','quantity':1}]}, dch)
        label = cart['lineItems'][0]['label']
        assert label.endswith(' · Haferbeige / S'), label
        placed = request('/store-api/checkout/order', {}, {**dch,'Idempotency-Key':suffix + '-de'})
        knit = request('/api/merchant/products/knit', headers=merchant)
        assert knit['extra']['optionValues']['color']['oatmeal']['de'] == 'Haferbeige'
        knit['extra']['optionValues']['color']['oatmeal'] = {**knit['extra']['optionValues']['color']['oatmeal'], 'it': 'Avena'}
        request('/api/merchant/products/knit', knit, merchant, method='PUT', expected=400)
        knit = request('/api/merchant/products/knit', headers=merchant)
        knit['extra']['optionValues']['color']['oatmeal']['de'] = 'Hafer'
        knit['extra']['optionValues']['color']['oatmeal']['es'] = None
        assert request('/api/merchant/products/knit', knit, merchant, method='PUT')['saved'] is True
        assert request('/store-api/product/knit-s', headers=deh)['product']['extra']['optionLabels']['color'] == 'Hafer'
        assert request('/store-api/product/knit-s', headers={**h, 'x-commerce-locale': 'es-ES'})['product']['extra']['optionLabels']['color'] == 'Oatmeal'
        saved_order = next(o for o in request('/api/search/order', {}, merchant)['data'] if o['id'] == placed['id'])
        assert saved_order['cart']['lineItems'][0]['label'] == label, saved_order['cart']['lineItems'][0]
        check('Option value names: four languages, main-language fallback, localized search and cart label, enabled-language validation, frozen order snapshot')
        # A merchant edit to the built-in demo must survive a fresh server process.
        subprocess.run(psql(os.environ['DB_CONTAINER'],'commerce',os.environ['TEST_DATABASE'],'-v','ON_ERROR_STOP=1','-c',"UPDATE products SET price=251 WHERE tenant='nord-atelier' AND id='coat'"), check=True, capture_output=True)
        stop(server)
        server = serve(env, base, log)
        assert request('/store-api/product/coat', headers={'x-tenant':'nord-atelier'})['product']['price'] == 251
        assert request('/store-api/product/coat-s', headers=h)['product']['stock'] == stock-1
        assert any(o['id']==order['id'] for o in request('/api/search/order', {}, merchant)['data'])
        check('Fresh-process restart preserves edited prices, reserved stock and orders instead of reseeding them')
    finally:
        stop(server)
print(json.dumps({'checks':len(checks),'realMoneyCharged':False,'products':12,'skus':34}))
