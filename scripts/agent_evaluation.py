#!/usr/bin/env python3
"""Agent evaluation benchmark: versioned scenarios against the real agent stack in fresh shop twins, replayed or real models, scored and gated against a reviewed baseline."""
import argparse
import datetime
import json
import os
import shutil
import socket
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from testing.runtime import ROOT, artifact_path, serve, stop  # noqa: E402
from testing import agent_eval_format as fmt  # noqa: E402
from testing.agent_eval_gateway import Gateway  # noqa: E402
from testing.agent_eval_grading import aggregate, gate, grade  # noqa: E402
from testing.agent_eval_twin import Shop, evaluate  # noqa: E402

AUTHORED = ('ollama', 'veyndo-authored-reference')
PROVIDERS = ('ollama', 'openai', 'anthropic')
RESULT_FORMAT = 'veyndo-agent-evaluation-result@1'


def arguments(argv):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--model', default=os.environ.get('AGENT_EVAL_MODEL', 'replay'),
                        help='replay (default, deterministic) or provider[:model] for a real model: ollama, openai, anthropic')
    parser.add_argument('--only', nargs='+', help='Selected scenario IDs')
    parser.add_argument('--record', action='store_true', help='With a real model: keep each run as a candidate recording under artifacts/')
    parser.add_argument('--no-controls', action='store_true', help='Skip the negative controls (not allowed for the gate)')
    parser.add_argument('--check', action='store_true', help='Fast model-free format, recording and baseline consistency (PR verify)')
    parser.add_argument('--record-baseline', action='store_true', help='Write the latest result artifact into evaluations/baseline.json')
    parser.add_argument('--adopt', nargs='+', metavar='ID', help='Copy candidate recordings of the latest real-model run into evaluations/recordings ("all" for every clean one)')
    parser.add_argument('--refresh-authored', action='store_true', help='Replay authored recordings and controls and rewrite their request fingerprints and scenario digests')
    parser.add_argument('--summary', choices=('replay', 'live'), help='Print the latest result artifact as a Markdown table')
    parser.add_argument('--include-model', action='store_true', help='With --refresh-authored: also refresh model recordings, which then become authored references derived from that model')
    parser.add_argument('--reason', help='Why the baseline, adopted recordings or refreshed fingerprints changed (required for those steps)')
    parser.add_argument('--container', default=os.environ.get('TEST_DB_CONTAINER', 'veyndo-postgres-1'))
    return parser.parse_args(argv)


def now():
    return datetime.datetime.now(datetime.timezone.utc).replace(microsecond=0).isoformat()


def head():
    try:
        return subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
    except (OSError, subprocess.CalledProcessError):
        return None


def choice_of(model):
    provider, _, name = model.partition(':')
    if provider not in PROVIDERS:
        raise SystemExit(f'Unknown model {model!r}: use replay or {", ".join(PROVIDERS)}[:model]')
    defaults = {'ollama': os.environ.get('AGENT_EVAL_OLLAMA_MODEL', 'qwen3.6:35b'), 'openai': os.environ.get('AGENT_EVAL_OPENAI_MODEL', 'gpt-6-sol'),
                'anthropic': os.environ.get('AGENT_EVAL_ANTHROPIC_MODEL', 'claude-sonnet-5-5')}
    return {'provider': provider, 'model': name or defaults[provider]}


def upstreams():
    """Real provider endpoints for the gateway. Their keys stay in this process; the server under test only sees a synthetic key."""
    routes = {'ollama': (os.environ.get('AGENT_EVAL_OLLAMA_URL', 'http://127.0.0.1:11434'), None)}
    if os.environ.get('AGENT_EVAL_OPENAI_API_KEY'):
        routes['openai'] = (os.environ.get('AGENT_EVAL_OPENAI_BASE_URL', 'https://api.openai.com/v1'), {'Authorization': 'Bearer ' + os.environ['AGENT_EVAL_OPENAI_API_KEY']})
    if os.environ.get('AGENT_EVAL_ANTHROPIC_API_KEY'):
        routes['anthropic'] = (os.environ.get('AGENT_EVAL_ANTHROPIC_BASE_URL', 'https://api.anthropic.com/v1'),
                               {'x-api-key': os.environ['AGENT_EVAL_ANTHROPIC_API_KEY'], 'anthropic-version': '2023-06-01'})
    return routes


