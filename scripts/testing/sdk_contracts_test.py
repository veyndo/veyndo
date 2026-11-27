"""Tooling suite: negative controls for the contract and SDK generators, then the drift check on the real tree."""
import copy
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts/sdks'))
import contracts  # noqa: E402
import model  # noqa: E402


def refused(call, needle):
    try:
        call()
    except (AssertionError, SystemExit) as error:
        assert needle in str(error), (needle, str(error))
        return
    raise AssertionError(f'accepted although it should fail with {needle!r}')


def ownership_controls():
    names = contracts.component_names()
    operations = [{'name': 'catalog.search'}]
    # A new route without an owner fails instead of landing in some component.
    rows = [{'path': '/api/unowned/thing', 'method': 'GET', 'source': 'src/x.rs', 'permission': None, 'handler': None}]
    refused(lambda: contracts.assign(rows, operations, names), 'no owner')
    # A handler that names a component crate must agree with the declared owner.
    rows = [{'path': '/api/merchant/products', 'method': 'GET', 'source': 'src/x.rs', 'permission': None,
             'handler': 'veyndo_orders::http::list'}]
    refused(lambda: contracts.assign(rows, operations, names), 'owned by catalog')
    # An operation without an owner fails.
    rows = [{'path': '/health', 'method': 'GET', 'source': 'src/x.rs', 'permission': None, 'handler': None}]
    refused(lambda: contracts.assign(rows, [{'name': 'unknown.tool'}], names), 'unknown.tool')
    # Prefixes match whole path segments: /api/apps does not own /api/appsx.
    assert contracts.longest_prefix({'/api/apps': 'apps'}, '/api/appsx', '/.') is None
    assert contracts.longest_prefix({'/api/apps': 'apps', '/api/apps/{id}/jobs': 'x'}, '/api/apps/{id}/jobs/{job}', '/.') == 'x'


def envelope_controls():
    problem, envelope = contracts.problem_schema()
    source = contracts.FAILURE.read_text()
    assert len(problem['properties']['kind']['enum']) == source.split('enum Kind {', 1)[1].split('}', 1)[0].count(',')
    assert {'kind', 'retryable', 'code', 'status', 'detail'} == set(problem['required'])
    assert envelope['properties']['errors']['items']['$ref'] == '#/components/schemas/Problem'


def contract_controls():
    docs = contracts.build()
    combined = docs['veyndo.openapi.json']
    # Every component document holds exactly its own routes, and together they hold all of them.
    per_component = sum(len(doc['paths'].get(p, {})) for name, doc in docs.items() if name != 'veyndo.openapi.json' for p in doc['paths'])
    assert per_component == sum(len(item) for item in combined['paths'].values())
    broken = copy.deepcopy(combined)
    broken['paths']['/health']['get']['responses']['default']['content']['application/json']['schema']['$ref'] = '#/components/schemas/Missing'
    refused(lambda: contracts.validate('broken', broken), 'unresolved')
    broken = copy.deepcopy(combined)
    broken['paths']['/api/apps/{id}'] = {'put': {**broken['paths']['/api/apps/{id}']['put'], 'parameters': []}}
    refused(lambda: contracts.validate('broken', broken), 'path parameters')
    # The digest a version is derived from ignores the version itself.
    bumped = copy.deepcopy(combined)
    bumped['info']['version'] = '9.9.9'
    assert contracts.contract_digest(bumped) == contracts.contract_digest(combined)
    # Identifiers stay valid in every target language.
    assert model.snake('type') == 'type_' and model.snake('categoryId') == 'category_id'
    assert contracts.operation_id('GET', '/store-api/product/{id}') == 'getStoreProductById'


def drift_control():
    target = ROOT / 'sdks/python/veyndo_sdk/components/catalog.py'
    original = target.read_text()
    try:
        target.write_text(original + '# hand edit\n')
        result = subprocess.run([sys.executable, 'scripts/sdks/generate.py', '--check'], cwd=ROOT, capture_output=True, text=True)
        assert result.returncode != 0 and 'catalog.py' in result.stderr, result.stderr
    finally:
        target.write_text(original)


ownership_controls()
envelope_controls()
contract_controls()
drift_control()
subprocess.run([sys.executable, 'scripts/sdks/generate.py', '--check'], cwd=ROOT, check=True)
print('PASS contract/SDK generators: ownership, envelope, structure and drift negative controls; committed output current')
