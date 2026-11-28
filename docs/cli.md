# The `veyndo` command-line client

`veyndo` manages shops, catalogues, apps and agents from a terminal. It is a client: it calls
the public HTTP and MCP APIs with your own credential through the generated Rust SDK
(`sdks/rust`, crate `veyndo-sdk`), so every command has exactly the rights, tenant checks,
quotas and approvals the same call has from Studio or your own code. It has no database
access and no operator authority.

**Server and client are two programs.** The server binary built from the repository root is
also called `veyndo` (`target/*/veyndo`, `/app/veyndo` in the production image) and carries
the operator flags `--provision`, `--bootstrap-operator`, `--database-authority`,
`--operation-contracts` and `--sign-app`. The client lives in `sdks/cli` (crate `veyndo-cli`)
in its own Cargo workspace with its own target directory, so the two executables never
overwrite each other, and no production image contains the client.

## Install

From a repository checkout:

```bash
cargo install --locked --path sdks/cli
```

Release builds for macOS and Linux (x86_64 and arm64) are attached to `cli-v*` GitHub
releases by [the CLI release workflow](../.github/workflows/cli-release.yml); each archive
holds the binary, the license and bash/zsh/fish completions.

Shell completions:

```bash
veyndo completions zsh > "${fpath[1]}/_veyndo"
```

`veyndo completions bash|zsh|fish|elvish|powershell` prints the script for any shell.

## Sign in

```bash
veyndo login --email you@example.com
```

```bash
veyndo login --with-key --shop my-shop
```

| Way | What it stores | Notes |
| --- | --- | --- |
| `--email` | A personal session (12 hours) | Password and authenticator or recovery code are prompted without echo; `--password-stdin` reads the password from stdin. Needed for `shop list`, `shop create` and anything that requires a personal session. |
| `--with-key` | An integration key | Create it in **Developer → API & integrations** with the scopes you need. The key is read from stdin or a hidden prompt, never from an argument. A key belongs to one shop, so `--shop` is required. |
| `VEYNDO_API_KEY` + `VEYNDO_SHOP` | Nothing | For CI. The environment key takes precedence over a stored credential. |

The server offers no browser device sign-in yet; when it does, `login` will use it. The
credential is written to `credentials.json` in `$VEYNDO_CONFIG_DIR`, else
`$XDG_CONFIG_HOME/veyndo`, else `~/.config/veyndo`, with mode 0600 in a 0700 directory. It is
never printed: not by `status`, not in `--json` output, not in MCP snippets and not in error
messages. `veyndo logout` ends a personal session on the server and removes the file.

`veyndo status` shows the server, whether it is reachable, the selected shop, the credential
type and its granted permissions. It exits 0 even when signed out.

## Shops

```bash
veyndo shop list
```

```bash
veyndo shop use my-shop
```

```bash
veyndo shop create my-second-shop --name "Second shop"
```

`create` needs a live personal session and the same account rights as Studio's *new shop*;
it selects the new shop. `--shop` (or `VEYNDO_SHOP`) overrides the selection for one command.

## Catalogue and orders

```bash
veyndo products list --search lamp --limit 20
```

```bash
veyndo products get <product-id> --json
```

```bash
veyndo orders list --state open
```

Lists follow the server's cursor pages until `--limit`. Tables show a summary; `--json` prints
the server's records unchanged, so scripts can use `jq`. `--sandbox <id>` reads a sandbox
instead of the live shop.

### Import a CSV

```bash
veyndo products import catalog.csv --tax-rate 19
```

