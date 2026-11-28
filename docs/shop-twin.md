# Test a twin of your existing shop

A shop twin is a private copy that behaves like your shop: same catalog, prices,
rules, taxes, shipping and checkout. Fictional customers and scripted attackers
buy in the copy. Your live shop, its stock and its customers are not involved,
and you do not have to migrate anything first.

![Shop twin readiness in Studio](assets/roadmap/shop-twin-studio.jpg)

*Local Studio, synthetic shop, 18 November 2026.*

## Build the twin

1. Open **Staging & releases** and create a **private sandbox**. This is the twin.
2. Bring your catalog into it. Choose the path that fits your current shop:

   | Your shop today | Path | Guide |
   | --- | --- | --- |
   | Shopify | Upload the product export CSV | [Shop imports](agentic-commerce.md) |
   | Shopware 6, WooCommerce, PrestaShop | Migration Assistant with the twin as private target | [Bring an existing shop](agentic-migration.md) |
   | Anything with a spreadsheet | Generic CSV with name, description, price, product number, image | [Shop imports](agentic-commerce.md) |
   | Already on Veyndo | Nothing to import; the sandbox starts as a copy of your shop | [Sandboxes](workbench.md) |

3. Open **Shop knowledge → Customer simulator** with the sandbox selected. The
   **Shop twin** panel shows five steps and what is still missing.

![Twin after a CSV import: test stock set, attack test passed](assets/roadmap/shop-twin-import.jpg)

## Read the five steps

| Step | Done when | If it is open |
| --- | --- | --- |
| **Catalog** | The copy has purchasable items and the selected sales channel shows them | **Bring catalog** opens the import |
| **Test stock** | At least one item has stock | **Set test stock to 10** fills every item without stock; **Open products** sets it per product. Both stay in the copy |
| **Checkout** | A simulated payment method and a shipping method are active for the first delivery country | **Open settings** |
| **Customer tests** | You saved at least one customer test in this copy | Describe a goal below the panel and start |
| **Attack test** | The last saved attack test refused every attack and the configuration is unchanged since | Run the attacks below the customer profiles |

![A copy without stock is reported as not ready](assets/roadmap/shop-twin-open.jpg)

Imported products start with **zero stock**. That is deliberate: Veyndo does not
invent inventory. The twin therefore says "No item has stock" instead of letting
every test customer fail at checkout. **Set test stock to 10** gives every active
item without stock ten units through the normal product revision, search index
and product event, up to 500 items per click. Items that already have stock keep
it. The action exists only in a private sandbox and needs the right to edit the
catalog. Stock you set in a sandbox is never published to the live shop.

## Test the twin

- **Fictional customers** try a goal you describe and place simulated orders.
  [Customer simulator](customer-scenarios.md).
- **Scripted attackers** try to cheat cart and checkout.
  [Attack test](customer-scenarios.md#attack-test).
- **Recorded orders** show what a changed rule would have done.
  [Rule comparison](rule-history-comparison.md).

## What the twin is not

- It does not copy customers, orders or payment credentials from your current shop.
- It is not kept in sync with the source. Import again to refresh it.
- Fictional customers are test instructions, not a forecast of real conversion.
- Readiness checks the first delivery country and a sample of up to twelve items
  in the selected sales channel, not every country and product.

## Operation

`merchant.scenario.readiness` (`GET /api/agent/scenario-readiness`) returns the
figures of the panel for the selected private sandbox. It is read-only, needs the
same rights as the customer simulator and refuses live shops. MCP exposes the
same operation. `merchant.scenario.stock` (`POST /api/agent/scenario-stock` with
`{"quantity": 10, "approve": true}`) sets the test stock.

## Simulate agent proposals before you approve them

Every agent proposal is replayed in a twin before it reaches your review. The
result is saved with the proposal and shown next to it in Studio, in three
columns:

| Column | What it shows |
| --- | --- |
| **Changes** | Each changed field of each product, before and after, as the replay actually wrote it. |
| **Projected impact** | Units, gross revenue, margin and stock at the end of the next 30 days, without and with the change, plus the difference per currency. |
| **Rules** | Every rule the twin checked, with its outcome. A **blocking** violation means the proposal cannot be approved. |

Below the columns, **Why the agent acted** lists each step of the agent run: the
data it read, the proposal and the simulation, each with its sources.

### How the twin replays a proposal

The twin for a proposal is the shop it targets, at this moment, inside a database
transaction that is always rolled back. The replay uses the same write as the
later approval (same product rows, same revision check), reads the rows back for
the diff and then discards everything, including history entries and events.
Nothing reaches your customers, your stock or your search index. A replay waits
at most 250 ms for a product that live checkout is updating; if it cannot, the
simulation says **Simulation failed** and you ask the agent again.

A saved private sandbox is not used for this: it is a copy that is not kept in
sync, so revisions and prices there can differ from the shop the proposal will
change. Proposals made inside a private sandbox are replayed in that sandbox.

### What the projection assumes

The projection is arithmetic on recorded orders, not a forecast. Studio lists the
assumptions with every result:

- Demand equals the units of placed orders of the last 30 days that were not
  cancelled, expired or in payment review.
- Price elasticity is not modelled: the same units sell at the new price.
- Sales cannot exceed the stock, and there is no restocking.
- Only the base gross price counts. Promotions, customer-group and quantity
  prices, shipping, fees and returns are not included.
- Margin needs the unit cost from the product's AI price corridor. Without it,
  margin is shown as **Not projected**.

A product without sales in the window shows **No sales recorded** and no revenue
figures. If you may not read order figures (`orders.read` or `overview.read`),
the twin does not read them either and says so.

### Which rules are checked

| Rule | Checks | Effect |
| --- | --- | --- |
| AI price guardrails | Corridor minimum and maximum, minimum margin, maximum discount, price lock, availability, corridor currency | Blocking |
| Current product version | The product has not changed since the agent planned | Blocking |
| Price indication | A list price that no longer shows a reduction; a reduction shown without the lowest price of the last 30 days | Warning |

Rule Builder conditions, promotions, customer-group prices, shipping and payment
rules and destination taxes are **not** evaluated for a price or stock change.
The result names them under **Not checked**, so silence is never read as a pass.
The warnings point at the price indication fields of the product; they are not a
legal assessment.

### Proposals the twin cannot simulate

| Proposal part | Twin result |
| --- | --- |
| Product prices and stock | Simulated |
| Storefront presentation | Not simulated: there is no conversion model for presentation |
| App records (for example a page design) | Not simulated: app records belong to the app |
| Native work steps | Not simulated: reviewed steps are not replayed |
| Translation drafts | Nothing to simulate: drafts are reviewed separately before publication |

For these parts Studio shows the reason and no projection.

### Approving a simulated proposal

Approval re-checks everything on the current shop in one transaction. It refuses
a proposal whose simulation failed or found a blocking violation. It follows the
same clock rule as the product editor
([editing while orders arrive](product-management.md#editing-while-orders-arrive)):
a price-only proposal is checked against the product's content revision, so a
sale between review and approval does not make it stale and the current stock is
kept. A stock change, or a proposal whose product content changed, is refused with
the clock that changed (`/revision` or `/contentRevision`) and its current value.
Ask the agent again to get a fresh, simulated proposal.

The simulation is stored in the proposal evidence of the existing `tasks` row
(`simulation`, version 1). It needs no schema change and no separate twin
database. `merchant.run.status` returns it; `merchant.run.explain` returns it with
the explanation.
