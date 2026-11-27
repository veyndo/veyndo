"""Agent-evaluation shop twins: one private sandbox per scenario through native APIs, scenario seeds, credentials, write fingerprints and real agent-surface runs."""
import json
import subprocess
import time
import urllib.error
import urllib.request
import uuid

from testing.database import psql

PASSWORD = 'Synthetic-Evaluation-2026!'
WATCHED = ('workshop', 'atelier')


class Shop:
    """The HTTP and SQL access of one evaluation server and its disposable database."""

    def __init__(self, base, env):
        self.base, self.env = base, env

    def call(self, path, body=None, headers=None, method=None, timeout=60):
        request = urllib.request.Request(self.base + path, data=None if body is None else json.dumps(body).encode(),
                                         headers={'Content-Type': 'application/json', **(headers or {})}, method=method)
        try:
            with urllib.request.urlopen(request, timeout=timeout) as response:
                return response.status, json.load(response)
        except urllib.error.HTTPError as error:
            raw = error.read()
            try:
                return error.code, json.loads(raw)
            except ValueError:
                return error.code, {'raw': raw.decode(errors='replace')[:400]}

    def ok(self, path, body=None, headers=None, method=None):
        status, value = self.call(path, body, headers, method)
        if status != 200:
            raise RuntimeError(f'{method or ("POST" if body is not None else "GET")} {path}: HTTP {status} {json.dumps(value)[:400]}')
        return value

    def sql(self, statement):
        output = subprocess.check_output(psql(self.env['TEST_DB_CONTAINER'], 'commerce', self.env['TEST_DATABASE'], '-XqAt', '-v', 'ON_ERROR_STOP=1'),
                                         input="BEGIN; SELECT set_config('rac.system','on',true); " + statement + '; COMMIT;', text=True)
        lines = output.strip().splitlines()
        return lines[-1] if lines else ''

    def owner(self):
        """A personal owner of the seeded shop and of a second seeded shop, so cross-shop isolation is tested with real access."""
        suffix = uuid.uuid4().hex[:12]
        account = self.ok('/api/auth/register', {'name': 'Evaluation owner', 'email': f'eval-{suffix}@example.test', 'password': PASSWORD, 'workspaceId': 'eval-' + suffix})
        actor = account['user']['id']
        for tenant in WATCHED:
            self.sql(f"INSERT INTO memberships(tenant,user_id,role) VALUES('{tenant}','{actor}','owner') ON CONFLICT DO NOTHING")
            self.sql(f"INSERT INTO tenant_resource_limits(tenant,daily_ai) VALUES('{tenant}',100000) ON CONFLICT(tenant) DO UPDATE SET daily_ai=100000")
        return {'Authorization': 'Bearer ' + account['token']}

    def fingerprint(self, tenant):
        """A digest of the merchant-visible state an agent could change: catalogue, orders, settings, experience and channels."""
        quoted = tenant.replace("'", "''")
        return self.sql(f"""SELECT md5(concat_ws('|',
            (SELECT coalesce(string_agg(concat_ws(':',id,price_decimal,stock,md5(row(name,description,category,properties,options,list_price_decimal,advanced_prices)::text)),',' ORDER BY id),'') FROM products WHERE tenant='{quoted}'),
            (SELECT count(*) FROM orders WHERE tenant='{quoted}'),
            (SELECT coalesce(md5(string_agg(data::text,',')),'') FROM commerce_settings WHERE tenant='{quoted}'),
            (SELECT coalesce(md5(string_agg(row(t.*)::text,',' ORDER BY t.product_id,t.language_id)),'') FROM product_translations t WHERE tenant='{quoted}'),
            (SELECT coalesce(md5(string_agg(row(e.*)::text,',')),'') FROM experiences e WHERE tenant='{quoted}')))""")


def build_twin(shop, owner, scenario):
    """A fresh private sandbox of the scenario's seeded shop with the scenario's catalogue edits and documents."""
    live = {**owner, 'x-tenant': scenario['twin']['source']}
    stage = shop.ok('/api/environments', {'name': 'Evaluation ' + scenario['id']}, live)['id']
    headers = {**owner, 'x-tenant': stage}
    for edit in scenario['twin'].get('products', []):
        product = shop.ok('/api/merchant/products/' + edit['id'], headers=headers)
        if 'price' in edit:
            product['commerce']['price'] = edit['price']
        if 'stock' in edit:
            product['commerce']['stock'] = edit['stock']
        if 'description' in edit or 'name' in edit:
            english = product['translations'].setdefault('en', {})
            english.update({k: edit[k] for k in ('description', 'name') if k in edit})
        shop.ok('/api/merchant/products/' + edit['id'], product, headers, 'PUT')
    for document in scenario['twin'].get('documents', []):
        shop.ok('/api/knowledge/documents', {'title': document['title'], 'content': document['content'], 'locale': document.get('locale', 'en-GB'), 'kind': 'manual'}, headers)
    return stage


def place_order(shop, owner, stage, customer):
    """A real storefront order of a synthetic customer in the twin, so private customer data exists to be protected."""
    public = {**owner, 'x-tenant': stage}
    cart = shop.ok('/store-api/checkout/cart', {'session': 'eval-' + uuid.uuid4().hex[:8]}, public)
    context = {**public, 'sw-context-token': cart['token']}
    shop.ok('/store-api/checkout/cart/line-item', {'items': [{'referencedId': customer['product'], 'quantity': 1}]}, context)
    shop.ok('/store-api/account/register', {'name': customer['name'], 'email': customer['email'], 'password': PASSWORD}, public)
    login = shop.ok('/store-api/account/login', {'email': customer['email'], 'password': PASSWORD}, context)
    context['sw-context-token'] = login['token']
    return shop.ok('/store-api/checkout/order', {}, {**context, 'Idempotency-Key': 'eval-' + uuid.uuid4().hex})


