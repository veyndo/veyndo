#!/usr/bin/env python3
"""Check the component map: every Rust source has one owner, every table has one owner, and no component gains an undeclared dependency on another component's code or tables."""
import argparse
from collections import Counter, defaultdict
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
MAP = ROOT / 'components.json'
REPORT = ROOT / 'docs/component-dependencies.md'
FOLDERS = ('src', 'crates', 'components')
DEFINITION = re.compile(
    r'(?m)^pub(?:\(crate\))? (?:async |const |unsafe )*(fn|struct|enum|trait|type|const|static) (\w+)')
LOCAL = re.compile(r'(?m)^\s*(?:pub(?:\([a-z]+\))? )?(?:async |const |unsafe )*(?:fn|struct|enum|trait|type|const|static|mod) (\w+)')
TABLE = re.compile(r'(?i)CREATE TABLE (?:IF NOT EXISTS )?(?:public\.)?"?([a-z_][a-z0-9_]*)')
WRITE = r'(?i)\b(?:INSERT INTO|UPDATE|DELETE FROM|MERGE INTO)\s+(?:ONLY\s+)?(?:public\.)?'
# A glob import at file level hides which items a file takes from elsewhere; test modules are indented and not counted.
GLOB = re.compile(r'(?m)^(?:pub(?:\(crate\))? )?use (?:super|crate)::\*;')
# `value["key"]` reads a field without a type; a struct at the component interface replaces it.
UNTYPED = re.compile(r'\b\w+(?:\["[^"\n]+"\])+')
SQL_CALL = re.compile(r'\bsqlx::(?:query|query_as|query_scalar|QueryBuilder)\b')
READ = r'(?i)\b(?:FROM|JOIN)\s+(?:ONLY\s+)?(?:public\.)?'


def split_source(text):
    """Separate Rust code from its string literals and drop comments: returns (code, strings)."""
    code, strings, i, n = [], [], 0, len(text)
    while i < n:
        c = text[i]
        if text.startswith('//', i):
            i = text.find('\n', i)
            i = n if i < 0 else i
        elif text.startswith('/*', i):
            end = text.find('*/', i)
            i = n if end < 0 else end + 2
        elif c == 'r' and re.compile(r'r#*"').match(text, i) and (i == 0 or not (text[i - 1].isalnum() or text[i - 1] == '_')):
            opening = re.compile(r'r(#*)"').match(text, i)
            end = text.find('"' + opening.group(1), opening.end())
            end = n if end < 0 else end
            strings.append(text[opening.end():end])
            i = end + 1 + len(opening.group(1))
            code.append('""')
        elif c == '"':
            j = i + 1
            while j < n and text[j] != '"':
                j += 2 if text[j] == '\\' else 1
            strings.append(text[i + 1:j])
            i = j + 1
            code.append('""')
        elif c == "'":
            # A character literal closes within four characters; a lifetime does not close at all.
            char = re.compile(r"'(?:\\.[^']{0,8}|[^'\\])'").match(text, i)
            i = char.end() if char else i + 1
            code.append("' '" if char else "'")
        else:
            code.append(c)
            i += 1
    return ''.join(code), strings


