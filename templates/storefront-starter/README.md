# Headless storefront starter

A small server-rendered storefront on the generated TypeScript SDK
(`@veyndo/commerce-client`): a catalog page and product pages, in English,
German, Spanish and French. No framework, no build step, no credential: the
Store API is public and every request names the shop with `x-tenant`.

| File | Job |
| --- | --- |
| `src/sdk.ts` | The one import of the SDK |
| `src/shop.ts` | `catalog()` and `product()` through `client.catalog.*` |
| `src/render.ts` | Escaped HTML; prices formatted for the language |
| `src/text.ts` | Interface words in four languages |
| `src/server.ts` | `node:http` server: `/`, `/product/<id>`, `?lang=` |

## Run it against your shop

Node 22.18 or newer runs the TypeScript directly.

```sh
npm test
```

```sh
VEYNDO_URL=https://api.veyndo.ai VEYNDO_SHOP=<shop id> npm start
```

Open http://localhost:3000. Use your live shop ID: private sandboxes refuse
anonymous Store API reads, so a storefront cannot read them. Deploy the folder
to any Node host with the same two variables.

## Next steps

- Cart and checkout: create a cart with `client.checkout.postStoreCheckoutCart`
  on your server and keep its `sw-context-token` in your own session, as in
  [the hosted API guide](../../docs/hosted-component-api.md).
- [API reference](../../docs/reference/catalog.md) for every catalog call.

Inside this repository `src/sdk.ts` imports `sdks/typescript/src`; `veyndo app
new storefront-starter` points it at a vendored copy of the same files.
