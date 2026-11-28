# Order-tag automation

Staff tag an order (**VIP customer**, **Gift**, **Needs a check**) in the
order's **Tags** tab. A flow, `flows/tag_large_orders.json`, runs on
`order.placed` for carts of 250 or more and calls the app's `tag_suggested`
action. The host binds the canonical order ID into the declared `orderId`
input; the flow supplies only the tag. The action publishes a typed app event
that other flows, apps or a notification can react to.

| Part | Where |
| --- | --- |
| Order tab | `admin.order` surface, view `workspace`, private `tags` records |
| Manual tag | `save_tags` (`orders.write`, also allowed in flows) |
| Automatic suggestion | `tag_suggested`, an `emit` action allowed in flows |
| Flow | `flows/tag_large_orders.json` |

A flow cannot write the suggestion into a tag record by itself, because flow
arguments are fixed values and only declared IDs are bound. To store automatic
tags, add a service action as in the [ERP handoff workshop](../../docs/app-tutorials/erp-handoff.md).

## Test, deploy, see it

```sh
npm test
```

```sh
VEYNDO_URL=https://api.veyndo.ai VEYNDO_SHOP=<shop id> VEYNDO_TOKEN=<personal session> npm run deploy
```

The deploy also saves `flows/tag_large_orders.json` as the flow
`tag_large_orders` in the sandbox (a flow ID is the file name: letters,
digits and underscores). Open Studio in the sandbox, open an order and its
**Tags** tab, and find the flow under **Flows**. Schedules and external
delivery are disabled in sandboxes. [Build your first app](../../docs/build-your-first-app.md).

`scripts/deploy-sandbox.mjs` and `tests/manifest.test.mjs` are shared by all
app templates; keep them unchanged so `veyndo app new` can refresh them.
