#!/usr/bin/env python3
"""Typed inputs gate: component and server functions read their JSON parameters through typed views.

A function that takes `Json(x): Json<Value>` or `x: &Value` and reads `x["key"]` directly must be listed in
`scripts/testing/typed-inputs.json` under a documented category; every other reader uses a typed view
(`veyndo_core::document::view`, a lenient struct or `Raw<T>`). Listed functions that no longer read untyped
fail too, so the list only shrinks. Writes into a document (`x["key"] = ...`) are not reads."""
import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
LIST = ROOT / 'scripts/testing/typed-inputs.json'
SCOPE = ('components/', 'src/')
SKIP = ('/tests/', 'tests.rs', 'src/bin/')
FN = re.compile(r'\bfn\s+(\w+)(?:<[^>]*>)?\s*\(')


def readers(text):
    """Untyped JSON parameter reads per function name in one source text."""
    text = text.split('#[cfg(test)]')[0]
    found = {}
    for m in FN.finditer(text):
        depth, i = 1, m.end()
        while depth and i < len(text):
            depth += {'(': 1, ')': -1}.get(text[i], 0)
            i += 1
        params = text[m.end():i - 1]
        names = re.findall(r'Json\((\w+)\)\s*:\s*Json<Value>', params) + re.findall(r'\b(\w+)\s*:\s*&(?:mut\s+)?Value\b', params)
        j = text.find('{', i)
        if not names or j < 0 or ';' in text[i:j]:
            continue
        depth, k = 1, j + 1
        while depth and k < len(text):
            depth += {'{': 1, '}': -1}.get(text[k], 0)
            k += 1
        body = text[j:k]
        n = 0
        for name in names:
            if re.search(r'\blet\s+(mut\s+)?' + name + r'\b', body):
                continue
            for r in re.finditer(r'\b' + name + r'((?:\["[^"\n]+"\])+)', body):
                rest = body[r.end():].lstrip()
                if not (rest.startswith('=') and not rest.startswith('==')):
                    n += 1
        if n:
            found[m.group(1)] = found.get(m.group(1), 0) + n
    return found


def controls():
    """The detector finds untyped reads and ignores typed views, writes and rebound names."""
    cases = {
        'fn a(v: &Value) -> bool { v["x"].as_str().is_some() }': {'a': 1},
        'async fn b(Json(v): Json<Value>) { let _ = v["x"]["y"] == true; }': {'b': 1},
        'fn c(v: &mut Value) { v["x"] = json!(1); }': {},
        'fn d(v: &Value) { let input: In = view(v); input.x; }': {},
        'fn e(v: &Value) { let v = other(); v["x"].clone(); }': {},
    }
    for source, expected in cases.items():
        got = readers(source)
        if got != expected:
            raise SystemExit(f'typed inputs negative control failed: {source!r} -> {got}, expected {expected}')


def main():
    controls()
    listed = json.loads(LIST.read_text())
    categories = listed['categories']
    allowed = {}
    for entry in listed['functions']:
        if entry['category'] not in categories:
            raise SystemExit(f"unknown category {entry['category']} for {entry['file']}::{entry['function']}")
        allowed[(entry['file'], entry['function'])] = entry['category']
    files = subprocess.run(['git', 'ls-files', '*.rs'], cwd=ROOT, capture_output=True, text=True, check=True).stdout.split()
    actual = {}
    for f in files:
        if not f.startswith(SCOPE) or any(s in f for s in SKIP):
            continue
        for fn, n in readers((ROOT / f).read_text()).items():
            actual[(f, fn)] = n
    unlisted = sorted(k for k in actual if k not in allowed)
    stale = sorted(k for k in allowed if k not in actual)
    if unlisted or stale:
        for f, fn in unlisted:
            print(f'untyped JSON parameter reads in {f}::{fn} ({actual[(f, fn)]}); read through a typed view', file=sys.stderr)
        for f, fn in stale:
            print(f'{f}::{fn} no longer reads untyped; remove it from {LIST.relative_to(ROOT)}', file=sys.stderr)
        raise SystemExit(1)
    by = {}
    for key, cat in allowed.items():
        by[cat] = by.get(cat, 0) + actual[key]
    detail = ', '.join(f'{c} {n}' for c, n in sorted(by.items()))
    print(f'PASS typed inputs: component and server JSON parameters read through typed views; {len(allowed)} listed readers ({detail}); negative controls rejected')


if __name__ == '__main__':
    main()