The import uses the native catalogue intake (`/api/merchant/catalog/intake`, the same path as
Studio's *Add products → CSV*). Columns: `name`, `price` required; `description`,
`productNumber`, `imageUrl` optional; UTF-8, at most 50 rows and 200 kB per batch.
`--format shopify` reads a Shopify product export. Prices are in the shop currency;
`--tax-rate` is required when the shop has more than one rate.

The products are created in a **private sandbox**, never in the live shop: a new sandbox
named after the file, or `--sandbox <id>`. Each row gets a product ID derived from the sandbox
and the row, so running the same import again reports `unchanged` instead of creating
duplicates. A row you edit becomes a new product in the sandbox (the ID follows the row), so
fix mistakes in the sandbox or in a fresh one. Image URLs pass the same public-egress checks as
in Studio.

### Review and release

```bash
veyndo sandbox diff <sandbox>
```

```bash
veyndo sandbox release <sandbox>
```

`release` lists the changes and asks for confirmation; `--yes` approves without the prompt
(required without a terminal). Without `--key` it releases every change that does not
conflict with the live shop; `--key product:<id>` (repeatable) releases a selection. Each
change is released with the digest you reviewed, at most 50 per release; a change edited after
the diff is refused. `veyndo sandbox create` and `veyndo sandbox list` manage sandboxes; a
shop has at most ten.

## Apps

```bash
veyndo app new advisor my-advisor
```

```bash
veyndo app validate my-advisor
```

```bash
veyndo app deploy my-advisor
```

```bash
veyndo app logs my_advisor --sandbox <sandbox> --follow
```

`app new` without a template lists them. The templates are the app-owned patterns App Studio
offers (`advisor`, `returns`, `erp`, `edi`, `furniture`), copied with your Manifest ID and
version 0.1.0, plus a README; the [app tutorials](app-tutorials/README.md) explain each one.

`app validate` checks the files (JSON, ID, semantic version, English name, permissions, a
`contract.json` that names the same app and version) and then asks the server's compiler with
`POST /api/apps/review`, which writes nothing and returns the canonical digest, permissions and
added permissions. `--offline` skips the server. Findings exit with 10.

`app deploy` reviews the package and installs it **into a sandbox** with `approve: true`, the
reviewed digest, the complete permission list and the contract digest, exactly the consent
Studio asks for; it needs `apps.manage` and `team.manage`. A `contract.json` beside the
Manifest is sent along. For a signed package (Manifest `distribution`), sign the Manifest and
contract offline with the server binary (`veyndo --sign-app manifest.json contract.json <
seed`, see [signed packages](app-platform.md#signed-packages-versions-and-rollback)); the
installation verifies the signature against the operator's `APP_PUBLISHERS`. Published
versions are immutable: raise `version` for every changed package. Release the sandbox to
publish the app.

`app logs` reads the metadata-only activity log (`GET /api/apps/<app>/activity`): recent
calls with status and duration, and event deliveries with their state. It never shows
payloads. `--follow` polls every three seconds.

## Agents

```bash
veyndo ops list
```

```bash
veyndo ops check plan.json
```

`ops list` calls `merchant.operations.list` (the [typed operations graph](business-capabilities.md#the-typed-operations-graph)):
every operation the credential may call, with writes, risk, cost and whether it needs approval;
`--all` includes the ones it may not call now, `--name` selects some. `ops check` calls
`merchant.operations.check` with a plan, `{"steps":[{"name":…,"arguments":{…}}]}` or the bare
array (`-` reads stdin). It runs nothing and prints each step's verdict (`allowed`,
`approval`, `blocked`, `unknown`) with its reason codes, unmet prerequisites and argument
errors. A plan that is not executable exits with 10, so a CI job can gate on it.

```bash
veyndo agent eval -- --only pricing-raise-lamp
```

`agent eval` runs [the agent evaluation benchmark](agent-evaluation.md) from a repository
checkout (`--repo` or `VEYNDO_REPO`, else the nearest parent directory with
`scripts/agent_evaluation.py`); arguments after `--` go to the runner. It uses its own local
server and twin shops, never your hosted shop.

### MCP clients

```bash
veyndo mcp
```

prints the endpoint (`<server>/mcp`, shop in `x-tenant`) and configuration for Claude Code,
Claude Desktop and Cursor (`--client claude-code|claude-desktop|cursor` for one). The HTTP
snippets reference `$VEYNDO_API_KEY` instead of containing a key; give MCP clients a scoped
integration key, because a personal session expires after 12 hours. The Claude Desktop
snippet starts `veyndo mcp serve`, a stdio bridge that forwards each JSON-RPC line to the
server with the stored credential, so no secret goes into the client's configuration.

## Output and exit codes

Tables go to stdout and notes to stderr; `--json` makes stdout one JSON document (or one JSON
line per entry with `app logs --follow`).

| Code | Meaning |
| --- | --- |
| 0 | Success |
| 1 | Unexpected failure (server internal error, undecodable response, cancelled release) |
| 2 | Wrong command line, or a confirmation that needs a terminal |
| 3 | Not signed in, or the credential expired or was revoked |
| 4 | Permission denied for this shop or operation |
| 5 | Shop, sandbox, product, order or app not found |
| 6 | Invalid input: file, CSV, plan, manifest or argument value refused |
| 7 | Conflict with newer state; review again and retry |
| 8 | Server unreachable, busy, rate limited or over quota |
| 10 | A check reported findings (`app validate`, `ops check`, `agent eval`) |

## Server address

`--server`, `VEYNDO_SERVER` or the value stored at login; the default is
`https://api.veyndo.ai`. Plain HTTP is accepted only for loopback addresses
(`http://127.0.0.1:8080`, `http://shop.localhost:3000`).

## Development and tests

The crate is its own workspace, so its checks name its manifest:

```bash
cargo clippy --locked --manifest-path sdks/cli/Cargo.toml --all-targets -- -D warnings -W clippy::too_many_lines
```

```bash
cargo test --locked --manifest-path sdks/cli/Cargo.toml
```

The HTTP suite `cli_client` ([scripts/cli_client.py](../scripts/cli_client.py)) drives the
built client against the suite server: personal and key sign-in, wrong passwords and keys,
shop list/use/create, catalogue reads and pagination, CSV import into a sandbox with replay,
refused CSV and sandbox, diff and explicit release, app scaffold, offline and server
validation, deploy into a sandbox and logs, operations list and check, the MCP bridge,
completions and exit codes. It fails if any stored secret appears in any output.

```bash
cargo build --manifest-path sdks/cli/Cargo.toml
```

```bash
python3 scripts/verify_integration.py --container veyndo-postgres-1 --only cli_client
```

`scripts/testing/workspace_boundaries.py` also checks the client's own manifest: its only
local dependency is `veyndo-sdk`, and its external crates are a reviewed list.

## Not covered yet

- Browser device sign-in (the server has no device-authorization endpoint) and OS keychain
  storage; the 0600 file is the store on every platform.
- Creating or revoking integration keys from the CLI; use Studio or the identity API.
- Imports larger than one reviewed batch of 50 rows; split the file.
- Signing app packages in the client; signing stays an offline step of the server binary.
