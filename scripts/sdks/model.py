"""Read the generated OpenAPI contracts into one language-neutral model the SDK generators render.

Development tooling only. The SDKs never read Rust sources or the registry directly: they are
generated from `contracts/http/*.openapi.json`, so an SDK cannot describe anything the checked
contract does not.
"""
from dataclasses import dataclass, field
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[2]
CONTRACTS = ROOT / 'contracts/http'
KEYWORDS = {
    # Python and Rust reserved words that can appear as JSON keys or path parameters.
    'and', 'as', 'assert', 'async', 'await', 'break', 'class', 'const', 'continue', 'crate', 'def', 'del',
    'dyn', 'elif', 'else', 'enum', 'except', 'extern', 'false', 'finally', 'fn', 'for', 'from', 'global',
    'if', 'impl', 'import', 'in', 'is', 'lambda', 'let', 'loop', 'match', 'mod', 'move', 'mut', 'none',
    'nonlocal', 'not', 'or', 'pass', 'pub', 'raise', 'ref', 'return', 'self', 'static', 'struct', 'super',
    'trait', 'true', 'try', 'type', 'unsafe', 'use', 'where', 'while', 'with', 'yield', 'box', 'json',
}


def pascal(text):
    return ''.join(part[:1].upper() + part[1:] for part in re.split(r'[^A-Za-z0-9]+', text) if part)


def snake(text):
    text = re.sub(r'([a-z0-9])([A-Z])', r'\1_\2', text)
    text = re.sub(r'[^A-Za-z0-9]+', '_', text).strip('_').lower()
    if not text or text[0].isdigit():
        text = 'v_' + text
    return text + '_' if text in KEYWORDS else text


def camel(text):
    value = pascal(text)
    return value[:1].lower() + value[1:]


# Type model ------------------------------------------------------------------------------
@dataclass
class Prim:
    name: str  # string | integer | number | boolean


@dataclass
class Enum:
    values: list


@dataclass
class Const:
    value: object


@dataclass
class Array:
    item: object


@dataclass
class Map:
    value: object


@dataclass
class Json:
    pass


@dataclass
class Nullable:
    inner: object


@dataclass
class Field:
    key: str
    type: object
    required: bool
    description: str = ''


@dataclass
class Struct:
    name: str
    fields: list
    open: bool  # unknown keys allowed (additionalProperties not false)


@dataclass
class Tool:
    name: str
    component: str
    description: str
    read_only: bool
    input: Struct
    output: object  # a type, or None when the registry declares no output schema
    structs: list = field(default_factory=list)


@dataclass
class Route:
    operation_id: str
    method: str
    path: str
    params: list
    body: bool
    permission: str
    source: str


@dataclass
class Component:
    name: str
    title: str
    version: str
    routes: list
    tools: list


@dataclass
class Contract:
    version: str
    components: list
    problem_kinds: list
    problem_fields: list


def to_type(schema, name, structs):
    """JSON schema -> model type; named object types are appended to `structs` once."""
    if not isinstance(schema, dict) or not schema:
        return Json()
    kind = schema.get('type')
    if isinstance(kind, list):
        rest = [k for k in kind if k != 'null']
        inner = to_type({**schema, 'type': rest[0]}, name, structs) if len(rest) == 1 else Json()
        return Nullable(inner) if 'null' in kind else inner
    if 'const' in schema:
        return Const(schema['const'])
    if 'enum' in schema and kind == 'string':
        return Enum(list(schema['enum']))
    if kind in ('string', 'integer', 'number', 'boolean'):
        return Prim(kind)
    if kind == 'array':
        return Array(to_type(schema.get('items', {}), name + 'Item', structs))
    if kind == 'object':
        properties = schema.get('properties')
        extra = schema.get('additionalProperties')
        if properties is None and isinstance(extra, dict):
            return Map(to_type(extra, name + 'Value', structs))
        if properties is None:
            return Json()
        required = set(schema.get('required', []))
        fields = [Field(key, to_type(value, name + pascal(key), structs), key in required, value.get('description', ''))
                  for key, value in properties.items()]
        struct = Struct(name, fields, extra is not False)
        structs.append(struct)
        return struct
    return Json()


def load():
    combined = json.loads((CONTRACTS / 'veyndo.openapi.json').read_text())
    problem = combined['components']['schemas']['Problem']
    components = []
    for tag in combined['tags']:
        doc = json.loads((CONTRACTS / f"{tag['name']}.openapi.json").read_text())
        routes = []
        for path, item in doc['paths'].items():
            for method, op in item.items():
                params = [p['name'] for p in op['parameters'] if p.get('in') == 'path']
                routes.append(Route(op['operationId'], method.upper(), path, params, 'requestBody' in op,
                                    op.get('x-veyndo-permission'), op['x-veyndo-source']))
        routes.sort(key=lambda r: r.operation_id)
        schemas = doc['components']['schemas']
        tools = []
        for entry in doc['x-veyndo-tools']:
            structs = []
            base = pascal(entry['name'])
            ref = entry['input']['$ref'].rsplit('/', 1)[1]
            input_type = to_type(schemas[ref], base + 'Input', structs)
            if not isinstance(input_type, Struct):
                input_type = Struct(base + 'Input', [], True)
                structs.append(input_type)
            output = None
            if 'output' in entry:
                output = to_type(schemas[entry['output']['$ref'].rsplit('/', 1)[1]], base + 'Output', structs)
            tools.append(Tool(entry['name'], tag['name'], entry['description'], entry['readOnly'], input_type, output, structs))
        components.append(Component(tag['name'], tag['description'], doc['info']['version'], routes, tools))
    return Contract(combined['info']['version'], components, problem['properties']['kind']['enum'],
                    [(k, k in problem['required'], v['type']) for k, v in problem['properties'].items()])
