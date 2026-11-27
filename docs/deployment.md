# Vercel frontend + self-hosted Rust / PostgreSQL

**Deployment choices:** this page describes the optional Vercel frontend and
self-hosted backend package. The active hosted system uses Northflank, documented
in [managed hosting](managed-hosting.md). Its 15 October 2026 restricted-runtime
transition includes live sign-in, shop creation and simulated checkout checks;
it does not verify this alternative Vercel topology or real payment settlement.
GitHub Pages hosts documentation, not the Rust commerce service.

## Prepare the private host configuration

Use a Linux host with Docker Compose, a public DNS name pointing to it and
reachable ports 80/443. PostgreSQL/Qdrant stay on the private Compose
network. The repository does not purchase a server or reuse unrelated services.

From the repository root:

```sh
python3 scripts/prepare_host.py --domain commerce-api.YOUR-DOMAIN --admin-email YOUR-OPERATOR-EMAIL
```

This creates ignored `deploy/.env` with mode 0600, independent generated database,
instance and operator credentials. It refuses to overwrite an existing file and
never prints secret values. Configure the private inference endpoint and optional
provider/app endpoints there. Transfer this private file securely to the selected
host; do not commit it or paste resolved Compose configuration into logs.

Public Compose enforces `SEED_DEMO=false`, `ALLOW_PUBLIC_SIGNUP=false` and
`ALLOW_BOOTSTRAP_AUTH=false`. It creates no known demo customer accounts.

## Start the backend and your operator account

```sh
docker compose --env-file deploy/.env -f deploy/compose.yaml up -d --build postgres qdrant commerce gateway
docker compose --env-file deploy/.env -f deploy/compose.yaml --profile operator-setup run --rm operator
```

The image builds the real Rust binary and frontend, including the shared analytics
SDK. Rust runs as UID 10001. The one-shot operator service receives the initial
operator password; the long-running commerce service does not. After successful
setup, remove `PLATFORM_ADMIN_PASSWORD` from the private file (retain your password
in your own password manager). Sign in at `https://YOUR-BACKEND-DOMAIN/#platform`.
No instance token should be entered in the browser. Use the console to create
empty or example-catalogue shops, then invite individual merchant team members.

