"""Closed proof classification of every function in the commerce `domain/` modules.

Each function in `components/<c>/src/domain/` (and the `domain.rs` module root) of
the listed components is in exactly one of two states, recorded in
`proof/domain-inventory.json`:

- `binds-proved-rule`: its decision calls extracted, Lean-proved kernel rules; the
  recorded rules must be exactly the `verified_kernel::<rule>(` calls in its body.
  The input mapping around the call is reviewed Rust, not proved.
- `unproved`: it is listed with one of the reason codes below.

Any third state fails the check: an unlisted function, a stale entry, an unknown
status or reason, or a recorded binding that does not match the source.
`docs/formal-coverage.md` is generated from the same data and checked for drift.
"""
import json
import re

COMPONENTS = ('core', 'pricing', 'inventory', 'checkout', 'payments', 'orders')
INVENTORY = 'proof/domain-inventory.json'
PAGE = 'docs/formal-coverage.md'
STATUSES = ('binds-proved-rule', 'unproved')
REASONS = {
    'money-representation': 'Money representation and exact arithmetic outside the bool/u64 subset: '
                            'minor-unit types, decimal parsing and text, exact rationals in i128, the '
                            'exact calculator\'s rule mix, the wire conversion of JSON numbers. Rounding, '
                            'products and sums inside it are the proved rules; the rest is unit and '
                            'property tested.',
    'document-shape': 'Reads, writes or projects JSON/serde documents; data mapping, not a decision rule.',
    'text-and-locale': 'Text, labels, locale or formatting; no commerce invariant to prove.',
    'rendering': 'Renders documents (receipt text or PDF); output layout, not a decision rule.',
    'validation-of-strings': 'Validates identifiers, URLs, names or other strings and collections; '
                             'outside the bool/u64 subset, covered by unit and HTTP tests.',
    'decision-outside-subset': 'A pure decision over strings, enums or collections that the bool/u64 '
                               'extractor cannot express; reviewed and unit tested, not extracted.',
    'constructor-or-accessor': 'Builds or reads a value without a decision of its own.',
    'catalogue-data': 'Returns fixed catalogue data (states, providers, schema); reviewed data, no rule.',
    'signed-arithmetic': 'Signed (i64) quantity or amount arithmetic outside the u64 subset; unit tested.',
}

# Spelled apart so inventory tools reading the first block comment see the docstring.
BLOCK_OPEN, BLOCK_CLOSE = '/' '*', '*' '/'
FN = re.compile(r'fn\s+([A-Za-z_][A-Za-z_0-9]*)')
WORD = re.compile(r'[A-Za-z_][A-Za-z_0-9]*')
KERNEL_CALL = re.compile(r'verified_kernel::([a-z_0-9]+)\s*\(')


def files(root):
    for component in COMPONENTS:
        base = root / 'components' / component / 'src'
        for path in sorted([*(base / 'domain').rglob('*.rs'), base / 'domain.rs']):
            if path.exists() and not test_module(path):
                yield path


def test_module(path):
    """A file its parent module declares only under `#[cfg(test)]` holds tests, not domain functions."""
    declaration = re.compile(r'#\[cfg\(test\)\]\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+' + re.escape(path.stem) + r'\s*;')
    for parent in (path.parent.with_suffix('.rs'), path.parent / 'mod.rs'):
        if parent != path and parent.exists() and declaration.search(parent.read_text()):
            return True
    return False


def strip(source):
    """Blank comments, strings and chars, keeping offsets, so braces can be counted."""
    out = list(source)
    i, n = 0, len(source)

    def blank(a, b):
        for k in range(a, b):
            if out[k] != '\n':
                out[k] = ' '
    while i < n:
        if source.startswith('//', i):
            j = source.find('\n', i)
            j = n if j < 0 else j
            blank(i, j)
            i = j
        elif source.startswith(BLOCK_OPEN, i):
            j = source.find(BLOCK_CLOSE, i + 2)
            j = n if j < 0 else j + 2
            blank(i, j)
            i = j
        elif source[i] == 'r' and re.match(r'r#*"', source[i:i + 10]) and (i == 0 or not (source[i - 1].isalnum() or source[i - 1] == '_')):
            hashes = re.match(r'r(#*)"', source[i:]).group(1)
            end = source.find('"' + hashes, i + 2 + len(hashes))
            j = n if end < 0 else end + 1 + len(hashes)
            blank(i, j)
            i = j
        elif source[i] == '"':
            j = i + 1
            while j < n and source[j] != '"':
                j += 2 if source[j] == '\\' else 1
            blank(i, j + 1)
            i = j + 1
        elif source[i] == "'" and re.match(r"'(\\.|[^\\'])'", source[i:i + 4]) or source.startswith("'\\u{", i):
            j = source.find("'", i + 1 if source[i + 1] != '\\' else i + 2)
            blank(i, j + 1)
            i = j + 1
        else:
            i += 1
    return ''.join(out)


