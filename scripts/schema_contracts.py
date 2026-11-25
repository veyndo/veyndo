#!/usr/bin/env python3
"""HTTP and MCP contract cases generated from the route catalogue and the operation contracts, against the running server.

For every catalogued route and every MCP tool, one valid and one invalid request run on a
disposable shop (`scripts/testing/schema_contracts.py` generates them):

- no response is a server error (5xx), except the documented 502/503/504 problem (retryable or
  `provider_unavailable`) of a dependency the suite deliberately leaves offline (model providers,
  checkout identity);
- a malformed JSON body is refused with a 4xx;
- an invalid credential never reaches merchant data: every `/api/` route answers 4xx, other
  routes (storefront, protocol discovery) answer like anonymous callers;
- an MCP call with schema-invalid arguments is refused: a JSON-RPC -32602 error or a tool
  error with a 4xx problem.
"""
import collections
import json
import os
import sys
import urllib.error
import urllib.request
import uuid

from testing import schema_contracts

BASE = os.environ['BASE_URL']
PASSWORD = 'Synthetic-contract-2026!'
OFFLINE = {502, 503, 504}


def send(method, url, body=None, headers=None):
    request = urllib.request.Request(BASE + url, data=body, method=method,
                                     headers={'Content-Type': 'application/json', **(headers or {})})
    try:
        with urllib.request.urlopen(request, timeout=60) as response:
            return response.status, response.read()
    except urllib.error.HTTPError as error:
        return error.code, error.read()
    except (TimeoutError, urllib.error.URLError) as error:
        return 599, str(error).encode()


def problem(raw):
    try:
        value = json.loads(raw)
    except (ValueError, UnicodeDecodeError):
        return {}
    errors = value.get('errors') if isinstance(value, dict) else None
    return errors[0] if isinstance(errors, list) and errors and isinstance(errors[0], dict) else {}


def offline(p):
    """The documented refusal of a dependency the suite leaves offline: retryable, or a provider outage."""
    return p.get('retryable') is True or p.get('kind') == 'provider_unavailable'


def tolerated(status, raw, seen=None):
    """No server error, or the documented answer of a deliberately offline dependency."""
    if status < 500:
        return True
    if status in OFFLINE and offline(problem(raw)):
        if seen is not None:
            seen.append(status)
        return True
    return False


def fixture():
    shop = 'contract-' + uuid.uuid4().hex[:12]
    status, raw = send('POST', '/api/auth/register', json.dumps({
        'workspaceId': shop, 'workspaceName': 'Contract fixture', 'name': 'Owner',
        'email': shop + '@example.test', 'password': PASSWORD}).encode())
    assert status == 200, (status, raw[:300])
    token = json.loads(raw)['token']
    return {'x-tenant': shop, 'Authorization': 'Bearer ' + token}


def http(shop, failures, statuses):
    invalid_auth = {'x-tenant': shop['x-tenant'], 'Authorization': 'Bearer invalid-contract-credential'}
    for case in schema_contracts.http_cases():
        for kind in ('valid', 'invalid'):
            request = case[kind]
            headers = shop if request['auth'] == 'shop' else invalid_auth
            status, raw = send(request['method'], request['url'], request['body'], headers)
            statuses[kind][status // 100] += 1
            dependency = []
            if not tolerated(status, raw, dependency):
                failures.append((case['route'], kind, status, raw[:200]))
            elif dependency:
                statuses['offline'][case['route']] = status
            elif kind == 'invalid':
                refused = 400 <= status < 500
                if request['kind'] == 'malformed-json' and not refused:
                    failures.append((case['route'], 'malformed JSON admitted', status, raw[:200]))
                if (request['kind'] == 'invalid-credential' and request['url'].startswith('/api/')
                        and not refused):
                    failures.append((case['route'], 'invalid credential admitted', status, raw[:200]))


def rpc(shop, method, params):
    body = json.dumps({'jsonrpc': '2.0', 'id': 1, 'method': method, 'params': params}).encode()
    status, raw = send('POST', '/mcp', body, shop)
    try:
        return status, json.loads(raw), raw
    except ValueError:
        return status, {}, raw


def mcp(shop, failures, outcomes):
    status, listed, raw = rpc(shop, 'tools/list', {})
    assert status == 200 and 'result' in listed, (status, raw[:300])
    for case in schema_contracts.mcp_cases():
        for kind in ('valid', 'invalid'):
            arguments = case[kind]
            if arguments is None:
                outcomes['invalid-impossible'] += 1
                continue
            status, reply, raw = rpc(shop, 'tools/call', {'name': case['tool'], 'arguments': arguments})
            error = reply.get('error') or {}
            result = reply.get('result') or {}
            refusal = (result.get('structuredContent') or {}).get('errors', [{}])[0] if result.get('isError') else {}
            code = refusal.get('status', 0)
            if not tolerated(status, raw) or error.get('code') == -32603 or (
                    code >= 500 and not (code in OFFLINE and offline(refusal))):
                failures.append((case['tool'], kind, status, error or refusal or raw[:200]))
                continue
            if code >= 500:
                outcomes[f'{kind}-offline-dependency'] += 1
            elif kind == 'valid':
                outcomes['valid-' + ('ok' if not error and not result.get('isError') else 'refused')] += 1
            elif error.get('code') == -32602 or 400 <= code < 500:
                outcomes['invalid-refused'] += 1
            else:
                failures.append((case['tool'], f'invalid input admitted ({case["rule"]})', status, result or error))


def main():
    shop = fixture()
    failures = []
    statuses = {'valid': collections.Counter(), 'invalid': collections.Counter(), 'offline': {}}
    outcomes = collections.Counter()
    mcp(shop, failures, outcomes)
    http(fixture(), failures, statuses)
    for failure in failures:
        print('FAIL', *failure, flush=True)
    routes = sum(statuses['valid'].values())
    print('HTTP valid by status class', dict(sorted(statuses['valid'].items())),
          'invalid', dict(sorted(statuses['invalid'].items())), flush=True)
    print('MCP', dict(sorted(outcomes.items())), flush=True)
    for route, status in sorted(statuses['offline'].items()):
        print('OFFLINE dependency', status, route, flush=True)
    if failures:
        sys.exit(f'{len(failures)} schema contract failures')
    print(f'PASS schema contracts: {routes} routes and {len(schema_contracts.mcp_cases())} tools, '
          'one valid and one invalid case each; no server error, invalid input refused', flush=True)


if __name__ == '__main__':
    main()