Caddy terminates HTTPS and proxies to the internal Rust service.
[Caddy automatic HTTPS](https://caddyserver.com/docs/automatic-https) requires a
reachable configured domain. Persistent volumes retain PostgreSQL, uploaded
binary product assets (stored in PostgreSQL), connector state and certificates.
Automatic backup/restore/failover is not implemented.

Setup applies the checksummed component baselines and forward migrations
([schema baselines](rust-services.md#schema-baselines)). `BOOTSTRAP_MODE=migrate` performs
setup or the upgrade without workers; `serve` requires every migration it ships to be
recorded. Run the migration-only job, then `veyndo --provision`, then start
`serve` replicas; forward migrations are expand/contract, so the previous release keeps
serving meanwhile ([forward migrations](rust-services.md#forward-migrations)). Setup
refuses a database of the pre-baseline incremental release
([release notes](current-release.md#what-operators-must-know)).

## Provision roles and database authority

A fresh installation and every later release need no hand-written SQL. The database
manager (your provider console, or `deploy/sql/provision-runtime.sh` in the bundled
Compose stack) creates the logins and their passwords; the product never creates a
login or sees a password. Then run, on the migration-owner connection:

```sh
BOOTSTRAP_MODE=migrate DATABASE_URL=<migration owner> \
DB_RUNTIME_ROLE=veyndo_runtime DB_CONNECTOR_LOGIN=<connector login> \
DB_SYSTEM_LOGIN=<runtime/system login> DB_TENANT_LOGIN=<tenant login> \
  veyndo --provision
```

| Variable | Default | Meaning |
| --- | --- | --- |
| `DB_RUNTIME_ROLE` | `veyndo_runtime` | Group granted `SELECT,INSERT,UPDATE,DELETE,REFERENCES` on every non-`connector_` table and `USAGE,SELECT` on every sequence; connector tables are revoked from it. Must exist. |
| `DB_CONNECTOR_LOGIN` | `veyndo_connectors` | Connector service login granted DML on `connector_` tables, `USAGE,SELECT,UPDATE` on their sequences and `SELECT` on `commerce_migrations`. An explicitly named login must exist; the default is skipped when absent. |
| `DB_SYSTEM_LOGIN`, `DB_TENANT_LOGIN` | unset | Both set: `--database-authority configure` with these logins, then `activate`. Both unset: an existing configuration is re-applied, a missing one is reported and left alone. |
| `PLATFORM_ADMIN_EMAIL`, `PLATFORM_ADMIN_PASSWORD` | unset | Both set: the first operator is bootstrapped as by `--bootstrap-operator` (also set `SEED_DEMO=false` and `ALLOW_BOOTSTRAP_AUTH=false`). |

The command is idempotent. It then verifies and prints the coverage of each role
(`granted/total` tables and sequences), the system login's reach through the runtime
group, the tenant login's scoped tables, that the tenant login cannot create temporary
objects, and the authority `status`. Any gap fails the command with the role and the
count; it prints no secrets.

Every `BOOTSTRAP_MODE=migrate` run applies the same grant part after the baselines:
the runtime group (when it exists) and the connector login (`DB_CONNECTOR_LOGIN`, or
the default when it exists) receive the current tables, and an existing authority
configuration is re-applied so the tenant login reaches tables a release adds. It
never activates a staged authority. Set `DB_RUNTIME_ROLE` and `DB_CONNECTOR_LOGIN` on
the migration job when your names differ from the defaults.

Ollama is an operator-controlled private service. Compose does not download a
model or reserve a GPU. Optional OpenAI/Anthropic credentials stay in Rust.
The [connected-app service](connected-apps.md) is optional; configure private
Google/Slack clients, encryption/gateway keys and the actual callback URL before
starting the `connected-apps` profile. Storyfront remains independently deployed;
configure its admitted tenant mappings and `APP_SERVICES` as in
[Storyfront setup](storyfront.md). Creating a commerce shop does not automatically
provision those external services or enable real payment credentials.

## Deploy the frontend on Vercel

Only after the backend is reachable over valid HTTPS:

```sh
python3 scripts/hosting_check.py --origin https://YOUR-BACKEND-DOMAIN
python3 scripts/prepare_vercel.py --backend https://YOUR-BACKEND-DOMAIN
```

Create the dedicated `veyndo` Vercel project from this GitHub repository.
Select `frontend` as root, Vite, `npm run build`, and `dist`. **Enable including
source files outside the root directory**: the frontend imports the shared
`extensions/sdk/analytics` module. This is also covered by the corrected Docker
build. Keep the generated `extensions/apps/vercel/frontend/vercel.json` in the deployment checkout or
configure the same rewrites in the deployment's committed environment-specific
configuration; it is ignored by this generic source repository and must be
supplied before a Git-based build. Do not import a project with placeholder
rewrites and call it a working deployment.

Set `COMMERCE_PUBLIC_ORIGIN` on Rust to the HTTPS origin of the Studio browser.
MCP validates this configured origin; a proxy Host header cannot grant browser access.
For a separate Vercel Studio, use its real public origin rather than the internal Rust host.

Generated configuration proxies `/api`, `/store-api`, `/mcp`, `/ucp`,
`/.well-known`, `/media` and `/health` to the backend and serves the SPA. It contains
no credentials. Vercel supports [external rewrites](https://vercel.com/docs/routing/rewrites)
and [Vite deployments](https://vercel.com/docs/frameworks/frontend/vite). Private
API responses are marked non-cacheable. Authenticated/personalized results must
never be shared across tenants by CDN rules. Run the HTTPS check against the
Vercel preview too, then verify operator sign-in, creation of an isolated shop,
customer registration/address checkout and own order access before promoting it.

Use `?shop=SHOP_ID` and optionally `&channel=CHANNEL_ID` for storefronts.
`/#platform` opens the operator console; `?shop=SHOP_ID#merchant` opens a shop's
merchant studio. Staging previews use the existing private merchant session.
`SHOP_DOMAIN_SUFFIX` supports operator-configured shop subdomains with trusted
host-to-tenant binding; see [managed hosting](managed-hosting.md). Automatic
DNS/certificate provisioning for arbitrary merchant domains remains unimplemented.

## Local release checks

```sh
docker build -f deploy/Dockerfile -t veyndo:platform .
# With the existing local Compose PostgreSQL container and private DATABASE_URL:
python3 scripts/hosting_container.py
```

This creates and removes only a uniquely named synthetic test database/container,
checks the built frontend, non-root Rust process, personal operator login,
actual shop creation and closed signup/bootstrap gates. It is distinct from a
public HTTPS/Vercel deployment. `platform.py` and `platform_setup.py` are mandatory
HTTP/PostgreSQL CI tests.

Additional public deployments need their own host/domain and operator identity.
Broader production SaaS requires recovery/email verification, full provider/token/spend quotas, signed app trust, billing,
large-catalog staged branches, backups/restores and measured failover. The
[operator guide](platform.md) describes exactly what the dashboard measures.

The Northflank topology, cost model and migration boundaries are in [managed-hosting.md](managed-hosting.md).

## Central provider secrets and shop domains

With wildcard DNS/TLS and `SHOP_DOMAIN_SUFFIX=veyndo.ai`, new shops use
`SHOP.veyndo.ai`; `admin.veyndo.ai` is the public service directory. Set
`COMMERCE_PUBLIC_ORIGIN=https://app.veyndo.ai` for the shared merchant Studio.
Browser sessions are admitted only on that origin. A refused browser request names it
in the `x-veyndo-studio-origin` header, so the directory links both sign-ins there and
`/#platform` or a merchant sign-in opened on another host moves to it; the operator
sign-in reports such a host separately from an account without platform access.
Provision `PLATFORM_SECRET_KEY` as a persistent runtime-only 64-character random
hex key before saving provider API keys in the operator console. All replicas and
AI workers need the same key; back it up separately. Do not rotate it without
re-encrypting existing credentials. `INFERENCE_ALLOW_LOOPBACK` stays false in
public deployments. [Control-plane behavior and boundaries](platform.md).

Set `SOVEREIGN_AI=true` to admit only self-hosted models for every shop; the
operator console then shows the mode as fixed. `SOVEREIGN_MODEL_HOSTS` lists
model hosts you operate under a public name. A cluster-internal model address
such as `http://ollama:11434` comes from `OLLAMA_URL`, because the console
accepts only HTTPS endpoints. [Sovereign AI mode](sovereign-ai.md).

## Optional private experience service

The generic trusted broker, one-use Studio handoff and hosted frontend mount are opt-in. Configure the exact private service issuer/origin and matching runtime-only shared keys; see [experience integration](experience-integration.md). The private service and Storyfront never belong in this public build or repository.

### Storefront product addresses

Public storefronts use `https://SHOP.veyndo.ai/`. The shared Studio deliberately remains at
`https://app.veyndo.ai/?shop=SHOP#merchant` so merchant login stays on its original domain.
Legacy product bookmarks discard stale `studio` parameters and move to the shop domain.
Product links use `/products/SKU/LOCALIZED-SLUG` (or `/products/SKU` without a slug),
with the selected content language and sales channel retained. SKU identity prevents two
same-name products from colliding; translated slugs inherit through the existing language
chain. The server admits direct HTML requests through the same tenant, active-product
and sales-channel checks as the Store API. Browser navigation and Back/Forward retain
cart state, and reloads serve the application at that URL. Older slug addresses still find
the SKU and canonicalize to the currently saved slug. Metadata and canonical tags are
rendered by the client; server-side product HTML, sitemap generation and slug-only
redirect history are not implemented by this change.

## Strict runtime and real transaction pooling

The production Compose template separates migration owner, commerce runtime and
connector credentials. Migrations finish before `veyndo --provision`; commerce starts
in `serve` with strict RLS, no bootstrap auth and no demo fallback.

`DB_CLUSTER_CONNECTION_BUDGET` is shared by all HTTP/worker processes. Each reserves
`DB_POOL_MAX + 2`; set `DB_MAX_PROCESSES` and the same budget on every replica.
Transaction pooling requires PgBouncer 1.21+, protocol prepared-statement support
and a **direct/session** `DATABASE_LISTENER_URL` under the non-owner login. Set
`DB_POOLER_MODE=transaction`; statement pooling is unsupported. [Security boundaries](security.md).

## Production image compile-time dependencies

Run `python3 scripts/testing/image_context.py` before building `deploy/Dockerfile`.
This CI gate reads literal Rust `include_str!`/`include_bytes!` dependencies and
checks them against the Rust stage COPY set. Its negative control removes the
historical app-manifest COPY and must find the missing inputs. These files are used by app approval and package compatibility checks; tests against a full checkout
alone cannot detect their absence from a container build. The final image still
contains no Python interpreter. A green dependency check does not replace an
actual image build.