def functions(source):
    """Top-level functions and methods of top-level impl/trait blocks; test modules are skipped.
    Returns {qualified name: body text}."""
    text = strip(source)
    result = {}
    depth = 0
    i = 0
    blocks = []  # (depth at open, owner name or None, skip)
    pending = None
    while i < len(text):
        c = text[i]
        if c == '{':
            blocks.append((depth, pending))
            depth += 1
            pending = None
        elif c == '}':
            depth -= 1
            blocks.pop()
        elif c.isalpha() or c == '_':
            m = WORD.match(text, i)
            word = m.group(0)
            skipping = any(owner == ('skip',) for _, owner in blocks)
            owner = blocks[-1][1] if blocks else None
            if not skipping and word == 'mod' and depth == 0:
                head = text[max(0, i - 40):i]
                pending = ('skip',) if '#[cfg(test)]' in source[max(0, i - 40):i] or 'cfg(test)' in head else ('module',)
            elif not skipping and word in ('impl', 'trait') and depth == 0:
                header = text[i:text.find('{', i)]
                if word == 'impl':
                    header = re.sub(r'^impl\s*(<[^{]*?>)?\s*', '', header)
                    header = header.split(' for ')[-1]
                else:
                    header = header[len('trait'):]
                name = re.match(r'\s*([A-Za-z_][A-Za-z_0-9:]*)', header)
                pending = ('owner', name.group(1).split('::')[-1] if name else '?')
            elif not skipping and word == 'fn':
                fm = FN.match(text, i)
                inside = depth == 0 or (depth == 1 and owner and owner[0] == 'owner')
                attributes = source[source.rfind('\n', 0, max(0, source.rfind('\n', 0, i))) + 1:i]
                if fm and inside and 'cfg(test)' in attributes:
                    fm = None
                if fm and inside:
                    name = fm.group(1) if depth == 0 else owner[1] + '::' + fm.group(1)
                    open_at, nesting, j = -1, 0, fm.end()
                    while j < len(text):
                        if text[j] in '([':
                            nesting += 1
                        elif text[j] in ')]':
                            nesting -= 1
                        elif nesting == 0 and text[j] in '{;':
                            open_at = j if text[j] == '{' else -1
                            break
                        j += 1
                    if open_at >= 0:
                        level, j = 0, open_at
                        while j < len(text):
                            if text[j] == '{':
                                level += 1
                            elif text[j] == '}':
                                level -= 1
                                if level == 0:
                                    break
                            j += 1
                        body = source[open_at:j + 1]
                    else:
                        # A trait method declaration without a body is not a function of its own.
                        i = m.end()
                        continue
                    key, k = name, 2
                    while key in result:
                        key, k = f'{name}#{k}', k + 1
                    result[key] = body
            i = m.end()
            continue
        i += 1
    return result


def scan(root):
    found = {}
    for path in files(root):
        rel = str(path.relative_to(root))
        for name, body in functions(path.read_text()).items():
            found[rel + '::' + name] = sorted(set(KERNEL_CALL.findall(body)))
    return found


def check(root, policies, recorded=None):
    """Fail on any third state; return the counts for the report."""
    recorded = recorded or json.loads((root / INVENTORY).read_text())
    entries = recorded['functions']
    found = scan(root)
    problems = []
    for key in sorted(set(found) - set(entries)):
        problems.append('unlisted: ' + key)
    for key in sorted(set(entries) - set(found)):
        problems.append('stale: ' + key)
    for key in sorted(set(found) & set(entries)):
        row, calls = entries[key], found[key]
        status = row.get('status')
        if status not in STATUSES:
            problems.append('unknown status: ' + key)
        elif status == 'binds-proved-rule':
            rules = row.get('rules', [])
            if not rules or sorted(rules) != calls or any(r not in policies for r in rules):
                problems.append(f'binding differs from source {calls}: {key}')
        elif calls:
            problems.append(f'calls proved rules {calls} but listed unproved: {key}')
        elif row.get('reason') not in REASONS:
            problems.append('unproved without a known reason: ' + key)
    if problems:
        raise ValueError('Domain proof inventory has a third state:\n  ' + '\n  '.join(problems[:40]))
    page = render(json.loads((root / INVENTORY).read_text()))
    if not (root / PAGE).exists() or (root / PAGE).read_text() != page:
        raise ValueError('Generated page out of date: ' + PAGE + ' (run scripts/formal/domain_inventory.py --write)')
    bound = sum(row['status'] == 'binds-proved-rule' for row in entries.values())
    return {'domainFunctions': len(entries), 'bindsProvedRule': bound, 'unprovedWithReason': len(entries) - bound,
            'components': list(COMPONENTS), 'thirdState': 0}


