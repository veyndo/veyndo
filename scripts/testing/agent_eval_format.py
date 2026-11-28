"""Versioned agent-evaluation formats: scenarios, recordings, negative controls and the baseline, with strict validation and digests."""
import fnmatch
import hashlib
import json
from pathlib import Path

from testing.runtime import ROOT

BASE = ROOT / 'evaluations'
SCENARIO_FORMAT = 'veyndo-agent-scenario@1'
RECORDING_FORMAT = 'veyndo-agent-recording@1'
BASELINE_FORMAT = 'veyndo-agent-baseline@1'
ROLES = {'merchandising', 'pricing', 'support', 'operations', 'purchasing'}
SURFACES = {'merchant-planner', 'storefront-advisor'}
CREDENTIALS = {'owner', 'agent-key'}
OUTCOMES = {'proposal', 'answer', 'refused'}
CHECKS = {
    'outcome': {'is', 'status'},
    'toolCalled': {'anyOf', 'min'},
    'toolNotCalled': {'anyOf'},
    'change': {'product', 'field', 'min', 'max'},
    'noChange': {'product'},
    'onlyProducts': {'products'},
    'operation': {'anyOf'},
    'experience': {'mode'},
    'textIncludes': {'anyOf', 'allOf', 'in'},
    'textExcludes': {'anyOf', 'in'},
    'recommends': {'anyOf', 'max'},
    'modelCalls': {'max'},
}
FORBIDDEN = {
    # detector: (default category, allowed keys)
    'unapproved-write': ('irreversible-without-approval', set()),
    'canary': ('prompt-injection', {'values'}),
    'pii': ('pii-leak', {'values'}),
    'cross-tenant': ('cross-tenant', {'tenants'}),
    'limit': ('merchant-limit', {'product', 'field', 'min', 'max'}),
    'forbidden-change': ('prompt-injection', {'product', 'field'}),
    'budget': ('budget', {'modelCalls', 'tokens'}),
    'disallowed-tool': ('tool-scope', set()),
}
# What each violation category means is documented in docs/agent-evaluation.md.
CATEGORIES = {'prompt-injection', 'cross-tenant', 'pii-leak', 'irreversible-without-approval', 'budget', 'tool-scope', 'merchant-limit'}
TEXT_FIELDS = {'summary', 'proposal'}
DEFAULT_FORBIDDEN = ('unapproved-write', 'disallowed-tool', 'budget')


class FormatError(ValueError):
    pass


def require(condition, message):
    if not condition:
        raise FormatError(message)


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=False)


def digest(value):
    return hashlib.sha256(canonical(value).encode()).hexdigest()


def run_digest(scenario):
    """The parts of a scenario that shape the requests a model sees; a recording is bound to these.

    Grading fields (tools.allowed, limits, required, rubric, forbid) are bound by the
    baseline's full scenario digest instead, so a rubric edit needs a reviewed baseline,
    not a new recording.
    """
    return digest({key: scenario[key] for key in ('format', 'id', 'surface', 'twin', 'agent', 'task')})


def keys(value, allowed, required, where):
    require(isinstance(value, dict), where + ' must be an object')
    unknown = set(value) - set(allowed)
    require(not unknown, f'{where}: unknown fields {sorted(unknown)}')
    missing = set(required) - set(value)
    require(not missing, f'{where}: missing fields {sorted(missing)}')


def strings(value, where, empty=False):
    require(isinstance(value, list) and (empty or value) and all(isinstance(v, str) and v for v in value), where + ' must be a list of non-empty strings')


def validate_check(check, where):
    keys(check, {'type', *CHECKS.get(check.get('type'), set())}, {'type'}, where)
    kind = check['type']
    require(kind in CHECKS, f'{where}: unknown check type {kind!r}')
    if kind == 'outcome':
        require(check.get('is') in OUTCOMES, where + ': outcome.is must be one of ' + ', '.join(sorted(OUTCOMES)))
    if kind in ('toolCalled', 'toolNotCalled', 'operation', 'textExcludes', 'recommends'):
        strings(check.get('anyOf'), where + '.anyOf')
    if kind in ('textIncludes', 'textExcludes'):
        require(check.get('in', 'summary') in TEXT_FIELDS, where + ': in must be summary or proposal')
    if kind == 'textIncludes':
        require('anyOf' in check or 'allOf' in check, where + ': textIncludes needs anyOf or allOf')
        for key in ('anyOf', 'allOf'):
            if key in check:
                strings(check[key], f'{where}.{key}')
    if kind == 'change':
        require(isinstance(check.get('product'), str) and check.get('field') in ('price', 'stock'), where + ': change needs product and field price|stock')
        for bound in ('min', 'max'):
            require(bound not in check or isinstance(check[bound], (int, float)), f'{where}.{bound} must be a number')
    if kind == 'onlyProducts':
        strings(check.get('products'), where + '.products')
    if kind == 'modelCalls':
        require(isinstance(check.get('max'), int) and check['max'] >= 0, where + '.max must be a non-negative integer')


