"""Generate one valid and one invalid request per HTTP route and per MCP tool from the route catalogue and the operation contracts.

Sources: `frontend/src/kit/api/generated/routes.ts` (written by `scripts/api_catalogue.py` from the
Rust router declarations) and `extensions/sdk/generated/operations.json` (written by
`scripts/testing/operation_contracts.py` from the compiled operation registry). The runner is
`scripts/schema_contracts.py`; `python3 scripts/testing/schema_contracts.py` checks the generator
itself, including negative controls.
"""
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
ROUTES = ROOT / 'frontend/src/kit/api/generated/routes.ts'
OPERATIONS = ROOT / 'extensions/sdk/generated/operations.json'
PROBE = 'contract-probe'
BODY_METHODS = {'POST', 'PUT', 'PATCH'}
# Routes that end the fixture's session or account run last, so earlier cases keep their credential.
# POST routes whose handler reads no request body: a malformed body is not an input they could refuse.
# The generator fails when one of them leaves the catalogue, so the list cannot go stale.
NO_BODY = {
    'POST /api/auth/handoff': 'issues a handoff ticket for the session',
    'POST /api/knowledge/reindex': 'queues the shop reindex',
    'POST /api/merchant/supply/proposal': 'computes the reorder proposal of the shop',
    'POST /api/search/order': 'lists orders like the GET form',
    'POST /channel-preview/end': 'ends the preview cookie',
    'POST /store-api/navigation': 'returns the navigation like the GET form',
}
SESSION_ENDING = re.compile(r'logout|sign-?out|sessions|revoke|password|delete-account|/account$|mfa|leave')


def routes():
    """Every catalogued route as (method, path, declaring file)."""
    found = re.findall(r'"([A-Z]+) ([^"]+)": "([^"]+)"', ROUTES.read_text())
    assert found, 'empty route catalogue'
    return [(method, path, source) for method, path, source in found]


def concrete(path):
    """The path with every parameter, including wildcards, replaced by the probe value."""
    return re.sub(r'\{\*?[^}]+\}', PROBE, path)


def http_cases():
    """Per route: a well-formed authenticated request and an invalid one.

    Valid: the probe path with the shop credential and, for body methods, an empty JSON object.
    Invalid: for body methods a malformed JSON body; otherwise, and for the listed routes that
    read no body, an invalid bearer credential.
    """
    cases = []
    for method, path, source in routes():
        url = concrete(path)
        valid = {'method': method, 'url': url, 'auth': 'shop', 'body': b'{}' if method in BODY_METHODS else None}
        if method in BODY_METHODS and f'{method} {path}' not in NO_BODY:
            invalid = {**valid, 'body': b'{"unterminated', 'kind': 'malformed-json'}
        else:
            invalid = {**valid, 'auth': 'invalid', 'kind': 'invalid-credential'}
        cases.append({'route': f'{method} {path}', 'source': source, 'valid': valid, 'invalid': invalid,
                      'late': bool(SESSION_ENDING.search(path))})
    cases.sort(key=lambda c: c['late'])
    return cases


def operations():
    data = json.loads(OPERATIONS.read_text())
    assert len(data['operations']) > 100, 'operation contracts missing'
    return data['operations']


def value(schema, depth=0):
    """A minimal value that satisfies the schema's type, enum, const and required properties."""
    if 'const' in schema:
        return schema['const']
    if schema.get('enum'):
        return schema['enum'][0]
    for key in ('oneOf', 'anyOf'):
        if schema.get(key):
            return value(schema[key][0], depth)
    kind = schema.get('type')
    if isinstance(kind, list):
        kind = next((k for k in kind if k != 'null'), 'null')
    if kind == 'object' or (kind is None and 'properties' in schema):
        props = schema.get('properties', {})
        return {k: value(props.get(k, {}), depth + 1) for k in schema.get('required', [])} if depth < 8 else {}
    if kind == 'array':
        n = schema.get('minItems', 0)
        return [value(schema.get('items', {}), depth + 1) for _ in range(n)]
    if kind == 'string':
        text = PROBE
        if schema.get('format') == 'date-time':
            text = '2026-01-01T00:00:00Z'
        if schema.get('format') == 'uri':
            text = 'https://example.test/' + PROBE
        if 'minLength' in schema:
            text = text.ljust(schema['minLength'], 'x')
        if 'maxLength' in schema:
            text = text[:schema['maxLength']]
        return text
    if kind in ('integer', 'number'):
        low = schema.get('minimum', schema.get('exclusiveMinimum', 0) + (1 if 'exclusiveMinimum' in schema else 0))
        return max(1, low) if schema.get('maximum', 1) >= 1 else low
    if kind == 'boolean':
        return False
    if kind == 'null':
        return None
    return {}


