"""Agent-evaluation model gateway: replays recorded model decisions or forwards to a real provider while recording them; never a model itself."""
import hashlib
import json
import re
import threading
import time
import urllib.error
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

from testing.native_tools import response as native_response

PROMPT_ID = re.compile(r'\[prompt:([a-z-]+@\d+)\]')
WIRES = {'/api/chat': 'ollama', '/v1/responses': 'openai', '/v1/chat/completions': 'openai-chat', '/v1/messages': 'anthropic'}


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=False)


def digest(value):
    return hashlib.sha256(canonical(value).encode()).hexdigest()


def strings(value):
    if isinstance(value, str):
        yield value
    elif isinstance(value, dict):
        for item in value.values():
            yield from strings(item)
    elif isinstance(value, list):
        for item in value:
            yield from strings(item)


def tool_definitions(body):
    """(canonical name, alias, parameters) of every tool the agent stack offered in this request."""
    tools = []
    for tool in body.get('tools') or []:
        definition = tool.get('function', tool)
        description = definition.get('description', '')
        name = description.split(':', 1)[0] if ':' in description else definition.get('name', '')
        parameters = definition.get('parameters', definition.get('input_schema', {}))
        tools.append((name, definition.get('name', ''), parameters))
    return tools


def output_schema(body):
    for value in (body.get('format'), (body.get('text') or {}).get('format', {}).get('schema'),
                  (body.get('output_config') or {}).get('format', {}).get('schema'),
                  (body.get('response_format') or {}).get('json_schema', {}).get('schema')):
        if isinstance(value, dict):
            return value
    return None


def fingerprint(body, redact=()):
    """What makes a recorded decision valid for this request: prompt versions, offered tools and their schemas, and the output schema.

    Dynamic identifiers of the scenario's fresh twin (sandbox tenant IDs) are redacted
    so the same code and scenario produce the same fingerprint in every fresh database.
    """
    def clean(value):
        text = canonical(value)
        for secret in redact:
            text = text.replace(secret, '<twin>')
        return json.loads(text)
    tools = tool_definitions(body)
    schema = output_schema(body)
    return {
        'phase': 'read' if tools else 'final',
        'prompts': sorted({match for text in strings(body) for match in PROMPT_ID.findall(text)}),
        'tools': sorted(name for name, _, _ in tools),
        'toolsDigest': digest(sorted([name, clean(parameters)] for name, _, parameters in tools)),
        'schemaDigest': digest(clean(schema)) if schema is not None else None,
    }


def alias_map(body):
    return {alias: name for name, alias, _ in tool_definitions(body)}


def decision_of(wire, body, raw):
    """A provider-neutral decision from a real provider response: tool calls by canonical name, or the final text."""
    names = alias_map(body)
    calls, text = [], None
    if wire == 'ollama':
        message = raw.get('message') or {}
        for call in message.get('tool_calls') or []:
            function = call.get('function', {})
            arguments = function.get('arguments', {})
            calls.append({'name': names.get(function.get('name'), function.get('name')), 'arguments_json': arguments if isinstance(arguments, str) else canonical(arguments)})
        text = message.get('content')
    elif wire == 'openai':
        parts = []
        for item in raw.get('output') or []:
            if item.get('type') == 'function_call':
                calls.append({'name': names.get(item.get('name'), item.get('name')), 'arguments_json': item.get('arguments', '')})
            for content in item.get('content') or []:
                if content.get('type') == 'output_text':
                    parts.append(content.get('text', ''))
        text = ''.join(parts) or None
    elif wire == 'openai-chat':
        message = ((raw.get('choices') or [{}])[0]).get('message') or {}
        for call in message.get('tool_calls') or []:
            function = call.get('function', {})
            calls.append({'name': names.get(function.get('name'), function.get('name')), 'arguments_json': function.get('arguments', '')})
        text = message.get('content')
    elif wire == 'anthropic':
        parts = []
        for block in raw.get('content') or []:
            if block.get('type') == 'tool_use':
                calls.append({'name': names.get(block.get('name'), block.get('name')), 'arguments_json': canonical(block.get('input', {}))})
            elif block.get('type') == 'text':
                parts.append(block.get('text', ''))
        text = ''.join(parts) or None
    if body.get('tools'):
        return {'tool_calls': calls}
    return {'text': text or ''}


def usage_of(wire, raw):
    if wire == 'ollama':
        return {'input': int(raw.get('prompt_eval_count') or 0), 'output': int(raw.get('eval_count') or 0)}
    usage = raw.get('usage') or {}
    if wire == 'openai-chat':
        return {'input': int(usage.get('prompt_tokens') or 0), 'output': int(usage.get('completion_tokens') or 0)}
    return {'input': int(usage.get('input_tokens') or 0), 'output': int(usage.get('output_tokens') or 0)}


