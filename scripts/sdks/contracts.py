#!/usr/bin/env python3
"""Generate the per-component and combined OpenAPI 3.1 contracts from the Rust router declarations, the compiled operation registry and the shared failure type.

Development tooling only. Inputs, all of them checked elsewhere against the compiled server:

* HTTP routes: the `.route(...)`/`.secure_route(...)` declarations in `src/` (the same scan as
  `scripts/api_catalogue.py`, whose `routeSources` this cross-checks), with the permission each
  `secure_route` registers;
* native MCP operations: `extensions/sdk/generated/operations.json`, which
  `scripts/testing/operation_contracts.py --check` keeps equal to `veyndo --operation-contracts`;
* the error envelope: the `Problem`/`Kind` types and `Failure::problem` in
  `components/core/src/failure.rs`;
* ownership: `contracts/ownership.json` (longest prefix wins), cross-checked against handlers
  that name a component crate explicitly;
* versions: `components.release.json` (see `scripts/component_mirrors.py --release`).

HTTP handlers take and return `Json<Value>` (167 of 180 bodies), and the workspace has no
`schemars`, so HTTP bodies are declared as open JSON. Typed request and response schemas come
from the operation registry: every native tool a component owns appears under
`components.schemas` and in `x-veyndo-tools`.

    python3 scripts/sdks/contracts.py            write contracts/http/*.openapi.json
    python3 scripts/sdks/contracts.py --check    fail on drift
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / 'contracts/http'
OWNERSHIP = ROOT / 'contracts/ownership.json'
OPERATIONS = ROOT / 'extensions/sdk/generated/operations.json'
FAILURE = ROOT / 'components/core/src/failure.rs'
ROUTE_CATALOGUE = ROOT / 'frontend/src/kit/api/generated/routes.ts'
RELEASE = ROOT / 'components.release.json'
METHODS = ('get', 'post', 'put', 'patch', 'delete', 'head', 'options')
DEFAULT_VERSION = '0.1.0'


def component_names():
    return list(json.loads((ROOT / 'components.json').read_text())['components'])


def crate_directories():
    """Rust crate identifier (veyndo_catalog) -> crate directory, for every workspace manifest."""
    found = {}
    for manifest in ROOT.glob('**/Cargo.toml'):
        if {'target', 'node_modules'} & set(manifest.relative_to(ROOT).parts):
            continue
        name = re.search(r'(?m)^name\s*=\s*"([^"]+)"', manifest.read_text())
        if name:
            found[name[1].replace('-', '_')] = manifest.parent.relative_to(ROOT).as_posix()
    return found


def owner_of_file(path, sources):
    for source, component in sources:
        if path == source or path.startswith(source.rstrip('/') + '/'):
            return component
    return None


def call_arguments(text, start):
    """The text inside the parentheses that open just before `start`, honouring strings."""
    depth, quoted, escaped, end = 1, False, False, start
    while end < len(text) and depth:
        c = text[end]
        if quoted:
            if escaped:
                escaped = False
            elif c == '\\':
                escaped = True
            elif c == '"':
                quoted = False
        elif c == '"':
            quoted = True
        elif c == '(':
            depth += 1
        elif c == ')':
            depth -= 1
        end += 1
    return text[start:end - 1]


def routes():
    """Every (path, method) the server declares, with source, permission and handler path."""
    files = [*(ROOT / 'src').rglob('*.rs')]
    # Same mount set as scripts/api_catalogue.py: native app routers compiled into the core.
    files += [ROOT / 'extensions/apps/site-builder' / name for name in ('context.rs', 'design_agent.rs')]
    rows = {}
    for file in sorted(files):
        if 'connectors' in file.relative_to(ROOT).parts:
            continue
        text = file.read_text()
        for match in re.finditer(r'\.(route|secure_route)\(\s*"([^"\n]+)"\s*,', text):
            expression = call_arguments(text, match.end())
            rights = {}
            if match[1] == 'secure_route':
                listed = re.match(r'\s*&\[(.*?)\]\s*,', expression, re.S)
                assert listed, f'{file}: secure_route {match[2]} without a rights list'
                rights = dict(re.findall(r'\(\s*"([A-Z]+)"\s*,\s*"([^"]+)"\s*\)', listed[1]))
                expression = expression[listed.end():]
            for method in re.finditer(r'\b(' + '|'.join(METHODS) + r')\s*\(\s*([A-Za-z_][\w:]*)?', expression):
                verb = method[1].upper()
                rows[(match[2], verb)] = {
                    'path': match[2], 'method': verb,
                    'source': file.relative_to(ROOT).as_posix(),
                    'permission': rights.get(verb) or (rights.get('GET') if verb == 'HEAD' else None),
                    'handler': method[2] or None,
                }
    return [rows[key] for key in sorted(rows)]


def longest_prefix(table, value, separators):
    best = None
    for prefix, component in table.items():
        exact = value == prefix
        nested = value.startswith(prefix) and (not separators or prefix[-1:] in separators or value[len(prefix):len(prefix) + 1] in separators)
        if (exact or nested) and (best is None or len(prefix) > len(best[0])):
            best = (prefix, component)
    return best[1] if best else None


def assign(rows, operations, names):
    table = json.loads(OWNERSHIP.read_text())
    assert set(table['routes'].values()) | set(table['operations'].values()) <= set(names), 'Ownership names an unknown component'
    crates = crate_directories()
    sources = sorted(((s, c) for c, r in json.loads((ROOT / 'components.json').read_text())['components'].items()
                      for s in r['sources']), key=lambda item: -len(item[0]))
    problems = []
    for row in rows:
        row['component'] = longest_prefix(table['routes'], row['path'], '/.')
        if not row['component']:
            problems.append(f"route {row['method']} {row['path']} ({row['source']}) has no owner in contracts/ownership.json")
            continue
        crate = (row['handler'] or '').split('::')[0]
        if crate.startswith('veyndo_') and crate in crates:
            declared = owner_of_file(crates[crate], sources)
            if declared not in (None, 'host', row['component']):
                problems.append(f"route {row['method']} {row['path']} is handled by {crate} ({declared}) but owned by {row['component']}")
    for operation in operations:
        operation['component'] = longest_prefix(table['operations'], operation['name'], '')
        if not operation['component']:
            problems.append(f"operation {operation['name']} has no owner in contracts/ownership.json")
    if problems:
        sys.exit('\n'.join(problems))


def problem_schema():
    """The JSON error envelope `Failure` serializes, read from components/core/src/failure.rs."""
    text = FAILURE.read_text()
    kinds_block = re.search(r'enum Kind \{(.*?)\}', text, re.S)[1]
    kinds = [re.sub(r'(?<!^)([A-Z])', r'_\1', name).lower() for name in re.findall(r'^\s*([A-Z]\w*),', kinds_block, re.M)]
    struct = re.search(r'struct Problem \{(.*?)\n\}', text, re.S)[1]
    properties, required = {}, []
    rust_types = {'bool': {'type': 'boolean'}, 'String': {'type': 'string'}, 'i64': {'type': 'integer'}, 'Kind': {'type': 'string', 'enum': kinds}}
    for attributes, name, rust in re.findall(r'((?:\s*#\[[^\]]*\]\s*)*)\s*(\w+):\s*([^,]+),', struct):
        key = re.sub(r'_([a-z])', lambda m: m[1].upper(), name)
        inner = re.fullmatch(r'Option<(\w+)>', rust.strip())
        properties[key] = dict(rust_types[(inner[1] if inner else rust.strip())])
        if 'skip_serializing_if' not in attributes and not inner:
            required.append(key)
    inserted = re.findall(r'fields\.insert\("(\w+)"\.into\(\), json!\(self\.([01])', text)
    assert {name for name, _ in inserted} == {'code', 'status', 'detail'}, 'Failure::problem changed; update the envelope reader'
    properties['code'] = {'type': 'string', 'description': 'HTTP status code as a string (compatible legacy field)'}
    properties['status'] = {'type': 'integer'}
    properties['detail'] = {'type': 'string'}
    required += ['code', 'status', 'detail']
    problem = {'type': 'object', 'description': 'One safe, actionable failure; never contains SQL or provider payloads.',
               'required': required, 'properties': properties}
    envelope = {'type': 'object', 'required': ['errors'], 'properties': {'errors': {'type': 'array', 'items': {'$ref': '#/components/schemas/Problem'}}}}
    return problem, envelope


def identifier(text):
    return ''.join(part[:1].upper() + part[1:] for part in re.split(r'[^A-Za-z0-9]+', text) if part)


def operation_id(method, path):
    """GET /api/merchant/products/{id} -> getMerchantProductsById; /store-api/... keeps a Store prefix."""
    segments = [s for s in path.split('/') if s]
    if segments[:1] == ['api']:
        segments = segments[1:]
    elif segments[:1] == ['store-api']:
        segments = ['store'] + segments[1:]
    words = []
    for segment in segments:
        param = re.fullmatch(r'\{\*?(\w+)\}', segment)
        words.append('By' + identifier(param[1]) if param else identifier(segment))
    return method.lower() + ''.join(words)


def tool_schema_name(name, suffix):
    return identifier(name) + suffix


def load_versions():
    """Component -> recorded version, and the SDK (combined contract) version; see scripts/component_releases.py."""
    data = json.loads(RELEASE.read_text()) if RELEASE.exists() else {}
    versions = {name: row['version'] for name, row in data.get('versions', {}).items()}
    return versions, (data.get('sdk') or {}).get('version', DEFAULT_VERSION)


def security_schemes():
    return {
        'personalSession': {'type': 'http', 'scheme': 'bearer', 'description': 'Personal merchant session token from POST /api/auth/login or /api/auth/register.'},
        'integrationKey': {'type': 'http', 'scheme': 'bearer', 'description': 'Scoped integration key from POST /api/workspace/integrations; intersects its creator\'s current membership.'},
    }


def path_operation(row):
    params = [{'name': name, 'in': 'path', 'required': True, 'schema': {'type': 'string'}}
              for name in re.findall(r'\{\*?(\w+)\}', row['path'])]
    params.append({'$ref': '#/components/parameters/Tenant'})
    operation = {
        'operationId': operation_id(row['method'], row['path']),
        'tags': [row['component']],
        'parameters': params,
        'responses': {
            '200': {'description': 'Success', 'content': {'application/json': {'schema': {}}}},
            'default': {'description': 'Failure envelope', 'content': {'application/json': {'schema': {'$ref': '#/components/schemas/ErrorEnvelope'}}}},
        },
        'x-veyndo-source': row['source'],
    }
    if row['method'] in ('POST', 'PUT', 'PATCH'):
        operation['requestBody'] = {'required': False, 'content': {'application/json': {'schema': {'type': 'object'}}}}
    if row['permission']:
        operation['x-veyndo-permission'] = row['permission']
        operation['security'] = [] if row['permission'] == 'public' else [{'personalSession': []}, {'integrationKey': []}]
    return operation


def document(title, version, rows, tools, problem, envelope, component=None):
    paths = {}
    for row in rows:
        paths.setdefault(row['path'], {})[row['method'].lower()] = path_operation(row)
    schemas = {'Problem': problem, 'ErrorEnvelope': envelope}
    listed = []
    for tool in tools:
        entry = {'name': tool['name'], 'description': tool['description'], 'readOnly': tool['readOnly'],
                 'input': {'$ref': '#/components/schemas/' + tool_schema_name(tool['name'], 'Input')}}
        # Typed planner facts from the operations graph (components/core/src/operation_facts.rs).
        for key in ('cost', 'requiresApproval', 'facts'):
            if key in tool:
                entry[key] = tool[key]
        schemas[tool_schema_name(tool['name'], 'Input')] = tool['inputSchema']
        if 'outputSchema' in tool:
            schemas[tool_schema_name(tool['name'], 'Output')] = tool['outputSchema']
            entry['output'] = {'$ref': '#/components/schemas/' + tool_schema_name(tool['name'], 'Output')}
        listed.append(entry)
    doc = {
        'openapi': '3.1.0',
        'info': {'title': title, 'version': version,
                 'description': 'Generated by scripts/sdks/contracts.py; do not edit. HTTP bodies are open JSON (the handlers take serde_json::Value); typed schemas are the native MCP tool contracts in x-veyndo-tools, called through POST /mcp.'},
        'servers': [{'url': 'https://api.veyndo.ai'}],
        'paths': paths,
        'components': {
            'schemas': schemas,
            'parameters': {'Tenant': {'name': 'x-tenant', 'in': 'header', 'required': False, 'schema': {'type': 'string'},
                                      'description': 'Shop ID. Required for shop-scoped calls without a session that already selects one.'}},
            'securitySchemes': security_schemes(),
        },
        'x-veyndo-tools': listed,
    }
    if component:
        doc['x-veyndo-component'] = component
    return doc


def contract_digest(doc):
    """Version-independent digest: the contract a version number is derived from."""
    stripped = json.loads(json.dumps(doc))
    stripped['info'].pop('version', None)
    return hashlib.sha256(json.dumps(stripped, sort_keys=True).encode()).hexdigest()


def cross_check(rows):
    """Our scan must list exactly the routes scripts/api_catalogue.py committed."""
    if not ROUTE_CATALOGUE.exists():
        return
    catalogue = set(re.findall(r'"([A-Z]+ /[^"]*)":', ROUTE_CATALOGUE.read_text()))
    ours = {row['method'] + ' ' + row['path'] for row in rows}
    assert catalogue == ours, f'Route scan differs from routes.ts: {sorted(catalogue ^ ours)[:10]}'


def build():
    """File name -> document for every component plus the combined `veyndo.openapi.json`."""
    names = component_names()
    rows = routes()
    cross_check(rows)
    registry = json.loads(OPERATIONS.read_text())
    operations = [dict(op) for op in registry['operations']]
    assign(rows, operations, names)
    ids = [operation_id(r['method'], r['path']) for r in rows]
    assert len(ids) == len(set(ids)), 'Colliding operationIds: ' + str({i for i in ids if ids.count(i) > 1})
    problem, envelope = problem_schema()
    versions, combined_version = load_versions()
    titles = {name: row['title'] for name, row in json.loads((ROOT / 'components.json').read_text())['components'].items()}
    docs = {}
    for name in names:
        docs[name + '.openapi.json'] = document(
            f'Veyndo {name}: {titles[name]}', versions.get(name, DEFAULT_VERSION),
            [r for r in rows if r['component'] == name], [o for o in operations if o['component'] == name],
            problem, envelope, name)
    combined = document('Veyndo Commerce API', combined_version, rows, operations, problem, envelope)
    combined['tags'] = [{'name': name, 'description': titles[name]} for name in names]
    docs['veyndo.openapi.json'] = combined
    return docs


def validate(name, doc):
    """Structural OpenAPI 3.1 checks the generator relies on: refs resolve, ids unique, path parameters declared."""
    def refs(value):
        if isinstance(value, dict):
            if isinstance(value.get('$ref'), str):
                yield value['$ref']
            for item in value.values():
                yield from refs(item)
        elif isinstance(value, list):
            for item in value:
                yield from refs(item)
    for ref in refs(doc):
        assert ref.startswith('#/'), f'{name}: external ref {ref}'
        node = doc
        for part in ref[2:].split('/'):
            assert isinstance(node, dict) and part in node, f'{name}: unresolved {ref}'
            node = node[part]
    ids = [op['operationId'] for item in doc['paths'].values() for op in item.values()]
    assert len(ids) == len(set(ids)), f'{name}: duplicate operationId'
    for path, item in doc['paths'].items():
        expected = set(re.findall(r'\{\*?(\w+)\}', path))
        for op in item.values():
            declared = {p['name'] for p in op['parameters'] if p.get('in') == 'path'}
            assert declared == expected, f'{name}: {path} path parameters {declared} != {expected}'


def render(doc):
    return json.dumps(doc, indent=2, ensure_ascii=False) + '\n'


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    docs = build()
    drift = []
    for name, doc in docs.items():
        validate(name, doc)
        path = OUT / name
        text = render(doc)
        if args.check:
            if not path.exists() or path.read_text() != text:
                drift.append(name)
        else:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text)
    stale = sorted(p.name for p in OUT.glob('*.openapi.json') if p.name not in docs) if OUT.exists() else []
    if args.check and (drift or stale):
        sys.exit('Contract drift: ' + ', '.join(drift + stale) + ' (run python3 scripts/sdks/contracts.py)')
    for name in stale:
        (OUT / name).unlink()
    combined = docs['veyndo.openapi.json']
    routes_count = sum(len(v) for v in combined['paths'].values())
    print(f'PASS contracts: {len(docs) - 1} components, {routes_count} route/method pairs, {len(combined["x-veyndo-tools"])} native tools')


if __name__ == '__main__':
    main()