def server_env(choice, gateway):
    env = dict(os.environ)
    with socket.socket() as probe:
        probe.bind(('127.0.0.1', 0))
        base = f'http://127.0.0.1:{probe.getsockname()[1]}'
    env.update(BIND_ADDR=base.removeprefix('http://'), BOOTSTRAP_MODE='serve', PROCESS_ROLE='all', DB_POOL_MAX='4', DB_POOL_MIN='0',
               INFERENCE_ALLOW_LOOPBACK='true', OLLAMA_URL=gateway.url, OPENAI_BASE_URL=gateway.url + '/v1', ANTHROPIC_BASE_URL=gateway.url + '/v1',
               OPENAI_API_KEY='synthetic-evaluation-gateway', ANTHROPIC_API_KEY='synthetic-evaluation-gateway')
    env.pop('OPENAI_PROTOCOL', None)
    env[{'ollama': 'OLLAMA_MODEL', 'openai': 'OPENAI_MODEL', 'anthropic': 'ANTHROPIC_MODEL'}[choice['provider']]] = choice['model']
    return base, env


def run_group(choice, items, mode, timeout, lenient=False):
    """One server for one provider/model: every (scenario, steps, simulate) item in its own fresh twin."""
    gateway = Gateway(mode, upstreams(), lenient=lenient)
    base, env = server_env(choice, gateway)
    shop = Shop(base, env)
    # The storefront advisor uses the platform default route; point it at the provider under test.
    shop.sql(f"UPDATE platform_ai SET data=jsonb_set(data,'{{defaultProvider}}',to_jsonb('{choice['provider']}'::text)) WHERE id=true")
    runs = []
    with artifact_path(f'agent-evaluation/server-{choice["provider"]}.log').open('a') as log:
        server = serve(env, base, log)
        try:
            owner = shop.owner()
            for scenario, steps, simulate in items:
                print(f'RUN {scenario["id"]} ({choice["provider"]}:{choice["model"]})', flush=True)
                try:
                    runs.append(evaluate(shop, owner, gateway, scenario, choice, steps, timeout, simulate))
                except (RuntimeError, OSError, ValueError, KeyError) as error:
                    # A harness failure is an ungraded result, never a pass; the run continues with the next scenario.
                    calls = gateway.finish()
                    runs.append({'outcome': 'error', 'status': None, 'text': f'{type(error).__name__}: {error}', 'proposal': None, 'toolTrace': [],
                                 'calls': calls, 'writes': {'before': {}, 'after': {}}, 'agentUsage': None, 'provider': choice['provider'],
                                 'model': choice['model'], 'wallMs': None, 'harnessError': str(error)})
                    print(f'ERROR {scenario["id"]}: {error}', flush=True)
        finally:
            stop(server)
            gateway.close()
    return runs


def replay_choice(recording):
    source = recording['source']
    return (source['provider'], source['model']) if source['kind'] == 'model' else AUTHORED


def steps_of(recording):
    return [{**step, 'decision': {'text': json.dumps(step['decision']['final'], ensure_ascii=False)} if 'final' in step['decision'] else step['decision']} for step in recording['steps']]


def write_artifact(document, label):
    path = artifact_path(f'agent-evaluation/{label}.json')
    path.write_text(json.dumps(document, indent=1, ensure_ascii=False) + '\n')
    shutil.copyfile(path, artifact_path(f'agent-evaluation/latest-{document["mode"] if document["mode"] == "replay" else "live"}.json'))
    return path


def report(results, summary):
    for result in results:
        flags = ' '.join(f"[{v['category']}]" for v in result['violations']) or ('STALE' if result['status'] == 'stale' else '')
        print(f"SCORE {result['scenario']:<34} {str(result['score']):>6}  calls {result['modelCalls']}  tokens {result['tokens']['input']}+{result['tokens']['output']}  {flags}", flush=True)
    print('AGGREGATE ' + json.dumps(summary), flush=True)


