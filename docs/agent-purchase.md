# Let an agent buy from your shop

A shopping agent uses the same catalog, cart and checkout as a person in the
storefront. This guide walks through that purchase against the public Lumen
demo shop. You need a terminal with `curl`. You do not need an account, an API
key or a model.

Every step is one MCP tool call. An agent such as Claude or ChatGPT makes the
same calls after you [connect it](connectors.md).

## What you will see

1. The agent discovers what it is allowed to do.
2. It finds a product and reads the current price and stock.
3. It builds a cart and receives a binding quote from Commerce Core.
4. It chooses delivery and payment, accepts the terms and places the order.
5. The core refuses anything that breaks a rule, and says why.

Payments in the demo are simulated. No money moves.

## Set up the call

All calls go to one endpoint. The shop is named in the `x-tenant` header.

```sh
SHOP=lumen-story-lab-15-o-98aa4751fa
mcp() {
  curl -fsS https://api.veyndo.ai/mcp \
    -H "x-tenant: $SHOP" \
    -H 'Content-Type: application/json' \
    -H 'Accept: application/json' \
    "$@"
}
```

## 1. Discover the tools

```sh
mcp --data '{"jsonrpc":"2.0","id":1,"method":"tools/list"}'
```

The public shop answers with the tools an anonymous shopper may use, such as
`catalog.search`, `cart.create`, `cart.quote` and `checkout.complete`. Merchant
tools are absent. A tool name is a way in, not a permission: the shop checks
the caller's current rights on every call.

## 2. Find a product

```sh
mcp --data '{"jsonrpc":"2.0","id":2,"method":"tools/call",
  "params":{"name":"catalog.search","arguments":{"query":"lamp","limit":3}}}'
```

The result is the same product data the storefront shows, in the shop's
language and currency. Note the product `id`, `price` and `stock`.

## 3. Create a cart

```sh
mcp --data '{"jsonrpc":"2.0","id":3,"method":"tools/call",
  "params":{"name":"cart.create","arguments":{}}}'
```

The response contains a cart `token` and a `revision`. The token identifies
this cart. Send it with every following call:

```sh
CART=paste-the-token-here
```

## 4. Add an item and ask for the quote

Replace `PRODUCT_ID` with the `id` from step 2. The `revision` must match the
cart's current revision, so two writers cannot overwrite each other.

```sh
mcp -H "sw-context-token: $CART" --data '{"jsonrpc":"2.0","id":4,"method":"tools/call",
  "params":{"name":"cart.replace","arguments":{"revision":1,
    "items":[{"id":"PRODUCT_ID","quantity":1}]}}}'

mcp -H "sw-context-token: $CART" --data '{"jsonrpc":"2.0","id":5,"method":"tools/call",
  "params":{"name":"cart.quote","arguments":{}}}'
```

The quote is calculated by Commerce Core, not by the agent. It contains the
line items with unit price, tax and the stock the core sees. On 17 November
2026 the demo lamp returned 59.90 EUR including 9.56 EUR tax at 19 %.

### Optional: get the quote signed

Call the tool `cart.offer` with the same cart token. The answer carries the
quote with an Ed25519 signature that the agent can check against the shop's
public key at `/.well-known/commerce-facts.json`. See
[signed offers](agentic-commerce.md#signed-offers).

## 5. Choose delivery and payment

```sh
mcp --data '{"jsonrpc":"2.0","id":6,"method":"tools/call",
  "params":{"name":"checkout.options","arguments":{}}}'
```

Pick a shipping method and a payment method from the answer and set them with
`checkout.select`, together with the country, the delivery address and the
buyer's email. Use the cart's current `revision`.

## 6. Accept the terms and place the order

`privacy.policy` returns the current legal documents and their
`policyVersion`. `legal.accept` records the buyer's acknowledgement for that
version. Then place the order:

```sh
mcp -H "sw-context-token: $CART" --data '{"jsonrpc":"2.0","id":9,"method":"tools/call",
  "params":{"name":"checkout.complete","arguments":{"idempotency_key":"demo-order-1"}}}'
```

Repeating the call with the same `idempotency_key` returns the same order. It
does not create a second one and does not take stock twice.

## What the core refuses

Each of these fails with a structured error instead of a wrong order.

| You try | The core answers |
| --- | --- |
| A cart call without the cart token | `authentication_required`: customer context required |
| `cart.replace` with an old `revision` | The write is rejected. Read the cart again and retry with its current revision. |
| A merchant tool over the public connection | The tool is not listed for an anonymous shopper. |
| Ordering stock that is not there | Checkout checks the current stock when the order is placed. On 17 November 2026 both demo products showed stock 0. |

## Why this matters

The agent never held a price list, a stock count or a discount rule. It asked,
and the shop answered with the same numbers a person would see. If you change
a price in Studio, the next quote reflects it. If you revoke a right, the next
call fails.

That is the difference between an agent that scrapes a shop and a shop that is
built for agents.

## Next steps

- **Connect a real agent.** [OpenAI, Claude and local MCP clients](connectors.md).
- **Hand a cart to a browser checkout.** [UCP Cart Transfer](agentic-commerce.md#ucp-cart-transfer)
  returns a `continue_url` where the buyer finishes in the normal checkout.
- **Show a product guide inside the conversation.** [Build an MCP App](product-guide-mcp-app.md).
- **Give a merchant agent a bounded job.** [Agent workshop](agent-workshop.md) and
  [agent limits](agent-authority-controls.md).
- **Do the same from JavaScript.** [Hosted API and SDK](hosted-component-api.md).

## What was checked

Steps 1 to 4 were run against the public Lumen demo on 17 November 2026,
including the cart token, the quote and the missing-token error. Steps 5 and 6
and the other refusals follow the published tool schemas and guides and were
not executed for this guide. The hosted status of checkout, payments and mail
delivery is recorded on the [status page](status.md).
