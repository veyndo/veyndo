#!/usr/bin/env python3
"""Create or verify a local development shop and its owner account for signing in to Studio.

Development tool only: it talks to a server on this machine (127.0.0.1, localhost
or ::1) and refuses any other host. On the first run it registers a shop and its
owner through the ordinary `/api/auth/register` endpoint and stores the generated
credentials in `.run/dev-account.env` (ignored by git); later runs sign in with them
to prove they still work. The password is never printed.

    python3 scripts/dev_account.py            # create once, then verify
    python3 scripts/dev_account.py --shop dev-shop --email owner@dev.test
    DEV_ACCOUNT_PASSWORD=... python3 scripts/dev_account.py   # choose the password

Environment: BASE_URL (default http://127.0.0.1:8787).
"""
import argparse
import json
import os
import pathlib
import secrets
import sys
import urllib.error
import urllib.parse
import urllib.request

ROOT = pathlib.Path(__file__).resolve().parents[1]
STORE = ROOT / '.run' / 'dev-account.env'
LOCAL_HOSTS = {'127.0.0.1', 'localhost', '::1'}


def base_url():
    base = os.getenv('BASE_URL', 'http://127.0.0.1:8787').rstrip('/')
    host = urllib.parse.urlparse(base).hostname
    if host not in LOCAL_HOSTS:
        sys.exit(f'Refusing {base}: development accounts are created on this machine only')
    return base


def call(base, path, body):
    request = urllib.request.Request(
        base + path,
        data=json.dumps(body).encode(),
        headers={'Content-Type': 'application/json'},
        method='POST',
    )
    try:
        with urllib.request.urlopen(request, timeout=60) as response:
            return response.status, json.load(response)
    except urllib.error.HTTPError as error:
        try:
            return error.code, json.load(error)
        except ValueError:
            return error.code, {}
    except urllib.error.URLError as error:
        sys.exit(f'Server at {base} is not reachable ({error.reason}); start it first')


def read_store():
    if not STORE.exists():
        return {}
    pairs = (line.split('=', 1) for line in STORE.read_text().splitlines() if '=' in line)
    return {key.strip(): value.strip() for key, value in pairs}


def write_store(values):
    STORE.parent.mkdir(exist_ok=True)
    STORE.write_text(''.join(f'{key}={value}\n' for key, value in values.items()))
    STORE.chmod(0o600)


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--shop', default='dev-shop', help='shop id to create')
    parser.add_argument('--name', default='Dev Shop', help='shop display name')
    parser.add_argument('--email', default='owner@dev-shop.test', help='owner sign-in email')
    args = parser.parse_args()
    base = base_url()
    stored = read_store()
    if stored.get('DEV_EMAIL') and stored.get('DEV_PASSWORD'):
        status, session = call(base, '/api/auth/login',
                               {'email': stored['DEV_EMAIL'], 'password': stored['DEV_PASSWORD']})
        if status != 200:
            sys.exit(f'Stored development account no longer signs in (HTTP {status}); '
                     f'delete {STORE.relative_to(ROOT)} to create a new one')
        print(f"PASS development account {stored['DEV_EMAIL']} signs in to shop {session.get('workspace')}")
        return
    password = os.getenv('DEV_ACCOUNT_PASSWORD') or secrets.token_urlsafe(18)
    if len(password) < 12:
        sys.exit('DEV_ACCOUNT_PASSWORD needs at least 12 characters')
    status, session = call(base, '/api/auth/register', {
        'name': 'Dev Owner',
        'email': args.email,
        'password': password,
        'workspaceId': args.shop,
        'workspaceName': args.name,
    })
    if status != 200:
        detail = (session.get('errors') or [{}])[0].get('detail', '')
        sys.exit(f'Registration failed (HTTP {status}) {detail}'.strip())
    write_store({'DEV_SHOP': session.get('workspace', args.shop), 'DEV_EMAIL': args.email,
                 'DEV_PASSWORD': password})
    print(f"PASS created shop {session.get('workspace', args.shop)} with owner {args.email}; "
          f'credentials stored in {STORE.relative_to(ROOT)}')


if __name__ == '__main__':
    main()
