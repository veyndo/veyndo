#!/usr/bin/env python3
"""Guard the documented Rust domain split and public extension examples."""
import subprocess
import os
from pathlib import Path
root=Path(__file__).resolve().parents[1]
subprocess.run(['python3', 'scripts/component_sources.py'], cwd=root, check=True)
subprocess.run(['python3', 'scripts/testing/host_exchange_contract.py'], cwd=root, check=True)
subprocess.run(['python3', 'scripts/testing/component_sources_test.py'], cwd=root, check=True)
subprocess.run(['python3', 'scripts/testing/composition_release_test.py'], cwd=root, check=True)
if os.environ.get('GITHUB_ACTIONS') == 'true':
    subprocess.run(['python3', 'scripts/composition_release.py', '--shared-schema', '--fetch-pinned'],
                   cwd=root, check=True)
files=[*(root/'src').rglob('*.rs'),*(root/'crates').rglob('*.rs'),*(root/'components').rglob('*.rs'),*(root/'extensions').rglob('*.rs')]
for path in files:
    lines=path.read_text().splitlines()
    assert lines and (lines[0].startswith('//!') or lines[0].startswith('///')),str(path.relative_to(root))+' needs a responsibility comment'
assert len((root/'src/main.rs').read_text().splitlines())<=120
for path in (root/'extensions').glob('*.wat'):assert path.name in (root/'extensions/README.md').read_text(),path.name+' needs documentation'
subprocess.run(["python3", "scripts/testing/plugin_ownership.py"], cwd=root, check=True)
subprocess.run(["python3", "scripts/security_route_gate.py"], cwd=root, check=True)
# File length is not limited: a component keeps related code together. Function length is limited by clippy
# (clippy.toml), and component boundaries, table ownership and SQL placement by scripts/components.py.
subprocess.run(['python3', 'scripts/components.py'], cwd=root, check=True)
subprocess.run(['python3', 'scripts/testing/typed_inputs.py'], cwd=root, check=True)
print(f'PASS {len(files)} Rust modules: responsibility documented; main <=120; every WAT example documented')

subprocess.run(["node", "frontend/scripts/architecture.mjs"], cwd=root, check=True)
subprocess.run(["node", "frontend/scripts/localization.mjs"], cwd=root, check=True)
subprocess.run(["python3", "scripts/testing/source_inventory.py"], cwd=root, check=True)
subprocess.run(["python3", "scripts/api_catalogue.py", "--check"], cwd=root, check=True)