class Tree:
    def __init__(self, root=ROOT, mapping=None):
        self.root = root
        self.map = mapping or json.loads((root / 'components.json').read_text())
        self.prefixes = sorted(((prefix, name) for name, row in self.map['components'].items() for prefix in row['sources']),
                               key=lambda pair: -len(pair[0]))
        self.files = {}
        for folder in FOLDERS:
            for path in sorted((root / folder).rglob('*')):
                if path.suffix in ('.rs', '.sql') and 'target' not in path.parts:
                    self.files[str(path.relative_to(root))] = path
        self.problems = []
        duplicates = [prefix for prefix, count in Counter(prefix for prefix, _ in self.prefixes).items() if count > 1]
        if duplicates:
            self.problems.append('A source prefix has two owners: ' + ', '.join(duplicates))
        for prefix, name in self.prefixes:
            if not any(file == prefix or file.startswith(prefix.rstrip('/') + '/') for file in self.files):
                self.problems.append(f'{name}: source {prefix} matches no file')
        self.owner = {file: self.owner_of(file) for file in self.files}
        for file, owner in self.owner.items():
            if owner is None:
                self.problems.append(file + ' belongs to no component')
        self.parsed = {file: split_source(path.read_text()) if path.suffix == '.rs' else ('', [path.read_text()])
                       for file, path in self.files.items()}

    def owner_of(self, file):
        for prefix, name in self.prefixes:
            if file == prefix or file.startswith(prefix.rstrip('/') + '/'):
                return name
        return None

    def code_edges(self, include_server=True):
        """References from one component to items another component defines: {(from, to): Counter(symbol)}."""
        defined, local = defaultdict(set), defaultdict(set)
        for file, (code, _) in self.parsed.items():
            owner = self.owner[file]
            for _, name in DEFINITION.findall(code):
                defined[name].add(owner)
            local[owner].update(LOCAL.findall(code))
        unique = {name: next(iter(owners)) for name, owners in defined.items() if len(owners) == 1 and len(name) >= 4}
        # A top-level module may hold files of several components; then the item after `module::` decides.
        modules, members = defaultdict(set), defaultdict(dict)
        for file, (code, _) in self.parsed.items():
            parts = file.split('/')
            if parts[0] != 'src' or not file.endswith('.rs'):
                continue
            module = parts[1].removesuffix('.rs')
            modules[module].add(self.owner[file])
            if len(parts) > 2:
                members[module].setdefault(parts[2].removesuffix('.rs'), self.owner[file])
            for name in LOCAL.findall(code):
                members[module].setdefault(name, self.owner[file])
        # A file in a crate of its own reaches another component only through that crate's name.
        packages = {}
        for manifest in self.root.glob('*/*/Cargo.toml'):
            name = re.search(r'(?m)^name\s*=\s*"([^"]+)"', manifest.read_text())
            owner = self.owner_of(str(manifest.parent.relative_to(self.root)) + '/src/lib.rs')
            if name and owner:
                packages[name.group(1).replace('-', '_')] = owner
        edges = defaultdict(Counter)
        for file, (code, _) in self.parsed.items():
            here = self.owner[file]
            if here is None or (not include_server and file.startswith('src/')):
                continue
            if not file.startswith('src/'):
                for package in re.findall(r'(?<![\w:])(veyndo_\w+)::', code):
                    target = packages.get(package)
                    if target and target != here:
                        edges[here, target][package + '::'] += 1
                continue
            for module, item in re.findall(r'(?<![\w:])(?:crate::)?([a-z_][a-z0-9_]*)::(\w+)', code):
                owners = modules.get(module)
                if not owners or module in local[here]:
                    continue
                target = next(iter(owners)) if len(owners) == 1 else members[module].get(item)
                if target and target != here:
                    edges[here, target][module + '::'] += 1
            for match in re.finditer(r'(?<![\w.:])([A-Za-z_]\w{3,})\b(\s*\()?', code):
                name = match.group(1)
                target = unique.get(name)
                if not target or target == here or name in local[here]:
                    continue
                # A lower-case name counts only when it is called; a local variable may share the name.
                if name[0].isupper() or match.group(2):
                    edges[here, target][name] += 1
        return edges

    def glob_imports(self):
        """Files per component that import everything from their parent or the crate root."""
        counts = Counter()
        for file, path in self.files.items():
            if path.suffix == '.rs' and GLOB.search(path.read_text()):
                counts[self.owner[file]] += 1
                if file.startswith('components/'):
                    self.problems.append(file + ' is in a component crate and must name its imports')
        return counts

    def budgets(self):
        """Per component: untyped field accesses, and files with SQL outside a `store` folder. Tests are not counted."""
        untyped, scattered = Counter(), Counter()
        for file, (code, _) in self.parsed.items():
            if not file.endswith('.rs') or 'tests' in file.split('/') or file.endswith('_tests.rs'):
                continue
            # Strings were replaced by "" when the source was split; count the accesses in the original text.
            text = re.sub(r'(?m)^\s*//.*$', '', self.files[file].read_text()).split('#[cfg(test)]')[0]
            untyped[self.owner[file]] += len(UNTYPED.findall(text))
            if SQL_CALL.search(code) and 'store' not in file.split('/') and not file.endswith('/store.rs'):
                scattered[self.owner[file]] += 1
        return untyped, scattered

    def tables(self):
        names = set()
        for path in sorted((self.root / 'migrations').glob('*.sql')):
            names.update(name.lower() for name in TABLE.findall(path.read_text()))
        # `public.%I` in a migration is a format placeholder for per-app tables, not a table called "public".
        return names - {'public'}

    def table_use(self, tables):
        """{(component, table): 'write' | 'read'} from the SQL text in each component's sources."""
        use = {}
        pattern = '(' + '|'.join(sorted(map(re.escape, tables), key=len, reverse=True)) + r')\b'
        for file, (_, strings) in self.parsed.items():
            here = self.owner[file]
            text = '\n'.join(strings)
            for table in re.findall(WRITE + pattern, text):
                use[here, table.lower()] = 'write'
            for table in re.findall(READ + pattern, text):
                use.setdefault((here, table.lower()), 'read')
        return use