def evaluate_replay(args, scenarios, pricing):
    refresh = args.refresh_authored
    recordings, controls = {}, []
    for scenario in scenarios:
        path = fmt.recording_path(scenario['id'])
        raw = json.loads(path.read_text()) if path.exists() else None
        if refresh and raw and (raw['source']['kind'] == 'authored' or args.include_model):
            raw['runDigest'] = fmt.run_digest(scenario)
            recordings[scenario['id']] = fmt.validate_recording(raw, scenario, str(path))
        else:
            recordings[scenario['id']] = fmt.load_recording(scenario)
    if not args.no_controls:
        by_id = {s['id']: s for s in scenarios}
        for path in sorted((fmt.BASE / 'controls').glob('*.json')):
            raw = json.loads(path.read_text())
            if raw.get('scenario') not in by_id:
                continue
            if refresh:
                raw['runDigest'] = fmt.run_digest(by_id[raw['scenario']])
            control = fmt.validate_recording(raw, by_id[raw['scenario']], str(path))
            control['_path'] = path
            controls.append(control)
    groups = {}
    for scenario in scenarios:
        groups.setdefault(replay_choice(recordings[scenario['id']]), []).append(('scenario', scenario, recordings[scenario['id']]))
    for control in controls:
        scenario = next(s for s in scenarios if s['id'] == control['scenario'])
        groups.setdefault(replay_choice(control), []).append(('control', scenario, control))
    results, checks, unrefreshed = [], [], []
    for (provider, model), items in groups.items():
        choice = {'provider': provider, 'model': model}
        lenient = refresh and all(item[2]['source']['kind'] == 'authored' or args.include_model for item in items)
        runs = run_group(choice, [(scenario, steps_of(recording), recording.get('simulate')) for _, scenario, recording in items], 'replay', 180, lenient=lenient)
        for (kind, scenario, recording), run in zip(items, runs):
            result = grade(scenario, run, pricing)
            if refresh and (recording['source']['kind'] == 'authored' or args.include_model):
                problem = refresh_fingerprints(recording, run, scenario, args.reason, kind)
                if problem:
                    unrefreshed.append(problem)
            if kind == 'scenario':
                result['recordingDigest'] = fmt.digest(recordings[scenario['id']])
                result['recordingSource'] = recordings[scenario['id']]['source']
                results.append(result)
            else:
                expect = recording['expect']
                categories = {v['category'] for v in result['violations']}
                detected = (expect.get('violation') is None or expect['violation'] in categories) and \
                           ('maxScore' not in expect or (result['score'] is not None and result['score'] <= expect['maxScore']))
                checks.append({'control': recording['label'], 'scenario': scenario['id'], 'expect': expect, 'detected': detected and result['status'] == 'graded',
                               'violations': result['violations'], 'score': result['score'], 'stale': result['stale']})
    order = {s['id']: i for i, s in enumerate(scenarios)}
    results.sort(key=lambda r: order[r['scenario']])
    if unrefreshed:
        raise SystemExit('Not refreshed:\n' + '\n'.join(unrefreshed))
    return results, checks


def refresh_fingerprints(recording, run, scenario, reason, kind):
    if not reason or len(reason.strip()) < 12:
        raise SystemExit('--refresh-authored needs a specific --reason')
    if len(run['calls']) != len(recording['steps']):
        return (f'{recording.get("label", scenario["id"])}: the stack made {len(run["calls"])} model calls, the authored recording has '
                f'{len(recording["steps"])} steps; edit its decisions first (run ended: {run.get("text", "")[:300]}; '
                f'offered tools: {", ".join(run["calls"][0]["fingerprint"]["tools"]) if run["calls"] else "none"})')
    path = recording.pop('_path', None) or fmt.recording_path(scenario['id'])
    for step, call in zip(recording['steps'], run['calls']):
        step['fingerprint'] = call['fingerprint']
    if recording['source']['kind'] == 'model':
        # The decisions no longer answered exactly these requests: keep them as a reference, not as a model response.
        source = recording['source']
        recording['source'] = {'kind': 'authored', 'derivedFrom': {k: source.get(k) for k in ('provider', 'model', 'recordedAt', 'head')}}
    recording['runDigest'] = fmt.run_digest(scenario)
    recording['source'].update(recordedAt=now(), head=head(), reason=reason)
    Path(path).write_text(json.dumps(recording, indent=1, ensure_ascii=False) + '\n')
    print(f'REFRESHED {Path(path).relative_to(ROOT)} ({kind})', flush=True)
    return None


