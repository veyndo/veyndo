"""Smoke test of the generated Python SDK against a running Veyndo server (BASE_URL); synthetic accounts only.

Covers both credentials, cursor pagination, the typed failure envelope on HTTP and MCP, a typed
tool input and a JSON-RPC protocol error. Run by scripts/sdk_smoke.py.
"""
import os
from pathlib import Path
import sys
import uuid

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from veyndo_sdk import Auth, McpProtocolError, VeyndoClient, VeyndoError, paginate  # noqa: E402
from veyndo_sdk.tools import CatalogSearchInput, MerchantProductsInput  # noqa: E402

BASE = os.environ['BASE_URL']
SHOP = os.environ.get('SDK_SMOKE_SHOP', 'atelier')


def expect_error(call, status, kind):
    try:
        call()
    except VeyndoError as error:
        assert error.status == status and error.kind == kind, (error.status, error.kind, error.problems)
        assert error.problems and error.problems[0].detail, error.body
        return error
    raise AssertionError(f'expected HTTP {status} {kind}')


anonymous = VeyndoClient(BASE)
assert anonymous.host.get_health()['status'] == 'ok'
expect_error(lambda: anonymous.identity.get_auth_session(), 401, 'authentication_required')

store = anonymous.with_options(tenant=SHOP)
complete = store.catalog.post_store_product(body={'limit': 100})['elements']
paged = list(paginate(lambda cursor: store.catalog.post_store_product(body={'limit': 2, **({'after': cursor} if cursor else {})})))
assert [p['id'] for p in paged] == [p['id'] for p in complete] and len(paged) > 2, (len(paged), len(complete))
expect_error(lambda: store.catalog.post_store_product(body={'limit': 0}), 400, 'validation_error')

found = store.mcp.call(CatalogSearchInput(limit=2))
assert len(found['elements']) == 2, found
invalid = expect_error(lambda: store.mcp.call(CatalogSearchInput(limit=0)), 400, 'validation_error')
assert invalid.problems[0].pointer == '/limit', invalid.problems
expect_error(lambda: store.mcp.call('merchant.products', {}), 403, 'permission_denied')
try:
    store.mcp.call('no.such.tool', {})
    raise AssertionError('an unknown tool must be a protocol error')
except McpProtocolError as error:
    assert error.code == -32602, error.code

suffix = uuid.uuid4().hex[:12]
account = anonymous.identity.post_auth_register(body={
    'email': f'sdk-python-{suffix}@example.test', 'name': 'SDK smoke', 'password': 'Synthetic-sdk-2026!',
    'workspaceId': f'sdk-python-{suffix}'})
personal = anonymous.with_options(auth=Auth.personal_session(account['token']), tenant=account['workspace'])
assert personal.identity.get_auth_session()['user']['email'] == f'sdk-python-{suffix}@example.test'
key = personal.identity.post_workspace_integrations(body={'name': 'sdk-smoke', 'permissions': ['catalog.read'], 'expiresInDays': 1})
integration = anonymous.with_options(auth=Auth.integration_key(key['key']), tenant=account['workspace'])
assert 'merchant.products' in {tool['name'] for tool in integration.mcp.list_tools()}
assert isinstance(integration.mcp.call(MerchantProductsInput(limit=5))['elements'], list)
expect_error(lambda: integration.identity.get_workspace_integrations(), 403, 'permission_denied')
print('PASS Python SDK: health, store pagination, typed HTTP/MCP failures, personal session, integration key and MCP tools')