def render(wire, body, decision, usage):
    """The recorded decision in the wire format of the current request; unknown tool names stay invalid."""
    if body.get('tools'):
        answer = native_response(body, {'tool_calls': decision.get('tool_calls', [])}, wire)
    else:
        text = decision.get('text', '')
        if wire == 'ollama':
            answer = {'done': True, 'done_reason': 'stop', 'message': {'role': 'assistant', 'content': text}}
        elif wire == 'anthropic':
            answer = {'stop_reason': 'end_turn', 'content': [{'type': 'text', 'text': text}]}
        elif wire == 'openai-chat':
            answer = {'choices': [{'finish_reason': 'stop', 'message': {'role': 'assistant', 'content': text}}]}
        else:
            answer = {'status': 'completed', 'output': [{'type': 'message', 'content': [{'type': 'output_text', 'text': text}]}]}
    if wire == 'ollama':
        answer.update(prompt_eval_count=usage['input'], eval_count=usage['output'])
    elif wire == 'openai-chat':
        answer['usage'] = {'prompt_tokens': usage['input'], 'completion_tokens': usage['output']}
    else:
        answer['usage'] = {'input_tokens': usage['input'], 'output_tokens': usage['output']}
    return answer


class Gateway:
    """One evaluation's model endpoint. `replay` serves recorded steps; `live` forwards to `upstreams` and records each step."""

    def __init__(self, mode, upstreams=None, lenient=False):
        assert mode in ('replay', 'live')
        # Lenient replay only refreshes authored recordings' fingerprints; it is never a gate.
        self.mode, self.upstreams, self.lenient = mode, upstreams or {}, lenient
        self.lock = threading.Lock()
        self.scenario, self.steps, self.calls, self.redact = None, [], [], ()
        self.server = ThreadingHTTPServer(('127.0.0.1', 0), self.handler())
        threading.Thread(target=self.server.serve_forever, daemon=True).start()

    @property
    def url(self):
        return f'http://127.0.0.1:{self.server.server_address[1]}'

    def begin(self, scenario, steps=(), redact=()):
        with self.lock:
            self.scenario, self.steps, self.calls, self.redact = scenario, list(steps), [], tuple(redact)

    def finish(self):
        with self.lock:
            calls, self.scenario = self.calls, None
            return calls

    def handle(self, path, headers, body):
        wire = WIRES.get(path)
        if wire is None:
            return 404, {'error': 'unknown model route'}
        with self.lock:
            scenario, index = self.scenario, len(self.calls)
            print_ = fingerprint(body, self.redact)
            record = {'index': index, 'wire': wire, 'fingerprint': print_}
            self.calls.append(record)
            step = self.steps[index] if index < len(self.steps) else None
        if scenario is None:
            record['error'] = 'model call outside a scenario'
            return 503, {'error': record['error']}
        if self.mode == 'replay':
            if step is None:
                record['stale'] = 'no recorded step'
                return 503, {'error': 'recording exhausted'}
            if step['fingerprint'] != print_ and not self.lenient:
                record['stale'] = {'recorded': step['fingerprint'], 'current': print_}
                return 503, {'error': 'recording is stale for this request'}
            record.update(decision=step['decision'], usage=step['usage'], latencyMs=step.get('latencyMs', 0), replayed=True)
            return 200, render(wire, body, step['decision'], step['usage'])
        upstream = self.upstreams.get(wire)
        if upstream is None:
            record['error'] = 'no upstream for ' + wire
            return 503, {'error': record['error']}
        url, auth = upstream
        target = url.rstrip('/') + (path.removeprefix('/v1') if url.rstrip('/').endswith('/v1') else path)
        request_headers = {'Content-Type': 'application/json'}
        if auth:
            request_headers.update(auth)
        started = time.monotonic()
        try:
            request = urllib.request.Request(target, data=json.dumps(body).encode(), headers=request_headers)
            with urllib.request.urlopen(request, timeout=900) as response:
                raw = json.load(response)
        except urllib.error.HTTPError as error:
            record.update(error=f'upstream HTTP {error.code}', latencyMs=round((time.monotonic() - started) * 1000))
            return 502, {'error': record['error']}
        except (urllib.error.URLError, TimeoutError, ValueError) as error:
            record.update(error=f'upstream unavailable: {type(error).__name__}', latencyMs=round((time.monotonic() - started) * 1000))
            return 502, {'error': record['error']}
        record.update(decision=decision_of(wire, body, raw), usage=usage_of(wire, raw), latencyMs=round((time.monotonic() - started) * 1000), model=raw.get('model'))
        return 200, raw

    def handler(self):
        gateway = self

        class Handler(BaseHTTPRequestHandler):
            def log_message(self, *args):
                pass

            def do_POST(self):
                body = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
                status, answer = gateway.handle(self.path.split('?')[0], dict(self.headers), body)
                payload = json.dumps(answer).encode()
                try:
                    self.send_response(status)
                    self.send_header('Content-Type', 'application/json')
                    self.send_header('Content-Length', str(len(payload)))
                    self.end_headers()
                    self.wfile.write(payload)
                except (BrokenPipeError, ConnectionResetError):
                    pass
        return Handler

    def close(self):
        self.server.shutdown()
