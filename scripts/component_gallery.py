#!/usr/bin/env python3
"""Nightly browser check of the component gallery: axe with contrast in English and German, and visual snapshots of every example frame.

Renders `frontend/dist/gallery/gallery.js` (built by `npm run build`) from a loopback
server in Chromium, through the pinned `playwright` driver of
`scripts/testing/requirements.txt` (`BROWSER_NODE`/`BROWSER_MODULE` select another).
Baselines live in `frontend/tests/visual/gallery/<sys.platform>/`; `--update` rewrites them.
Differences and missing baselines are written to `artifacts/component-gallery/`.
The fast deterministic part (rendering, roles, axe without contrast) runs in `npm test`.
"""
import argparse
import functools
import os
import shutil
import subprocess
import sys
import tempfile
import threading
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BUNDLE = ROOT / 'frontend/dist/gallery/gallery.js'
AXE = ROOT / 'frontend/node_modules/axe-core/axe.min.js'
PAGE = ('<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" '
        'content="width=device-width,initial-scale=1"><title>Component gallery</title></head>'
        '<body style="margin:0;padding:24px"><div id="root"></div><script src="gallery.js"></script></body></html>')


def browser_driver():
    node, module = os.environ.get('BROWSER_NODE'), os.environ.get('BROWSER_MODULE')
    if node and module:
        return node, module
    try:
        import playwright
    except ImportError:
        raise SystemExit('The gallery check needs the pinned driver: python3 -m pip install -r scripts/testing/requirements.txt '
                         '&& python3 -m playwright install chromium-headless-shell')
    driver = Path(playwright.__file__).parent / 'driver'
    return str(driver / 'node'), str(driver / 'package' / 'index.mjs')


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--update', action='store_true', help='rewrite the baselines of this platform')
    args = parser.parse_args()
    for required in (BUNDLE, AXE):
        if not required.is_file():
            raise SystemExit(f'Missing {required.relative_to(ROOT)}; run npm ci && npm run build in frontend/')
    node, module = browser_driver()
    artifacts = ROOT / 'artifacts/component-gallery'
    shutil.rmtree(artifacts, ignore_errors=True)
    with tempfile.TemporaryDirectory() as site:
        shutil.copyfile(BUNDLE, Path(site) / 'gallery.js')
        (Path(site) / 'index.html').write_text(PAGE)
        handler = functools.partial(SimpleHTTPRequestHandler, directory=site)
        handler.log_message = lambda *_: None
        server = ThreadingHTTPServer(('127.0.0.1', 0), handler)
        threading.Thread(target=server.serve_forever, daemon=True).start()
        try:
            env = {**os.environ, 'BROWSER_MODULE': module, 'AXE_PATH': str(AXE),
                   'GALLERY_URL': f'http://127.0.0.1:{server.server_address[1]}/index.html',
                   'GALLERY_BASELINES': str(ROOT / 'frontend/tests/visual/gallery' / sys.platform),
                   'GALLERY_ARTIFACTS': str(artifacts), 'GALLERY_UPDATE': '1' if args.update else '0'}
            result = subprocess.run([node, str(ROOT / 'scripts/testing/component_gallery.mjs')], cwd=ROOT, env=env)
        finally:
            server.shutdown()
    if result.returncode:
        raise SystemExit(f'FAILED component gallery browser check (see {artifacts.relative_to(ROOT)})')
    print('PASS component gallery: axe with contrast and visual snapshots')


if __name__ == '__main__':
    sys.exit(main())