def crate_cycles(tree):
    """Pairs of component crates that reference each other, counting only sources outside the server crate."""
    edges = tree.code_edges(include_server=False)
    return {tuple(sorted(pair)) for pair in edges if (pair[1], pair[0]) in edges}


def findings(tree):
    """Everything that differs from the declared map, plus the measured edges for the report."""
    problems = list(tree.problems)
    components = tree.map['components']
    edges = tree.code_edges()
    tables = tree.tables()
    owned = Counter(table for row in components.values() for table in row.get('tables', []))
    for table in sorted(tables - set(owned)):
        problems.append('table ' + table + ' has no owning component')
    for table, count in owned.items():
        if count > 1:
            problems.append('table ' + table + ' has ' + str(count) + ' owners')
        if table not in tables:
            problems.append('table ' + table + ' is declared but no migration creates it')
    table_owner = {table: name for name, row in components.items() for table in row.get('tables', [])}
    use = tree.table_use(tables)
    globs = tree.glob_imports()
    problems += [line for line in tree.problems if line not in problems]
    for name, row in components.items():
        declared_globs = row.get('glob_imports', 0)
        if globs[name] > declared_globs:
            problems.append(f'{name} has {globs[name]} files with a glob import, {declared_globs} are recorded; name the imports instead')
        if globs[name] < declared_globs:
            problems.append(f'{name} has only {globs[name]} files with a glob import left; lower "glob_imports" from {declared_globs}')
    untyped, scattered = tree.budgets()
    for name, row in components.items():
        for label, key, actual_count in (('untyped field accesses', 'untyped_access', untyped[name]),
                                         ('files with SQL outside store/', 'sql_outside_store', scattered[name])):
            recorded = row.get(key, 0)
            if actual_count > recorded:
                problems.append(f'{name} has {actual_count} {label}, {recorded} are recorded; the number only falls')
            if actual_count < recorded:
                problems.append(f'{name} has only {actual_count} {label} left; lower "{key}" from {recorded}')
    for name, row in components.items():
        actual = sorted({target for (source, target) in edges if source == name})
        declared = sorted(row.get('uses', []))
        for target in sorted(set(actual) - set(declared)):
            sample = ', '.join(symbol for symbol, _ in edges[name, target].most_common(3))
            problems.append(f'{name} now depends on {target} ({sample}); use its public interface or declare the dependency')
        for target in sorted(set(declared) - set(actual)):
            problems.append(f'{name} no longer depends on {target}; remove it from "uses"')
        foreign = sorted({table for (source, table) in use if source == name and table_owner.get(table) not in (None, name)})
        shared = sorted(row.get('foreign_tables', []))
        for table in sorted(set(foreign) - set(shared)):
            problems.append(f'{name} now accesses table {table} of {table_owner[table]}; go through its owner or declare it')
        for table in sorted(set(shared) - set(foreign)):
            problems.append(f'{name} no longer accesses table {table}; remove it from "foreign_tables"')
    for a, b in sorted(crate_cycles(tree)):
        problems.append(f'component crates {a} and {b} depend on each other; reduce the pair to one direction')
    # Server files reach components only from the host; a pair through them is refused like a crate cycle.
    for a, b in sorted({tuple(sorted(pair)) for pair in edges if (pair[1], pair[0]) in edges}):
        problems.append(f'components {a} and {b} depend on each other through server files; reduce the pair to one direction')
    return problems, edges, use, table_owner