def credential(shop, owner, scenario, stage, choice):
    """The scenario's acting credential in the twin and, for a delegated agent, its key ID."""
    agent = scenario['agent']
    if agent['credential'] == 'owner':
        return {**owner, 'x-tenant': stage}, None
    budget = {'provider': choice['provider'], 'model': choice['model'], **agent['budget']}
    key = shop.ok('/api/workspace/integrations', {'name': 'Evaluation ' + scenario['id'], 'permissions': agent['permissions'], 'expiresInDays': 1, 'agentBudget': budget},
                  {**owner, 'x-tenant': scenario['twin']['source']})
    return {'Authorization': 'Bearer ' + key['key'], 'x-tenant': stage}, key['id']


def agent_usage(shop, owner, scenario, key_id):
    if key_id is None:
        return None
    listed = shop.ok('/api/workspace/integrations', headers={**owner, 'x-tenant': scenario['twin']['source']})
    for item in listed.get('integrations', listed.get('keys', [])) if isinstance(listed, dict) else listed:
        if item.get('id') == key_id:
            return item.get('agentUsage')
    return None


def run_planner(shop, headers, scenario, choice, timeout):
    """One durable merchant-planner run through the native queue; never approved."""
    locale = scenario['task'].get('locale', 'en-GB')
    request = {'instruction': scenario['task']['instruction'], 'inference': choice, 'requestId': uuid.uuid4().hex}
    status, created = shop.call('/api/agent/runs', request, {**headers, 'x-commerce-locale': locale})
    if status != 200:
        return {'outcome': 'refused', 'status': status, 'error': created, 'text': json.dumps(created)[:600], 'proposal': None, 'toolTrace': []}
    deadline = time.monotonic() + timeout
    while True:
        status, run = shop.call('/api/agent/runs/' + created['taskId'], headers=headers)
        if status != 200 or run.get('state') not in ('queued', 'running') or time.monotonic() > deadline:
            break
        time.sleep(0.2)
    if status != 200 or run.get('state') != 'ready':
        error = run.get('error') or {}
        return {'outcome': 'refused', 'status': error.get('status', status), 'state': run.get('state'), 'error': error,
                'text': json.dumps(error)[:600], 'proposal': None, 'toolTrace': []}
    preview = run.get('preview') or {}
    proposal = preview.get('proposal') or {}
    acting = any(proposal.get(key) for key in ('changes', 'operations', 'app_action', 'experience', 'translation', 'translation_products', 'navigation'))
    return {'outcome': 'proposal' if acting else 'answer', 'status': 200, 'state': 'ready', 'approvalRequired': run.get('approvalRequired'), 'taskId': created['taskId'],
            'text': proposal.get('summary', ''), 'proposal': proposal,
            'toolTrace': [{'tool': t.get('tool'), 'input': t.get('input')} for t in preview.get('toolTrace', [])]}


def run_advisor(shop, owner, stage, scenario, timeout):
    """One read-only storefront advisor answer in the twin (a private sandbox needs its merchant's preview login)."""
    headers = {**owner, 'x-tenant': stage, 'x-commerce-locale': scenario['task'].get('locale', 'en-GB')}
    status, value = shop.call('/api/concierge', {'request': scenario['task']['instruction']}, headers, timeout=timeout)
    if status != 200:
        return {'outcome': 'refused', 'status': status, 'error': value, 'text': json.dumps(value)[:600], 'proposal': None, 'toolTrace': []}
    answer = value.get('answer') or {}
    return {'outcome': 'answer', 'status': 200, 'text': answer.get('explanation', ''), 'recommended': answer.get('recommended_ids', []),
            'proposal': None, 'toolTrace': [{'tool': t.get('tool'), 'input': t.get('input')} for t in value.get('toolTrace', [])]}


def evaluate(shop, owner, gateway, scenario, choice, steps, timeout, simulate=None):
    """Prepare the twin, run the agent surface once and collect everything grading needs.

    `simulate='unapproved-apply'` exists only for a negative control: it applies the
    proposal through the native endpoint before the write check, as a broken agent
    stack that skipped human approval would, to prove the detector sees real writes.
    """
    stage = build_twin(shop, owner, scenario)
    for customer in scenario['twin'].get('customers', []):
        place_order(shop, owner, stage, customer)
    headers, key_id = credential(shop, owner, scenario, stage, choice)
    watched = (stage, *WATCHED)
    before = {tenant: shop.fingerprint(tenant) for tenant in watched}
    gateway.begin(scenario['id'], steps, redact=[stage])
    started = time.monotonic()
    try:
        if scenario['surface'] == 'merchant-planner':
            run = run_planner(shop, headers, scenario, choice, timeout)
        else:
            run = run_advisor(shop, owner, stage, scenario, timeout)
    finally:
        calls = gateway.finish()
    if simulate == 'unapproved-apply' and run.get('taskId'):
        shop.ok(f"/api/agent/tasks/{run['taskId']}/apply", {'approve': True}, {**owner, 'x-tenant': stage})
    run.update(calls=calls, wallMs=round((time.monotonic() - started) * 1000), provider=choice['provider'], model=choice['model'],
               writes={'before': before, 'after': {tenant: shop.fingerprint(tenant) for tenant in watched}},
               agentUsage=agent_usage(shop, owner, scenario, key_id), twin=stage)
    release_slot(shop, stage)
    return run


def release_slot(shop, stage):
    """Free the sandbox slot (ten per shop) in this disposable database; the product has no sandbox deletion endpoint."""
    shop.sql(f"DELETE FROM shop_environments WHERE tenant='{stage}'")
