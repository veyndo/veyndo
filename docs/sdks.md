# Component contracts and generated SDKs

Every Veyndo component has a machine-readable contract, and three client SDKs are
generated from those contracts: TypeScript, Python and Rust. Nothing here is written
by hand that the server already declares, and nothing is published from the monorepo
yet. Versions and the release flow are in [component mirrors](component-mirrors.md#versions-changelogs-and-the-dry-run-publish).

## The contracts

`python3 scripts/sdks/contracts.py` writes `contracts/http/<component>.openapi.json`
for each of the 22 components in `components.json` and the combined
`contracts/http/veyndo.openapi.json` (OpenAPI 3.1). Its inputs are all checked
against the compiled server elsewhere:

| Part of the contract | Source |
| --- | --- |
| Paths and methods | the `.route(...)`/`.secure_route(...)` declarations in `src/`, the same scan as `scripts/api_catalogue.py`; the generator fails if its list differs from `frontend/src/kit/api/generated/routes.ts` |
| Permission per method (`x-veyndo-permission`, `security`) | the rights list each `secure_route` registers (`public`, `read`, `platform` or a scope such as `catalog.read`) |
| Error envelope (`Problem`, `ErrorEnvelope`) | `Problem`, `Kind` and `Failure::problem` in `components/core/src/failure.rs` |
| Typed request and response schemas (`x-veyndo-tools`) | the native operation registry, `extensions/sdk/generated/operations.json`, which `scripts/testing/operation_contracts.py --check` keeps equal to `veyndo --operation-contracts` |
| Typed operation facts (`x-veyndo-tools[].facts`, `cost`, `requiresApproval`) | the same registry export; see [the typed operations graph](business-capabilities.md#the-typed-operations-graph). A newly required permission or approval is a major release change |
| Owning component | `contracts/ownership.json`, longest path or name prefix |
| Version (`info.version`) | `components.release.json`, derived from contract changes |

Ownership is declared, not guessed: an unowned new route or operation fails the
generator, and a handler that names a component crate (`veyndo_orders::http::…`) must
agree with the declared owner. Components without routes or tools (core, automation,
inference, connectors) still get a contract, with empty `paths`, so their version can
follow the rest.

**What is typed and what is not.** The HTTP handlers take and return
`serde_json::Value` (167 of the 180 `Json<…>` request extractors; nearly every handler returns `Json<Value>`), and the workspace has no `schemars`, so
HTTP request and response bodies are declared as open JSON. The typed schemas are the
212 native MCP tools: their input schemas are the ones the server validates before
executing, and `automation.validate_flow` also declares its output. A typed HTTP body
needs a typed Rust DTO first; the generator will not invent one.

## The SDKs

`python3 scripts/sdks/generate.py` regenerates the contracts and all three SDKs;
`--check` fails on any drift and runs in `scripts/structure.py`, the quick CI path
(under one second). Generated code is committed.

| | TypeScript | Python | Rust |
| --- | --- | --- | --- |
| Folder | `sdks/typescript` | `sdks/python` | `sdks/rust` (workspace crate `veyndo-sdk`) |
| Package | `@veyndo/commerce-client`, ESM | `veyndo-commerce-client` | `veyndo-sdk` |
| Runtime dependencies | none (`fetch`) | none (`urllib`, dataclasses) | `reqwest`, `serde`, `serde_json` |
| Client | `new VeyndoClient({ baseUrl, auth, tenant })` | `VeyndoClient(base_url, auth=..., tenant=...)` | `Client::new(ClientOptions { .. })` |
| One API object per component | `client.catalog.postStoreProduct(...)` | `client.catalog.post_store_product(...)` | `client.catalog().post_store_product(...)` |
| Authentication | `Auth.personalSession(token)`, `Auth.integrationKey(key)` | `Auth.personal_session`, `Auth.integration_key` | `Auth::PersonalSession`, `Auth::IntegrationKey` |
| Typed errors | `VeyndoError` (`status`, `kind`, `problems`, `retryable`), `McpProtocolError` | same names | `Error::Api(ApiError)`, `Error::Mcp`, `ProblemKind` |
| Pagination | `paginate(cursor => ...)` async iterator | `paginate(...)` generator | `collect_pages(...)` |
| MCP | `client.mcp.call("catalog.search", { limit: 2 })`, typed by `ToolInputs` | `client.mcp.call(CatalogSearchInput(limit=2))` | `client.mcp().call(&CatalogSearchInput { .. })`, output type from the `Tool` trait |

Method names come from the method and path: `GET /api/merchant/products/{id}` is
`getMerchantProductsById`, a `/store-api/...` route keeps a `Store` prefix. Every
client sends the shop as `x-tenant`, the sales channel as `sw-sales-channel-id` and a
storefront context as `sw-context-token`; `with(...)`/`with_options(...)` returns a
client with another credential or context. Both credentials are bearer tokens; keep
integration keys on your server.

Pagination follows the server's cursor convention: a page carries `nextCursor` (and
often `hasMore`), and the next request passes it as the route's cursor field, `after`
for catalogue, product and customer lists. The helpers take a function from cursor to
page so they work for every list route.

A failure is the envelope `{"errors":[{"kind","code","status","detail","retryable",
"scope"?,"pointer"?,"currentRevision"?}]}` on HTTP, and the same problems inside
`structuredContent.errors` of an MCP tool result with `isError: true`; the SDKs raise
both as the same error type. JSON-RPC protocol errors (for example an unknown tool,
`-32602`) are a separate type.

The TypeScript SDK runs unbuilt on Node 22.18 or newer (type stripping) and builds to
`dist/` with `tsc -p tsconfig.build.json`. Hand-written and not generated: each SDK's
`examples/` and smoke test, and this page. The handbook's [API reference](reference/README.md)
shows a call for every route and tool in all three SDKs and curl, rendered from the same
contracts with these method names.

## Smoke test against a real server

The HTTP suite `sdk_smoke` runs all three SDKs against the suite server:

```bash
cargo build --workspace --bins
```

```bash
python3 scripts/verify_integration.py --container veyndo-postgres-1 --only sdk_smoke
```

Each SDK checks health, reads the demo catalogue page by page and compares it with
the unpaged list, gets typed `validation_error` (HTTP and MCP, with the JSON pointer),
`authentication_required` and `permission_denied` failures and a JSON-RPC protocol
error, registers a synthetic merchant account (personal session), creates an
integration key limited to `catalog.read`, lists and calls merchant tools with it, and
is refused an identity route the key does not cover. The tooling suite
`testing/sdk_contracts_test` holds the generators' negative controls.

## Not covered yet

- Typed HTTP bodies (see above), query parameters (passed as an open map) and
  streaming routes (`/api/agent/chat/stream`), which the SDKs return as text.
- Runtime-discovered app routes (`/api/apps/{id}/http/{route}` covers them generically).
- Publishing to npm, PyPI or crates.io; the dry run in
  [component mirrors](component-mirrors.md#versions-changelogs-and-the-dry-run-publish)
  lists what would be pushed.