def evaluate_live(args, scenarios, pricing, choice):
    runs = run_group(choice, [(scenario, [], None) for scenario in scenarios], 'live', 1800)
    results = []
    for scenario, run in zip(scenarios, runs):
        result = grade(scenario, run, pricing)
        results.append(result)
        if args.record and result['status'] == 'graded':
            recording = {'format': fmt.RECORDING_FORMAT, 'scenario': scenario['id'], 'runDigest': fmt.run_digest(scenario),
                         'source': {'kind': 'model', 'provider': choice['provider'], 'model': choice['model'], 'recordedAt': now(), 'head': head(), 'reason': 'candidate from a real-model run'},
                         'steps': [{'fingerprint': c['fingerprint'], 'decision': c['decision'], 'usage': c['usage'], 'latencyMs': c.get('latencyMs', 0)} for c in run['calls']]}
            artifact_path(f'agent-evaluation/recordings/{scenario["id"]}.json').write_text(json.dumps(recording, indent=1, ensure_ascii=False) + '\n')
    return results


def execute(args):
    if not os.environ.get('TEST_DATABASE'):
        # Every evaluation needs its own disposable database: run inside the single suite registry.
        env = {**os.environ, 'AGENT_EVAL_ARGV': json.dumps(sys.argv[1:])}
        command = [sys.executable, str(ROOT / 'scripts/verify_integration.py'), '--container', args.container, '--only', 'agent_evaluation']
        raise SystemExit(subprocess.run(command, cwd=ROOT, env=env).returncode)
    scenarios = fmt.load_scenarios(only=args.only)
    pricing = json.loads((fmt.BASE / 'pricing.json').read_text())['tariffs']
    started = now()
    if args.model == 'replay':
        mode, choice = 'replay', None
        results, controls = evaluate_replay(args, scenarios, pricing)
    else:
        if args.refresh_authored:
            raise SystemExit('--refresh-authored replays recordings; do not combine it with a real model')
        mode, choice = 'live', choice_of(args.model)
        results, controls = evaluate_live(args, scenarios, pricing, choice), []
    summary = aggregate(results)
    report(results, summary)
    failures = []
    baseline = fmt.load_baseline() if (fmt.BASE / 'baseline.json').exists() else None
    comparison = None
    if mode == 'replay' and not args.refresh_authored:
        if baseline is None:
            failures.append('No committed baseline: run --record-baseline --reason ...')
        else:
            rows = baseline['replay']['scenarios']
            failures += gate(results, summary if not args.only else {**summary, 'score': None}, rows, baseline['replay']['aggregate'])
            for result in results:
                row = rows.get(result['scenario'])
                scenario = next(s for s in scenarios if s['id'] == result['scenario'])
                if row and (row['scenarioDigest'] != fmt.digest(scenario) or row['recordingDigest'] != result['recordingDigest']):
                    failures.append(f"{result['scenario']}: scenario or recording changed since the baseline; record a reviewed baseline")
        for control in controls:
            print(f"CONTROL {control['control']:<44} {'detected' if control['detected'] else 'MISSED'}", flush=True)
            if not control['detected']:
                failures.append(f"negative control {control['control']} was not detected: {json.dumps(control['expect'])}")
        if not args.no_controls and not args.only and len(controls) < 5:
            failures.append('Fewer than five negative controls ran')
    elif mode == 'live':
        key = f"{choice['provider']}:{choice['model']}"
        live = (baseline or {}).get('live', {}).get(key)
        failures += [f"{r['scenario']}: safety violation [{v['category']}] {v['detail']}" for r in results for v in r['violations']]
        if live:
            comparison = {'baseline': live['aggregate'], 'current': summary['score'], 'tolerance': live.get('tolerance', 5),
                          'scenarios': {r['scenario']: {'baseline': live['scenarios'].get(r['scenario']), 'current': r['score']} for r in results}}
            if summary['score'] is not None and not args.only and summary['score'] < live['aggregate'] - comparison['tolerance']:
                failures.append(f"aggregate {summary['score']} below the {key} baseline {live['aggregate']} minus tolerance {comparison['tolerance']}")
    document = {'format': RESULT_FORMAT, 'mode': mode, 'model': choice, 'startedAt': started, 'finishedAt': now(), 'head': head(), 'only': args.only,
                'scenarioDigests': {s['id']: fmt.digest(s) for s in scenarios}, 'aggregate': summary, 'results': results, 'controls': controls,
                'comparison': comparison, 'failures': failures}
    label = 'replay' if mode == 'replay' else f"live-{choice['provider']}-{choice['model'].replace(':', '-').replace('/', '-')}"
    path = write_artifact(document, label + '-' + started.replace(':', '').replace('+0000', 'Z'))
    print('ARTIFACT ' + str(path.relative_to(ROOT)), flush=True)
    for failure in failures:
        print('FAIL ' + failure, flush=True)
    if failures:
        raise SystemExit(1)
    print(f"PASS agent evaluation ({mode}): {summary['graded']} scenarios, score {summary['score']}, {summary['violations']} violations"
          + (f", {len(controls)} negative controls detected" if controls else ''), flush=True)


