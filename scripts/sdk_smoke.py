#!/usr/bin/env python3
"""Run the generated TypeScript, Python and Rust SDK smoke tests against the suite's running server.

Registered HTTP suite: `scripts/verify_integration.py --only sdk_smoke` starts the server with the
demo shops; each SDK then registers its own synthetic account. The Rust smoke binary is built by
`cargo build --workspace --bins` (crate `veyndo-sdk`), like the server itself.
"""
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
BINARY = ROOT / 'target/debug/veyndo-sdk-smoke'
assert os.environ.get('BASE_URL'), 'Run through scripts/verify_integration.py --only sdk_smoke'
# A smoke binary older than the SDK sources would test yesterday's client.
sources = [p for p in (ROOT / 'sdks/rust').rglob('*') if p.is_file()]
assert BINARY.exists() and BINARY.stat().st_mtime >= max(p.stat().st_mtime for p in sources), \
    'Build the Rust SDK smoke binary first: cargo build --workspace --bins'
for command in ([sys.executable, 'sdks/python/tests/smoke.py'],
                ['node', 'sdks/typescript/tests/smoke.ts'],
                [str(BINARY)]):
    subprocess.run(command, cwd=ROOT, check=True, env=os.environ)
print('PASS generated SDKs (TypeScript, Python, Rust) against the running server')