WRONG = {'string': 12345, 'integer': 'not-a-number', 'number': 'not-a-number', 'boolean': 'not-a-boolean',
         'array': 'not-a-list', 'object': 'not-an-object'}


def invalid(schema):
    """Arguments the schema refuses, with the rule they break; None when the schema admits every object."""
    props = schema.get('properties', {})
    required = schema.get('required', [])
    if required:
        return {}, f'missing required {required[0]}'
    if schema.get('additionalProperties') is False:
        return {'contractProbeUnexpected': True}, 'unknown property'
    for name, prop in props.items():
        kind = prop.get('type')
        if isinstance(kind, str) and kind in WRONG:
            return {name: WRONG[kind]}, f'{name} is not a {kind}'
    return None, 'schema admits every object'


def mcp_cases():
    cases = []
    for op in operations():
        schema = op['inputSchema']
        wrong, rule = invalid(schema)
        cases.append({'tool': op['name'], 'readOnly': op['readOnly'], 'valid': value(schema),
                      'invalid': wrong, 'rule': rule})
    return cases


def admits(schema, v, depth=0):
    """A small JSON-schema subset check used only by the generator's own negative controls."""
    if 'const' in schema:
        return v == schema['const']
    if schema.get('enum'):
        return v in schema['enum']
    kind = schema.get('type')
    kinds = kind if isinstance(kind, list) else [kind] if kind else []
    checks = {'object': lambda x: isinstance(x, dict), 'array': lambda x: isinstance(x, list),
              'string': lambda x: isinstance(x, str), 'boolean': lambda x: isinstance(x, bool),
              'integer': lambda x: isinstance(x, int) and not isinstance(x, bool),
              'number': lambda x: isinstance(x, (int, float)) and not isinstance(x, bool),
              'null': lambda x: x is None}
    if kinds and not any(checks[k](v) for k in kinds if k in checks):
        return False
    if isinstance(v, dict):
        props = schema.get('properties', {})
        if any(k not in v for k in schema.get('required', [])):
            return False
        if schema.get('additionalProperties') is False and any(k not in props for k in v):
            return False
        return all(admits(props[k], x, depth + 1) for k, x in v.items() if k in props)
    return True


def main():
    http = http_cases()
    tools = mcp_cases()
    assert len(http) == len(routes()) and all(c['valid'] and c['invalid'] for c in http)
    assert all('{' not in c['valid']['url'] for c in http), 'unsubstituted path parameter'
    catalogued = {f'{m} {p}' for m, p, _ in routes()}
    assert set(NO_BODY) <= catalogued, ('stale body-free routes', sorted(set(NO_BODY) - catalogued))
    schemas = {op['name']: op['inputSchema'] for op in operations()}
    for case in tools:
        schema = schemas[case['tool']]
        assert admits(schema, case['valid']), ('generated valid input refused', case['tool'], case['valid'])
        if case['invalid'] is not None:
            assert not admits(schema, case['invalid']), ('generated invalid input admitted', case['tool'])
    # Negative controls: the subset checker refuses what the generator must never produce.
    required = {'type': 'object', 'properties': {'id': {'type': 'string'}}, 'required': ['id']}
    assert not admits(required, {}) and not admits(required, {'id': 5}) and admits(required, {'id': 'x'})
    closed = {'type': 'object', 'properties': {}, 'additionalProperties': False}
    assert not admits(closed, {'x': 1}) and invalid(closed)[0] == {'contractProbeUnexpected': True}
    assert invalid({'type': 'object'})[0] is None
    assert concrete('/api/{id}/files/{*path}') == f'/api/{PROBE}/files/{PROBE}'
    without = sum(c['invalid'] is None for c in tools)
    print(f'PASS schema contract generator: {len(http)} routes and {len(tools)} tools, each with a valid case; '
          f'{len(tools) - without} tools with an invalid case ({without} admit every object); negative controls rejected')


if __name__ == '__main__':
    main()