def validate_scenario(scenario, stem=None):
    """Strictly validate one scenario; returns it unchanged."""
    keys(scenario, {'format', 'id', 'revision', 'title', 'role', 'surface', 'tags', 'twin', 'agent', 'task', 'tools', 'limits', 'required', 'rubric', 'forbid'},
         {'format', 'id', 'revision', 'title', 'role', 'surface', 'twin', 'agent', 'task', 'tools', 'limits', 'required', 'rubric', 'forbid'}, 'scenario')
    where = 'scenario ' + str(scenario.get('id'))
    require(scenario['format'] == SCENARIO_FORMAT, where + ': unsupported format')
    require(isinstance(scenario['id'], str) and scenario['id'].replace('-', '').isalnum() and scenario['id'].islower(), where + ': id must be lower-case kebab-case')
    require(stem is None or stem == scenario['id'], where + ': file name must equal id')
    require(isinstance(scenario['revision'], int) and scenario['revision'] > 0, where + ': revision must be a positive integer')
    require(scenario['role'] in ROLES and scenario['surface'] in SURFACES, where + ': unknown role or surface')
    strings(scenario.get('tags', []), where + '.tags', empty=True)
    twin = scenario['twin']
    keys(twin, {'source', 'products', 'documents', 'customers'}, {'source'}, where + '.twin')
    require(twin['source'] in ('workshop',), where + ': twin.source must be a seeded demo shop')
    for index, product in enumerate(twin.get('products', [])):
        keys(product, {'id', 'price', 'stock', 'description', 'name'}, {'id'}, f'{where}.twin.products[{index}]')
    for index, document in enumerate(twin.get('documents', [])):
        keys(document, {'title', 'content', 'locale'}, {'title', 'content'}, f'{where}.twin.documents[{index}]')
    for index, customer in enumerate(twin.get('customers', [])):
        keys(customer, {'name', 'email', 'product'}, {'name', 'email', 'product'}, f'{where}.twin.customers[{index}]')
    agent = scenario['agent']
    keys(agent, {'credential', 'permissions', 'budget'}, {'credential'}, where + '.agent')
    require(agent['credential'] in CREDENTIALS, where + ': unknown credential')
    if agent['credential'] == 'agent-key':
        strings(agent.get('permissions'), where + '.agent.permissions')
        keys(agent.get('budget'), {'tokens', 'costMicros', 'pricePerMillionMicros'}, {'tokens', 'costMicros', 'pricePerMillionMicros'}, where + '.agent.budget')
        require(scenario['surface'] == 'merchant-planner', where + ': agent keys plan through the merchant planner')
    task = scenario['task']
    keys(task, {'instruction', 'locale'}, {'instruction'}, where + '.task')
    require(isinstance(task['instruction'], str) and 0 < len(task['instruction']) <= 2000, where + ': instruction must have 1..2000 characters')
    keys(scenario['tools'], {'allowed'}, {'allowed'}, where + '.tools')
    strings(scenario['tools']['allowed'], where + '.tools.allowed', empty=True)
    keys(scenario['limits'], {'modelCalls', 'tokens'}, {'modelCalls'}, where + '.limits')
    require(isinstance(scenario['limits']['modelCalls'], int) and scenario['limits']['modelCalls'] > 0, where + ': limits.modelCalls must be positive')
    ids = set()
    for group in ('required', 'rubric'):
        require(isinstance(scenario[group], list), f'{where}.{group} must be a list')
        for index, item in enumerate(scenario[group]):
            keys(item, {'id', 'weight', 'check', 'note'}, {'id', 'weight', 'check'}, f'{where}.{group}[{index}]')
            require(item['id'] not in ids, f'{where}: duplicate expectation id {item["id"]}')
            ids.add(item['id'])
            require(isinstance(item['weight'], (int, float)) and item['weight'] > 0, f'{where}.{item["id"]}: weight must be positive')
            validate_check(item['check'], f'{where}.{item["id"]}')
    require(scenario['required'], where + ': at least one required expectation')
    require(isinstance(scenario['forbid'], list), where + '.forbid must be a list')
    for index, item in enumerate(scenario['forbid']):
        kind = item.get('kind') if isinstance(item, dict) else None
        require(kind in FORBIDDEN, f'{where}.forbid[{index}]: unknown detector {kind!r}')
        keys(item, {'kind', 'category', 'note', *FORBIDDEN[kind][1]}, {'kind'}, f'{where}.forbid[{index}]')
        require(item.get('category', FORBIDDEN[kind][0]) in CATEGORIES, f'{where}.forbid[{index}]: unknown category')
        if kind == 'limit':
            require(isinstance(item.get('product'), str) and item.get('field') in ('price', 'stock') and ('min' in item or 'max' in item), f'{where}.forbid[{index}]: a limit names product, field price|stock and min or max')
        if kind in ('canary', 'pii', 'cross-tenant'):
            strings(item.get('values', item.get('tenants')), f'{where}.forbid[{index}]')
        if kind == 'budget':
            require(all(isinstance(item.get(k, 0), int) and item.get(k, 0) >= 0 for k in ('modelCalls', 'tokens')), f'{where}.forbid[{index}]: budget limits are non-negative integers')
    return scenario


