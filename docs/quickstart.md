# Run Veyndo locally

Build and run Veyndo from source. This is the development path. If you only
want to try the product, the [first tour](first-tour.md) starts a prepared
image without a Rust or Node build.

## What you need

- Rust stable 1.96+ (`cargo` on your PATH)
- Node.js 22 or newer and npm
- Docker with Docker Compose and a running daemon
- Python 3
- Free ports 8787 (application), 15487 (database) and 16333 (private Qdrant)

Ollama and API credentials are optional. Commerce works without a model.

## 1. Start commerce

```sh
git clone https://github.com/veyndo/veyndo.git
cd veyndo
./scripts/dev.sh
```

The first start pulls the PostgreSQL and Qdrant images and builds the frontend
and the Rust application. Keep the terminal open and open
http://127.0.0.1:8787/.

The storefront, variant selection, quantity pricing, cart and simulated
checkout work without a running model. Chat planning and vector indexing need
their model services and report an error when those are unavailable.

## 2. Create your shop

Open Studio at `/#merchant` and choose **Create shop**. Create a personal owner
account and a synthetic shop. One identity can belong to several shops.

The default Nord Atelier storefront has 12 parent products, 34 purchasable SKUs
and bundled generated photographs. [Fashion tour](fashion-demo.md). Payments in
the default tour are simulated or manual.

## 3. Stop and restart

Stop the app with Ctrl+C and start it again with `./scripts/dev.sh`. Database
data is kept. Stop the database with `docker compose -p veyndo stop`. Do not
remove its volume if you want to keep orders and accounts.

## Add AI (optional)

**Local models.** Install and start Ollama, then download the documented models:

```sh
ollama pull qwen3.6:35b
ollama pull qwen3-embedding:0.6b
```

The Qwen3.6-35B-A3B artifact is about 24 GB, and runtime and context need more
memory. It is not a lightweight default for every laptop. Select a compatible
installed model with `OLLAMA_MODEL` in the private `.env`. Smaller alternatives
are not claimed as verified by this project. An existing `.env` keeps its
previous model selection.

Restart the app after changing `.env`. In Studio, choose Local in **Settings**,
then **Refresh shop knowledge** when the embedding service is ready.

Try a bounded request such as: “Propose a price of EUR 159 for the bag. Do not
apply it yet.” Inspect the actual change fields before approving. The model's
summary alone is not an execution guarantee.

**OpenAI or Claude.** Configure your own server-side API credentials with the
[provider guide](connectors.md). API access is separate from consumer chat
subscriptions and may incur charges.

## Connect an MCP client (optional)

Follow [Claude Desktop and local MCP clients](connectors.md#claude-desktop--local-mcp-clients).
The bridge connects to your running Rust app and does not start the server.
Hosted clients cannot reach localhost; their setup is separate. To see the
calls an agent makes, try the [agent purchase](agent-purchase.md) first.

## Troubleshooting

| Symptom | Fix |
| --- | --- |
| `cargo: command not found` | Install Rust or put the existing toolchain on PATH. |
| Cannot connect to Docker | Start Docker and check that `docker info` succeeds. |
| Port already allocated | Run a [separate instance](#run-a-second-instance) on other ports. |
| AI unavailable | Commerce still works. Check the chosen provider and model service. |
| First build is slow | Images are downloaded and Rust and frontend dependencies are built. |

## Advanced

### Run a second instance

Use a different Compose project and other database and application ports on a
fresh checkout:

```sh
DB_PORT=15489 QDRANT_PORT=16335 BIND_ADDR=127.0.0.1:8789 \
COMPOSE_PROJECT_NAME=veyndo-second ./scripts/dev.sh
```

`DB_PORT` is used when generating a new `.env`. For an existing `.env`, update
its `DATABASE_URL` to the chosen port and keep `QDRANT_URL` consistent with
`QDRANT_PORT` if it is already configured. Use the same project name and ports
when restarting; stop its database with `docker compose -p veyndo-second stop`.
Never point a test run at a shop whose data you need to preserve.

### Run setup, HTTP and workers separately

Local development defaults to `BOOTSTRAP_MODE=auto` and `PROCESS_ROLE=all`.
For independently operated processes, run setup once before starting replicas:

```sh
BOOTSTRAP_MODE=migrate target/release/veyndo
BOOTSTRAP_MODE=serve PROCESS_ROLE=http target/release/veyndo
# Separate terminals/processes, using the same private database configuration:
BOOTSTRAP_MODE=serve PROCESS_ROLE=memory-worker target/release/veyndo
BOOTSTRAP_MODE=serve PROCESS_ROLE=payment-worker target/release/veyndo
BOOTSTRAP_MODE=serve PROCESS_ROLE=app-worker target/release/veyndo
BOOTSTRAP_MODE=serve PROCESS_ROLE=translation-worker target/release/veyndo
BOOTSTRAP_MODE=serve PROCESS_ROLE=media-worker target/release/veyndo
```

The HTTP role does not consume the durable outbox. The memory worker creates
commerce projections and app deliveries; the app worker delivers them. Serve
refuses an incomplete setup. Schema checksums detect changed applied migrations
when running setup; keep applied source immutable and add new migration files.
Plan the initial index build before serving a large existing database. This
process separation is a foundation, not a production deployment recipe.

### Accounts and credentials in a local instance

The generated global `MERCHANT_TOKEN` is an instance bootstrap credential. It
is not an invitation and not a merchant credential to distribute.

The legacy `atelier`/`workshop` compatibility shops can contain the synthetic
B2B customer `buyer@example.test` / `demo-business` when demo seeding is
enabled. This is separate from merchant accounts and is not a universal login
for every new shop.

A separately configured native PayPal adapter supports Sandbox and Live. See
[hosted PayPal acceptance](shopware-payments-acceptance.md) for the observed
Sandbox journeys and their scope.

### The legacy furniture playground

The playground CLI is the **legacy furniture compatibility tour**, with
mug and lamp IDs. Set `DEMO_CATALOG=legacy-furniture` in the private `.env` and
restart the app before creating it. Existing shops are not rewritten. For the
current Nord Atelier walkthrough, use the [feature guide](features.md) with the
normal `DEMO_CATALOG=fashion` default.

After creating your personal merchant account on the compatibility instance:

```sh
python3 scripts/playground.py --email your-personal-merchant@example.test
```

Enter that account's password privately. This creates a separate **Commerce
Playground** with a rule, `TRY10` coupon, Home collection sales channel,
engraving app and active branching invoice flow. The script prints its Studio
and storefront links. Repeating setup preserves your edits. Setup makes no AI
or provider calls and places no orders. [Ten-minute playground walkthrough](playground.md).

### An older local checkout

A database created by an earlier release is refused at setup: this release is a
fresh installation only. `scripts/dev.sh` still detects an older `rust-ai-commerce`
Compose project and would reuse its database; set `COMPOSE_PROJECT_NAME=veyndo` (or
another new name) to start with a new database instead. Rebuild frontend and Rust
before starting `target/debug/veyndo`.

[Project overview](../README.md) · [Feature guide](features.md) · [Security scope](security.md)
