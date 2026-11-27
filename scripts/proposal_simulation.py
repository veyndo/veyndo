#!/usr/bin/env python3
"""Simulate-then-apply over actual HTTP/PostgreSQL with Rust HTTP and worker processes and a synthetic model (never live inference).

A price proposal is replayed in the shop twin (a rolled-back transaction) before review: the stored result has the
field diff, a projection from recorded orders, rule findings and an explanation. Apply uses the editor's clocks: a sale
in between does not make a price-only proposal stale, a content change does. A corridor violation is stored and
refused, a storefront change is named as not simulated, the audit history names the proposal, and MCP returns the
explanation with current-permission sources."""
import json
import os
import socket
import subprocess
import threading
import time
import urllib.error
import urllib.request
import uuid
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from testing.database import psql
from testing.native_tools import response as native_response
from testing.runtime import ROOT, artifact_path, serve, stop

env = dict(os.environ); processes = []; logs = []; checks = []
mode = {'value': None}


def port():
    with socket.socket() as s:
        s.bind(('127.0.0.1', 0)); return s.getsockname()[1]


def sql(statement):
    out = subprocess.check_output(psql(env['DB_CONTAINER'], 'commerce', env['TEST_DATABASE'], '-XqAt', '-v', 'ON_ERROR_STOP=1'),
                                  input="BEGIN; SELECT set_config('rac.system','on',true); " + statement + "; COMMIT;", text=True).strip().splitlines()
    return out[-1] if out else ''


class Model(BaseHTTPRequestHandler):
    """Synthetic provider: one authorized read round, then the configured proposal."""
    def log_message(self, *args): pass

    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
        if body.get('tools'):
            answered = any(isinstance(v, dict) and v.get('type') == 'function_call_output' for v in body.get('input', []))
            value = {'tool_calls': [] if answered else [{'name': 'merchant.commerce.read', 'arguments_json': '{"area":"aiPolicy"}'}]}
            answer = native_response(body, value)
        else:
            answer = {'status': 'completed', 'output': [{'type': 'message', 'content': [{'type': 'output_text', 'text': json.dumps(mode['value'])}]}],
                      'usage': {'input_tokens': 10, 'output_tokens': 20}}
        self.send_response(200); self.send_header('Content-Type', 'application/json'); self.end_headers()
        self.wfile.write(json.dumps(answer).encode())


provider = ThreadingHTTPServer(('127.0.0.1', 0), Model)
threading.Thread(target=provider.serve_forever, daemon=True).start()
env.update(OPENAI_API_KEY='synthetic-twin-model', OPENAI_BASE_URL=f'http://127.0.0.1:{provider.server_address[1]}/v1', BOOTSTRAP_MODE='serve')


def start(role):
    base = f'http://127.0.0.1:{port()}'
    log = artifact_path(f'proposal-simulation-{len(logs)}.log').open('w'); logs.append(log)
    child = {**env, 'PROCESS_ROLE': role, 'BIND_ADDR': base.removeprefix('http://')}
    if role.endswith('-worker'):
        process = subprocess.Popen([str(ROOT / 'target/debug/veyndo')], cwd=ROOT, env=child, stdout=log, stderr=log)
    else:
        process = serve(child, base, log)
    processes.append(process); return process, base


def call(base, path, body=None, h=None, expected=200, method=None):
    req = urllib.request.Request(base + path, data=None if body is None else json.dumps(body).encode(),
                                 headers={'Content-Type': 'application/json', **(h or {})}, method=method)
    try:
        with urllib.request.urlopen(req, timeout=30) as r: status = r.status; value = json.load(r)
    except urllib.error.HTTPError as error: status = error.code; value = json.load(error)
    assert status == expected, (path, status, expected, value)
    return value


def passed(name): checks.append(name); print('PASS', name, flush=True)