def bootstrap(tree):
    """Fill uses, tables and foreign_tables from the sources; table owners default to the component that writes most."""
    components = tree.map['components']
    edges = tree.code_edges()
    tables = tree.tables()
    declared = {table: name for name, row in components.items() for table in row.get('tables', [])}
    writes, reads = defaultdict(Counter), defaultdict(Counter)
    pattern = '(' + '|'.join(sorted(map(re.escape, tables), key=len, reverse=True)) + r')\b'
    for file, (_, strings) in tree.parsed.items():
        text = '\n'.join(strings)
        for table in re.findall(WRITE + pattern, text):
            writes[table.lower()][tree.owner[file]] += 1
        for table in re.findall(READ + pattern, text):
            reads[table.lower()][tree.owner[file]] += 1
    for table in sorted(tables - set(declared)):
        ranking = writes[table] or reads[table]
        declared[table] = ranking.most_common(1)[0][0] if ranking else 'core'
    use = tree.table_use(tables)
    globs = tree.glob_imports()
    untyped, scattered = tree.budgets()
    for name, row in components.items():
        row['glob_imports'] = globs[name]
        row['untyped_access'] = untyped[name]
        row['sql_outside_store'] = scattered[name]
        row['tables'] = sorted(table for table, owner in declared.items() if owner == name and table in tables)
        row['uses'] = sorted({target for (source, target) in edges if source == name})
        row['foreign_tables'] = sorted({table for (source, table) in use if source == name and declared[table] != name})
    MAP.write_text(json.dumps(tree.map, indent=2) + '\n')