def require_reason(args):
    if not args.reason or len(args.reason.strip()) < 12:
        raise SystemExit('A specific --reason (at least 12 characters) is required; it is kept in the reviewed history')


def record_baseline(args):
    """Explicit, reviewed baseline update from the latest result artifact of the current files."""
    require_reason(args)
    live = args.model != 'replay'
    path = artifact_path('agent-evaluation/latest-live.json' if live else 'agent-evaluation/latest-replay.json')
    if not path.exists():
        raise SystemExit(f'No {path.relative_to(ROOT)}; run the evaluation first')
    document = json.loads(path.read_text())
    scenarios = fmt.load_scenarios()
    if document['only'] or set(document['scenarioDigests']) != {s['id'] for s in scenarios}:
        raise SystemExit('The baseline needs a complete run of every scenario')
    if document['scenarioDigests'] != {s['id']: fmt.digest(s) for s in scenarios}:
        raise SystemExit('Scenarios changed after that run; run the evaluation again')
    if any(r['violations'] or r['status'] != 'graded' for r in document['results']):
        raise SystemExit('A run with safety violations or stale recordings cannot become a baseline')
    path_baseline = fmt.BASE / 'baseline.json'
    baseline = json.loads(path_baseline.read_text()) if path_baseline.exists() else {'format': fmt.BASELINE_FORMAT, 'replay': {'aggregate': 0, 'scenarios': {}}, 'history': []}
    entry = {'at': now(), 'head': head(), 'reason': args.reason.strip(), 'aggregate': document['aggregate']['score']}
    if live:
        choice = document['model']
        if args.model != 'live' and choice_of(args.model) != choice:
            raise SystemExit('The latest real-model run used ' + json.dumps(choice))
        key = f"{choice['provider']}:{choice['model']}"
        baseline.setdefault('live', {})[key] = {'aggregate': document['aggregate']['score'], 'tolerance': baseline.get('live', {}).get(key, {}).get('tolerance', 5),
                                                'scenarios': {r['scenario']: r['score'] for r in document['results']}}
        entry.update(mode='live:' + key)
    else:
        if document['failures'] and any('negative control' in f for f in document['failures']):
            raise SystemExit('Negative controls were not detected in that run')
        for result in document['results']:
            current = json.loads(fmt.recording_path(result['scenario']).read_text())
            if fmt.digest(current) != result['recordingDigest']:
                raise SystemExit(f"{result['scenario']}: recording changed after that run; run the evaluation again")
        old = baseline['replay']['scenarios']
        rows = {r['scenario']: {'score': r['score'], 'scenarioDigest': document['scenarioDigests'][r['scenario']], 'recordingDigest': r['recordingDigest']} for r in document['results']}
        entry.update(mode='replay', changed=sorted(s for s in rows if old.get(s) != rows[s]))
        baseline['replay'] = {'aggregate': document['aggregate']['score'], 'scenarios': rows}
    baseline['history'].append(entry)
    path_baseline.write_text(json.dumps(baseline, indent=1, ensure_ascii=False) + '\n')
    print(f"RECORDED baseline {entry['mode']}: aggregate {entry['aggregate']} ({entry['reason']})")


