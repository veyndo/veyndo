# Start here: one shop for people and agents

Veyndo is a commerce platform where a person and an AI agent use the same
shop. A shopper can click through the storefront, or an assistant can search,
quote and order on their behalf. A merchant can edit a product in Studio, or
ask an agent to prepare the change. A developer can build an app by hand, or
describe it to a coding agent.

In all of these cases one Rust core decides the price, the stock, the
permissions and the order. That is the whole idea.

## What agents do in Veyndo

| | What happens | See it |
| --- | --- | --- |
| **Buy** | A shopping agent reads the catalog, gets a binding quote and places an order through the same checkout as the storefront. | [Let an agent buy from your shop](agent-purchase.md) |
| **Work** | A merchant agent assembles a working view, prepares changes in private and runs only the steps you approve. | [The AI workspace](agentic-workspace.md) |
| **Build** | You describe an app. Studio and coding agents share one versioned definition with its own data, views and actions. | [App Studio](app-studio.md) |

## What never changes: the core decides

A model can propose. It cannot decide that a payment arrived, that someone is
allowed to do something, or that stock can be sold.

- **One owner for money and stock.** Commerce Core calculates the price,
  allocates stock and records the order in one transaction.
- **The same operations for everyone.** The storefront, Studio, the SDK, HTTP,
  MCP and UCP call the same operations with the same permission checks.
- **Review before effect.** Agents prepare changes against a known revision.
  You approve them, or you set explicit limits within which they may act.
- **The shop keeps selling.** Knowledge, reporting and model providers can be
  unavailable without stopping checkout.

[How a request travels through the core](production-architecture.md) ·
[What the Lean proofs cover](formal-verification.md).

## Pick your path

| You want to | Start with | Time |
| --- | --- | --- |
| See an agent shop | [Let an agent buy from your shop](agent-purchase.md) | 5 minutes, no account |
| Build and deploy an app | [Build your first app](build-your-first-app.md) | 10 minutes |
| Click through a complete shop | [Create a hosted shop](https://start.veyndo.ai/start) or follow the [first tour](first-tour.md) | 10 minutes |
| Call the API from your own code | [Use Veyndo as a hosted API](hosted-component-api.md) and the [API reference](reference/README.md) | 5 minutes |
| Understand the parts | [One shop, composable parts](one-shop-components.md) | 10 minutes to read |
| Build an extension | [Five app workshops](app-tutorials/README.md) | 30 minutes each |
| Run it yourself | [Local quickstart](quickstart.md) | 15 minutes with Docker |
| Know what is proven | [Current status](status.md) | Reference |

## What is ready and what is not

Veyndo is a working prototype. It is useful for trying the model and building
against it. It is not established production infrastructure.

| Works today | Not yet accepted |
| --- | --- |
| Hosted demo shops with simulated payments | Live payments with real money |
| Public catalog, cart and quote over HTTP, the SDK and MCP | Order mail delivery for a paid hosted purchase |
| PayPal Sandbox capture, return and refund in the hosted pilot | Capacity and disaster recovery under production load |
| App Studio, Flows and the customer simulator in Studio | Approval on external agent shopping surfaces |

The [status page](status.md) records each of these with its date and evidence.
The [roadmap](roadmap.md) shows what was added most recently and what comes next.

## How this handbook is organised

1. **Start here.** The idea, one agent purchase and your first complete round.
2. **Agents buy.** Shopping channels, intent shopping, checkout and UCP.
3. **Agents work.** The merchant workspace, knowledge, approvals and limits.
4. **Build.** Your first app, the SDK, the components and the app platform.
5. **API reference.** Every route and MCP tool, with Try-it panels.
6. **Run the shop.** Catalog, orders, payments, countries and channels.
7. **Stay in control.** Rights, sandboxes, releases and hosting.
8. **Why you can trust it.** Architecture, tests, proofs and measurements.

Each chapter starts with the guide most people need first. Dated acceptance
records and module notes stay searchable in the complete reference.