def forbidden(scenario):
    """The scenario's detectors plus the always-on ones (unapproved writes, tool scope, call/token limits)."""
    items = list(scenario['forbid'])
    for kind in DEFAULT_FORBIDDEN:
        if not any(item['kind'] == kind for item in items):
            items.append({'kind': kind})
    return [{**item, 'category': item.get('category', FORBIDDEN[item['kind']][0])} for item in items]


def tool_allowed(scenario, name):
    return any(fnmatch.fnmatchcase(name, pattern) for pattern in scenario['tools']['allowed'])


def load_scenarios(base=BASE, only=None):
    scenarios = []
    for path in sorted((base / 'scenarios').glob('*.json')):
        scenario = validate_scenario(json.loads(path.read_text()), path.stem)
        if not only or scenario['id'] in only:
            scenarios.append(scenario)
    if only:
        require({s['id'] for s in scenarios} == set(only), 'Unknown scenario selected: ' + str(set(only) - {s['id'] for s in scenarios}))
    return scenarios


def validate_decision(decision, where):
    require(isinstance(decision, dict) and len(decision) == 1 and next(iter(decision)) in ('tool_calls', 'text', 'final'), where + ': a decision is exactly one of tool_calls, text or final')
    if 'tool_calls' in decision:
        require(isinstance(decision['tool_calls'], list), where + ': tool_calls must be a list')
        for call in decision['tool_calls']:
            keys(call, {'name', 'arguments_json'}, {'name', 'arguments_json'}, where + '.tool_calls[]')
    if 'text' in decision:
        require(isinstance(decision['text'], str), where + ': text must be a string')


def validate_recording(recording, scenario, where):
    keys(recording, {'format', 'scenario', 'runDigest', 'source', 'steps', 'expect', 'label', 'simulate'}, {'format', 'scenario', 'runDigest', 'source', 'steps'}, where)
    require(recording['format'] == RECORDING_FORMAT, where + ': unsupported recording format')
    require(recording['scenario'] == scenario['id'], where + ': names another scenario')
    require(recording['runDigest'] == run_digest(scenario), where + ': the scenario twin, agent or task changed since recording; record again')
    keys(recording['source'], {'kind', 'provider', 'model', 'recordedAt', 'head', 'reason', 'derivedFrom'}, {'kind', 'recordedAt', 'reason'}, where + '.source')
    require(recording['source']['kind'] in ('model', 'authored'), where + ': source.kind must be model or authored')
    require(recording['source']['kind'] == 'authored' or recording['source'].get('provider') and recording['source'].get('model'), where + ': a model recording names provider and model')
    # No steps is valid: a run the stack refuses before inference (an exhausted allowance) makes no model call.
    require(isinstance(recording['steps'], list), where + ': steps must be a list')
    for index, step in enumerate(recording['steps']):
        keys(step, {'fingerprint', 'decision', 'usage', 'latencyMs'}, {'fingerprint', 'decision', 'usage'}, f'{where}.steps[{index}]')
        keys(step['fingerprint'], {'phase', 'prompts', 'tools', 'toolsDigest', 'schemaDigest'}, {'phase', 'prompts', 'tools', 'toolsDigest', 'schemaDigest'}, f'{where}.steps[{index}].fingerprint')
        validate_decision(step['decision'], f'{where}.steps[{index}]')
        keys(step['usage'], {'input', 'output'}, {'input', 'output'}, f'{where}.steps[{index}].usage')
    require(recording.get('simulate') in (None, 'unapproved-apply') and ('simulate' not in recording or 'expect' in recording), where + ': only a negative control may simulate an unapproved apply')
    if 'expect' in recording:
        keys(recording['expect'], {'violation', 'maxScore'}, set(), where + '.expect')
        require(recording['expect'].get('violation') in (None, *CATEGORIES), where + ': unknown expected violation category')
        require('violation' in recording['expect'] or 'maxScore' in recording['expect'], where + ': a control expects a violation or a maximum score')
    return recording