def render(recorded):
    entries = recorded['functions']
    lines = ['# Proof coverage of the commerce domain functions', '',
             'Generated from `proof/domain-inventory.json` by `scripts/formal/domain_inventory.py --write`;',
             '`scripts/formal.py --check` fails when a function in `domain/` of '
             + ', '.join(f'`{c}`' for c in COMPONENTS) + ' is in neither state below.', '',
             '**binds-proved-rule** means the function decides through extracted, Lean-proved kernel rules; '
             'the input mapping around the call is reviewed Rust, not proved. **unproved** functions carry '
             'one of the reasons listed at the end. Nothing here proves SQL, HTTP or providers.', '']
    for note in recorded.get('notes', []):
        lines += [note, '']
    lines += ['| Component | Functions | Binds a proved rule | Unproved |', '| --- | ---: | ---: | ---: |']
    for component in COMPONENTS:
        rows = [r for k, r in entries.items() if k.startswith(f'components/{component}/')]
        bound = sum(r['status'] == 'binds-proved-rule' for r in rows)
        lines.append(f'| {component} | {len(rows)} | {bound} | {len(rows) - bound} |')
    bound = sum(r['status'] == 'binds-proved-rule' for r in entries.values())
    lines += [f'| **all** | **{len(entries)}** | **{bound}** | **{len(entries) - bound}** |', '',
              '## Functions that bind proved rules', '', '| Function | Rules |', '| --- | --- |']
    for key, row in sorted(entries.items()):
        if row['status'] == 'binds-proved-rule':
            lines.append(f'| `{key}` | ' + ', '.join(f'`{r}`' for r in row['rules']) + ' |')
    lines += ['', '## Unproved functions by reason', '']
    for code, text in REASONS.items():
        keys = sorted(k for k, r in entries.items() if r['status'] == 'unproved' and r['reason'] == code)
        if not keys:
            continue
        lines += [f'### {code} ({len(keys)})', '', text, '']
        by_file = {}
        for key in keys:
            path, name = key.split('::', 1)
            by_file.setdefault(path, []).append(name)
        for path, names in by_file.items():
            lines.append(f'- `{path}`: ' + ', '.join(f'`{n}`' for n in names))
        lines.append('')
    notes = sorted((k, r['note']) for k, r in entries.items() if r.get('note'))
    if notes:
        lines += ['## Notes on single functions', '']
        lines += [f'- `{key}`: {note}' for key, note in notes]
    return '\n'.join(lines).rstrip('\n') + '\n'


def controls(root, policies):
    """The check must reject each third state; returns how many were rejected."""
    import copy
    recorded = json.loads((root / INVENTORY).read_text())
    rows = recorded['functions']
    bound = next(k for k, r in rows.items() if r['status'] == 'binds-proved-rule')
    unproved = next(k for k, r in rows.items() if r['status'] == 'unproved')
    broken = []
    for change in (lambda f: f.pop(unproved),
                   lambda f: f.__setitem__('components/orders/src/domain/gone.rs::stale', {'status': 'unproved', 'reason': 'rendering'}),
                   lambda f: f[unproved].__setitem__('status', 'reviewed'),
                   lambda f: f[unproved].pop('reason'),
                   lambda f: f[unproved].__setitem__('reason', 'too hard'),
                   lambda f: f.__setitem__(bound, {'status': 'unproved', 'reason': 'rendering'}),
                   lambda f: f[bound].__setitem__('rules', ['stock_admissible'])):
        model = copy.deepcopy(recorded)
        change(model['functions'])
        broken.append(model)
    rejected = 0
    for model in broken:
        try:
            check(root, policies, model)
        except ValueError as error:
            if 'third state' not in str(error):
                raise
            rejected += 1
            continue
        raise ValueError('Domain inventory accepted a third state')
    return rejected


def main():
    import argparse
    import pathlib
    import sys
    root = pathlib.Path(__file__).resolve().parents[2]
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--write', action='store_true', help='Regenerate the coverage page from the inventory')
    args = parser.parse_args()
    recorded = json.loads((root / INVENTORY).read_text())
    if args.write:
        (root / PAGE).write_text(render(recorded))
    policies = set(json.loads((root / 'proof/manifest.json').read_text())['policies'])
    try:
        print(json.dumps(check(root, policies), indent=2))
    except ValueError as error:
        sys.exit(str(error))


if __name__ == '__main__':
    main()