def adopt(args):
    """Copy reviewed candidate recordings of the latest real-model run into the committed replay set."""
    require_reason(args)
    document = json.loads(artifact_path('agent-evaluation/latest-live.json').read_text())
    clean = {r['scenario'] for r in document['results'] if not r['violations'] and r['status'] == 'graded'}
    wanted = clean if args.adopt == ['all'] else set(args.adopt)
    refused = wanted - clean
    if refused:
        raise SystemExit('Not adoptable (violations, errors or not in the latest run): ' + ', '.join(sorted(refused)))
    for scenario_id in sorted(wanted):
        candidate = json.loads(artifact_path(f'agent-evaluation/recordings/{scenario_id}.json').read_text())
        candidate['source']['reason'] = args.reason.strip()
        fmt.recording_path(scenario_id).write_text(json.dumps(candidate, indent=1, ensure_ascii=False) + '\n')
        print(f'ADOPTED {scenario_id} from {candidate["source"]["provider"]}:{candidate["source"]["model"]}')
    print('Run the replay evaluation, then --record-baseline --reason ... to accept the new scores.')


def summary(mode):
    path = artifact_path(f'agent-evaluation/latest-{mode}.json')
    if not path.exists():
        raise SystemExit(f'No {path.relative_to(ROOT)}')
    document = json.loads(path.read_text())
    total = document['aggregate']
    model = 'recorded decisions' if document['mode'] == 'replay' else f"{document['model']['provider']}:{document['model']['model']}"
    lines = [f"### Agent evaluation ({model}, {document['finishedAt']})", '',
             f"Score **{total['score']}** over {total['graded']} graded scenarios, {total['violations']} safety violations, "
             f"{total['modelCalls']} model calls, {total['tokens']['input']}+{total['tokens']['output']} tokens.", '',
             '| Scenario | Role | Score | Violations | Calls | Model ms |', '|---|---|---:|---|---:|---:|']
    for r in document['results']:
        flags = ', '.join(sorted({v['category'] for v in r['violations']})) or r['status'] if r['status'] != 'graded' or r['violations'] else ''
        lines.append(f"| {r['scenario']} | {r['role']} | {r['score']} | {flags} | {r['modelCalls']} | {r['modelLatencyMs']} |")
    if document.get('controls'):
        lines += ['', f"Negative controls detected: {sum(c['detected'] for c in document['controls'])}/{len(document['controls'])}"]
    for failure in document['failures']:
        lines.append('- FAIL ' + failure)
    print('\n'.join(lines))


def main():
    argv = json.loads(os.environ.pop('AGENT_EVAL_ARGV')) if os.environ.get('AGENT_EVAL_ARGV') and os.environ.get('TEST_DATABASE') else sys.argv[1:]
    args = arguments(argv)
    if args.summary:
        summary(args.summary)
    elif args.check:
        print('PASS agent evaluation formats, recordings, controls and baseline: ' + json.dumps(fmt.static_check()))
    elif args.record_baseline:
        record_baseline(args)
    elif args.adopt:
        adopt(args)
    else:
        if args.refresh_authored:
            require_reason(args)
        if args.record and args.model == 'replay':
            raise SystemExit('--record needs a real model')
        execute(args)


if __name__ == '__main__':
    main()
