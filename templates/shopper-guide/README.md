# Shopper-facing MCP App

A product guide that a shopping assistant shows as an interactive view. The
shopper view (`storefront.assistant`) reads public product-care records with
read-only MCP tools; merchants edit them in the product tab and see private
merchandising notes in their own assistant (`admin.assistant`).

| Part | Where |
| --- | --- |
| Product tab editor | `admin.product.tab`, `list_entries` / `save_entries` |
| Shopper view in an assistant | `storefront.assistant`, public read tool `list_entries` |
| Merchant view in Studio's assistant | `admin.assistant`, `list_merchant_entries` (`apps.read`) |

It is the [Product Guide](../../docs/product-guide-mcp-app.md) package with a
new app ID; that guide explains the two views and how a JavaScript host calls
them.

## Test, deploy, see it

```sh
npm test
```

```sh
VEYNDO_URL=https://api.veyndo.ai VEYNDO_SHOP=<shop id> VEYNDO_TOKEN=<personal session> npm run deploy
```

In the sandbox, add a care entry to a product, then ask the storefront
assistant about that product. MCP clients find the tool with `tools/list` on
the sandbox's `POST /mcp`. [Build your first app](../../docs/build-your-first-app.md).

`scripts/deploy-sandbox.mjs` and `tests/manifest.test.mjs` are shared by all
app templates; keep them unchanged so `veyndo app new` can refresh them.
