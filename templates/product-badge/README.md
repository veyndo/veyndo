# Product badge with settings

A short badge such as **New**, **Sale** or **Sustainable** on product pages.
Merchants set it in the product's **Badge** tab; a settings page under
**Apps → Badge settings** holds the default style and whether badges show in
the storefront. Shoppers can only read badges.

| Part | Where |
| --- | --- |
| Product tab editor | `admin.product.tab` surface, view `workspace` |
| Product page badge | `product.detail` surface, view `shopper`, public read `list_badges` |
| Settings | `admin.navigation` surface, view `settings` (private record) |
| Storefront route | `GET /store-api/apps/<app id>/http/badges` |

## Test, deploy, see it

```sh
npm test
```

```sh
VEYNDO_URL=https://api.veyndo.ai VEYNDO_SHOP=<shop id> VEYNDO_TOKEN=<personal session> npm run deploy
```

The deploy creates a private sandbox, imports `manifest.json` as a build and
installs exactly that digest. Open Studio, choose the sandbox in the top bar,
open a product and the **Badge** tab. The live shop is untouched.
[Build your first app](../../docs/build-your-first-app.md) walks through it.

## Change it

- Edit `manifest.json`: add a style to both `tone` and `default_tone`, then
  run `npm test` (the tests check that both lists agree).
- Every label needs English, German and Spanish; French is included too.
- Raise `version` for each new build. A version is immutable once staged.

`scripts/deploy-sandbox.mjs` and `tests/manifest.test.mjs` are shared by all
app templates; keep them unchanged so `veyndo app new` can refresh them.