try:
    http, base = start('http')
    start('agent-worker')
    suffix = uuid.uuid4().hex[:12]
    account = call(base, '/api/auth/register', {'name': 'Twin fixture', 'email': suffix + '@example.test',
                                                'password': 'Synthetic-twin-2026!', 'workspaceId': 'twin-' + suffix})
    tenant = 'twin-' + suffix
    # Own fresh shop (registration seeds its demo lamp): neither the quota nor the one-run-per-shop worker slot
    # depends on earlier suites.
    sql(f"INSERT INTO tenant_resource_limits(tenant,daily_ai) VALUES('{tenant}',1000) ON CONFLICT(tenant) DO UPDATE SET daily_ai=1000")
    h = {'Authorization': 'Bearer ' + account['token'], 'x-tenant': tenant}

    def product():
        row = sql(f"SELECT json_build_object('price',price_decimal,'stock',stock,'revision',revision,'content',content_revision)::text FROM products WHERE tenant='{tenant}' AND id='lamp'")
        return json.loads(row)

    def history(reason=None):
        clause = f" AND reason='{reason}'" if reason else ''
        return int(sql(f"SELECT count(*) FROM entity_history WHERE tenant='{tenant}' AND entity='product' AND entity_id='lamp'{clause}"))

    def policy(value):
        snapshot = call(base, '/api/merchant/commerce', h=h); data = dict(snapshot['data']); data['aiPolicy'] = value
        call(base, '/api/merchant/commerce', {'revision': snapshot['revision'], 'data': data}, h, method='PUT')

    def propose(value, message):
        mode['value'] = {'summary': message, 'changes': [], 'experience': None, 'expected_experience_revision': None, **value}
        queued = call(base, '/api/agent/chat', {'message': message, 'background': True, 'requestId': uuid.uuid4().hex,
                                                 'inference': {'provider': 'openai'}}, h)
        run = queued['run']['taskId']
        for _ in range(200):
            status = call(base, f'/api/agent/runs/{run}', h=h)
            if status['state'] not in ('queued', 'running'): break
            time.sleep(.1)
        assert status['state'] == 'ready', status
        return run, status['preview']

    start_price = float(product()['price'])
    minor = round(start_price * 100)
    policy({'corridors': [{'productId': 'lamp', 'currency': 'EUR', 'minimumMinor': minor // 2, 'maximumMinor': minor * 2,
                           'minimumMarginBps': 0, 'unitCostNetMinor': minor // 4}],
            'autonomy': {'enabled': False, 'maxChangeBps': 500, 'productsPerDay': 1}})
    # A synthetic recorded order: three lamps in the trailing window feed the demand baseline.
    sql(f"""INSERT INTO carts(id,tenant,token,data,status) VALUES('twin-cart-{suffix}','{tenant}','twin-token-{suffix}','{{}}','ordered');
        INSERT INTO orders(id,tenant,cart_id,idempotency_key,fingerprint,data) VALUES('twin-order-{suffix}','{tenant}','twin-cart-{suffix}',
        'twin-key-{suffix}','fixture','{{"state":"placed","cart":{{"lineItems":[{{"id":"lamp","referencedId":"lamp","type":"product","quantity":3}}]}}}}')""")

    before = product(); history_before = history()
    target = round(start_price * 0.9, 2)
    run, preview = propose({'rationale': 'Synthetic: the corridor allows a ten percent reduction.', 'changes': [{'product_id': 'lamp', 'price': target}]},
                           'Lower the lamp price by ten percent.')
    s = preview['simulation']
    assert s['status'] == 'simulated' and s['twin'] == 'transactionalReplay' and not s['blocking'], s
    assert s['basis']['products'][0]['revision'] == before['revision'] and s['basis']['products'][0]['contentRevision'] == before['content'], s
    price_diff = next(d for d in s['diff'] if d['field'] == 'price')
    assert abs(price_diff['before'] - start_price) < 1e-9 and abs(price_diff['after'] - target) < 1e-9, s['diff']
    lamp = s['impact']['products'][0]
    assert lamp['demand'] == 'recorded' and lamp['windowUnits'] >= 3 and lamp['proposed']['units'] == lamp['baseline']['units'], lamp
    assert lamp['proposed']['grossRevenueMinor'] < lamp['baseline']['grossRevenueMinor'] and lamp['proposed']['marginMinor'] is not None, lamp
    assert 'priceElasticityNotModelled' in s['impact']['assumptions'] and s['impact']['horizonDays'] == 30
    assert any(r['rule'] == 'priceGuardrail' and r['outcome'] == 'passed' for r in s['rules']), s['rules']
    assert 'ruleBuilderConditions' in s['notChecked']
    assert product() == before and history() == history_before, 'The twin replay must be rolled back completely'
    passed('A price proposal is replayed in the twin before review: stored diff, projection from recorded orders, assumptions and passed guardrails; the shop is unchanged')

    e = preview['explanation']
    kinds = [a['kind'] for a in e['actions']]
    assert kinds == ['toolCall', 'proposal', 'simulation'], kinds
    proposal_action = e['actions'][1]
    assert proposal_action['rationale']['untrusted'] and 'ten percent' in proposal_action['rationale']['text']
    cited = [next(x for x in e['sources'] if x['id'] == i) for i in proposal_action['sourceIds']]
    assert any(x['type'] == 'product' and x['reference'] == 'lamp' and x['revision'] == before['revision'] for x in cited), cited
    assert any(x['type'] == 'rule' and x['reference'] == 'aiPolicy' for x in cited) and any(x['type'] == 'model' for x in cited), cited
    text = json.dumps(e)
    assert 'Lower the lamp price' not in text and 'synthetic-twin-model' not in text and account['token'] not in text
    passed('Each tool call, the proposal and the simulation record a rationale with source references; model text is marked untrusted and no instruction, credential or result is copied')

    # A sale between review and approval changes stock and the aggregate revision, not the content clock.
    sql(f"UPDATE products SET stock=stock-1,revision=revision+1 WHERE tenant='{tenant}' AND id='lamp'")
    sold = product()
    applied = call(base, f'/api/agent/tasks/{run}/apply', {'approve': True}, h)
    assert applied['applied'], applied
    after = product()
    assert abs(float(after['price']) - target) < 1e-9 and after['stock'] == sold['stock'], (sold, after)
    assert history(f'agent-proposal:{run}') >= 1, 'The audit history names the applied proposal'
    explained = call(base, '/mcp', {'jsonrpc': '2.0', 'id': 1, 'method': 'tools/call', 'params': {'name': 'merchant.run.explain', 'arguments': {'id': run}}}, h)
    content = explained['result']['structuredContent']
    assert content['applied'] and content['explanation']['actions'][-1]['kind'] == 'applied', content
    assert content['explanation']['actions'][-1]['rationale']['code'] == 'approvedAfterReview'
    assert call(base, f'/api/agent/runs/{run}/explanation', h=h)['explanation'] == content['explanation']
    passed('Apply keeps the editor clock rule (a sale in between is not stale and stock is preserved), the history names the proposal and MCP/HTTP return the applied explanation')

    # A content change after simulation makes the proposal stale; the conflict names the clock.
    run, preview = propose({'changes': [{'product_id': 'lamp', 'price': round(start_price * 0.95, 2)}]}, 'Set the lamp to 95 percent.')
    sql(f"UPDATE products SET description=description||' ',revision=revision+1 WHERE tenant='{tenant}' AND id='lamp'")
    stale = call(base, f'/api/agent/tasks/{run}/apply', {'approve': True}, h, expected=409)['errors'][0]
    assert stale['kind'] == 'revision_conflict' and stale.get('pointer') == '/contentRevision' and stale.get('currentRevision') == product()['content'], stale
    assert abs(float(product()['price']) - target) < 1e-9
    passed('A proposal is refused as stale after a content change, with the content clock pointer and current revision; the price stays')

    # A corridor violation becomes a stored blocking finding, and apply refuses it.
    run, preview = propose({'changes': [{'product_id': 'lamp', 'price': round(start_price * 0.3, 2)}]}, 'Sell the lamp at 30 percent.')
    s = preview['simulation']
    violated = next(r for r in s['rules'] if r['rule'] == 'priceGuardrail')
    assert s['blocking'] and violated['outcome'] == 'violated' and 'corridorMinimum' in violated['codes'], s
    refused = call(base, f'/api/agent/tasks/{run}/apply', {'approve': True}, h, expected=409)
    assert 'blocking rule' in json.dumps(refused), refused
    assert abs(float(product()['price']) - target) < 1e-9
    passed('A corridor violation is stored with its rule and code, shown before the decision, and apply refuses it without writing')

    # A storefront presentation change cannot be simulated: the twin says so and projects nothing.
    run, preview = propose({'experience': {'mode': 'discovery', 'headline': 'Synthetic twin headline'}}, 'Change the storefront headline.')
    s = preview['simulation']
    assert s['status'] == 'unsupported' and s['impact'] is None and s['diff'] == [], s
    assert s['parts'] == [{'kind': 'experience', 'status': 'unsupported', 'reason': 'noConversionModel'}], s['parts']
    passed('A proposal type the twin cannot simulate is stored as unsupported with its reason instead of a projection')

    other = call(base, '/api/auth/register', {'name': 'Other', 'email': 'other-' + suffix + '@example.test',
                                              'password': 'Synthetic-twin-2026!', 'workspaceId': 'other-' + suffix})
    call(base, f'/api/agent/runs/{run}/explanation', h={'Authorization': 'Bearer ' + other['token'], 'x-tenant': other['workspace']}, expected=404)
    passed('Another shop cannot read the explanation')
finally:
    for process in reversed(processes):
        stop(process)
    for log in logs:
        log.close()
    provider.shutdown()
report = {'passed': len(checks), 'checks': checks, 'liveModel': False}
print(json.dumps(report, indent=2))