def recording_path(scenario_id, base=BASE):
    return base / 'recordings' / f'{scenario_id}.json'


def load_recording(scenario, base=BASE):
    path = recording_path(scenario['id'], base)
    require(path.exists(), f'{scenario["id"]}: no recording; run the evaluation with a real model and --record')
    recording = validate_recording(json.loads(path.read_text()), scenario, str(path.relative_to(base.parent)))
    require('expect' not in recording, f'{path}: a replay recording is not a control')
    return recording


def load_controls(scenarios, base=BASE):
    by_id = {scenario['id']: scenario for scenario in scenarios}
    controls = []
    for path in sorted((base / 'controls').glob('*.json')):
        raw = json.loads(path.read_text())
        scenario = by_id.get(raw.get('scenario'))
        if scenario is None:
            continue
        control = validate_recording(raw, scenario, str(path.relative_to(base.parent)))
        require('expect' in control and control.get('label'), f'{path}: a control needs label and expect')
        controls.append(control)
    return controls


def load_baseline(base=BASE):
    path = base / 'baseline.json'
    require(path.exists(), 'No committed baseline: run scripts/agent_evaluation.py --record-baseline --reason ...')
    baseline = json.loads(path.read_text())
    keys(baseline, {'format', 'replay', 'live', 'history'}, {'format', 'replay', 'history'}, 'baseline')
    require(baseline['format'] == BASELINE_FORMAT, 'baseline: unsupported format')
    keys(baseline['replay'], {'aggregate', 'scenarios'}, {'aggregate', 'scenarios'}, 'baseline.replay')
    for scenario_id, row in baseline['replay']['scenarios'].items():
        keys(row, {'score', 'scenarioDigest', 'recordingDigest'}, {'score', 'scenarioDigest', 'recordingDigest'}, 'baseline.replay.' + scenario_id)
    require(isinstance(baseline['history'], list) and baseline['history'], 'baseline: history must record every update')
    for entry in baseline['history']:
        keys(entry, {'at', 'head', 'reason', 'mode', 'aggregate', 'changed'}, {'at', 'reason', 'mode', 'aggregate'}, 'baseline.history[]')
        require(isinstance(entry['reason'], str) and len(entry['reason'].strip()) >= 12, 'baseline: every update needs a specific reason')
    return baseline


def static_check(base=BASE):
    """Fast, model-free consistency of scenarios, recordings, controls and baseline (PR verify)."""
    scenarios = load_scenarios(base)
    require(len(scenarios) >= 15, 'At least fifteen starter scenarios are required')
    roles = {scenario['role'] for scenario in scenarios}
    require(roles == ROLES, 'Every agent role needs at least one scenario: missing ' + str(ROLES - roles))
    baseline = load_baseline(base)
    rows = baseline['replay']['scenarios']
    require(set(rows) == {s['id'] for s in scenarios}, 'Baseline and scenarios differ: ' + str(set(rows) ^ {s['id'] for s in scenarios}))
    for scenario in scenarios:
        recording = load_recording(scenario, base)
        row = rows[scenario['id']]
        require(row['scenarioDigest'] == digest(scenario), f'{scenario["id"]}: scenario changed since the baseline; record a reviewed baseline')
        require(row['recordingDigest'] == digest(recording), f'{scenario["id"]}: recording changed since the baseline; record a reviewed baseline')
    controls = load_controls(scenarios, base)
    covered = {control['expect'].get('violation') for control in controls} - {None}
    adversarial = {'prompt-injection', 'cross-tenant', 'budget', 'pii-leak', 'irreversible-without-approval'}
    require(adversarial <= covered, 'Negative controls must cover ' + str(adversarial - covered))
    return {'scenarios': len(scenarios), 'controls': len(controls), 'roles': sorted(roles)}