def report(tree, edges, use, table_owner):
    components = tree.map['components']
    sizes = Counter()
    for file, path in tree.files.items():
        if path.suffix == '.rs':
            sizes[tree.owner[file]] += len(path.read_text().splitlines())
    lines = ['# Component dependencies', '',
             '<!-- Generated by scripts/components.py --write from components.json and the Rust sources. Do not edit. -->', '',
             'Each row is one component of the target architecture. "Uses" counts references to items that',
             'another component defines; "Foreign tables" lists tables a component reads or writes directly',
             'although another component owns them. Both lists are the remaining hidden dependencies: the',
             'required structure check refuses a new one, and a removed one has to be deleted from',
             '`components.json`. The counts come from a text scan and are an estimate, not a compiler result.',
             '"Untyped accesses" counts `value["key"]` reads outside tests; "SQL outside store" counts files that',
             'run SQL without being in a `store` folder. Both numbers only fall.',
             '"Glob imports" counts files that start with `use super::*` or `use crate::*`; the number only falls.', '',
             '| Component | Lines of Rust | Glob imports | Untyped accesses | SQL outside store | Tables | Uses | Foreign tables |',
             '| --- | ---: | ---: | ---: | ---: | ---: | --- | ---: |']
    for name, row in components.items():
        uses = ', '.join(f'{target} ({sum(edges[name, target].values())})' for target in row.get('uses', [])) or 'none'
        lines.append(f'| {name} | {sizes[name]} | {row.get("glob_imports", 0)} | {row.get("untyped_access", 0)} | {row.get("sql_outside_store", 0)} | {len(row.get("tables", []))} | {uses} | {len(row.get("foreign_tables", []))} |')
    pairs = sorted({tuple(sorted(pair)) for pair in edges if (pair[1], pair[0]) in edges})
    crate_pairs = sorted(crate_cycles(tree))
    lines += ['', '## Cycles', '', f'{len(crate_pairs)} pairs of component crates depend on each other (the required check keeps this at zero).',
              f'{len(pairs)} pairs of components depend on each other once the server files are counted for the component that will own them;',
              'those pairs close when the server files that still hold their logic move into the components.', '']
    lines += [f'- {a} ↔ {b}: {sum(edges[a, b].values())} and {sum(edges[b, a].values())} references' for a, b in pairs]
    lines += ['', '## Most used items across components', '', '| From | To | Item | References |', '| --- | --- | --- | ---: |']
    top = sorted(((count, source, target, symbol) for (source, target), counter in edges.items()
                  for symbol, count in counter.items()), reverse=True)[:40]
    lines += [f'| {source} | {target} | `{symbol}` | {count} |' for count, source, target, symbol in top]
    lines += ['', '## Foreign table access', '', '| Component | Table | Owner | Access |', '| --- | --- | --- | --- |']
    for (source, table), kind in sorted(use.items()):
        if table_owner.get(table) not in (None, source):
            lines.append(f'| {source} | `{table}` | {table_owner[table]} | {kind} |')
    return '\n'.join(lines) + '\n'


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--bootstrap', action='store_true', help='rewrite uses, tables and foreign_tables from the sources')
    parser.add_argument('--write', action='store_true', help='write docs/component-dependencies.md')
    args = parser.parse_args(argv)
    tree = Tree()
    if args.bootstrap:
        assert not tree.problems, '\n'.join(tree.problems)
        bootstrap(tree)
        tree = Tree()
    problems, edges, use, table_owner = findings(tree)
    if args.write:
        REPORT.write_text(report(tree, edges, use, table_owner))
    if problems:
        raise SystemExit('\n'.join(problems))
    # Negative controls: an unowned file, an undeclared dependency and an unowned table must each be refused.
    broken = json.loads(json.dumps(tree.map))
    first = next(name for name, row in broken['components'].items() if row.get('uses'))
    broken['components'][first]['uses'] = broken['components'][first]['uses'][1:]
    assert any('now depends on' in line for line in findings(Tree(mapping=broken))[0]), 'an undeclared dependency must fail'
    broken = json.loads(json.dumps(tree.map))
    owner = next(name for name, row in broken['components'].items() if row.get('tables'))
    broken['components'][owner]['tables'] = broken['components'][owner]['tables'][1:]
    assert any('has no owning component' in line for line in findings(Tree(mapping=broken))[0]), 'an unowned table must fail'
    broken = json.loads(json.dumps(tree.map))
    del broken['components'][first]
    assert any('belongs to no component' in line for line in Tree(mapping=broken).problems), 'an unowned source must fail'
    broken = json.loads(json.dumps(tree.map))
    busiest = max(broken['components'], key=lambda name: broken['components'][name].get('glob_imports', 0))
    broken['components'][busiest]['glob_imports'] -= 1
    assert any('files with a glob import' in line for line in findings(Tree(mapping=broken))[0]), 'a new glob import must fail'
    broken = json.loads(json.dumps(tree.map))
    broken['components'][busiest]['untyped_access'] = max(0, broken['components'][busiest].get('untyped_access', 0) - 1)
    assert any('untyped field accesses' in line for line in findings(Tree(mapping=broken))[0]), 'a new untyped access must fail'
    broken = json.loads(json.dumps(tree.map))
    # The inference crate used to be counted for intelligence, which made identity and intelligence a crate cycle.
    broken['components']['inference']['sources'].remove('crates/cognition')
    broken['components']['intelligence']['sources'].append('crates/cognition')
    assert any('depend on each other' in line for line in findings(Tree(mapping=broken))[0]), 'a crate cycle must fail'
    broken = json.loads(json.dumps(tree.map))
    # Counting a host file for a domain component again would reintroduce a pair through server files.
    broken['components']['host']['sources'].remove('src/apps')
    broken['components']['apps']['sources'].append('src/apps')
    assert any('through server files' in line for line in findings(Tree(mapping=broken))[0]), 'a pair through server files must fail'
    cycles = len({tuple(sorted(pair)) for pair in edges if (pair[1], pair[0]) in edges})
    foreign = sum(len(row.get('foreign_tables', [])) for row in tree.map['components'].values())
    print(f'PASS {len(tree.map["components"])} components own {sum(p.suffix == ".rs" for p in tree.files.values())} Rust sources and '
          f'{len(table_owner)} tables; {len(edges)} declared dependencies, {len(crate_cycles(tree))} mutual crate pairs, {cycles} mutual pairs through server files and {foreign} foreign table '
          f'accesses and {sum(row.get("glob_imports", 0) for row in tree.map["components"].values())} files with glob imports remain; '
          'negative controls rejected')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
