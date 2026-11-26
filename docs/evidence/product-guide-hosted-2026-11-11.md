# Product Guide in the hosted shop — 11 November 2026

The public native storefront opened the Product Guide with the actual model
and a synthetic lamp in stock. The host calculated €59.90, the delivery
estimate and collection at €0.00. The cart stayed empty during the review.
Selecting **Add to cart** added exactly one unit. The same action on a sold-out
product offered no cart confirmation. The lamp has no distinct variants.

Studio refused to prepare an executable product change in the live workspace.
In the private Product Guide acceptance sandbox, its public care records were
available while the private notes started empty. A new synthetic private note
then informed the actual model's proposed German care sentence. Studio showed
the App table and the stored proposal in the same answer. After explicit
approval, one action completed and the sentence appeared in the sandbox product
editor. The live product description did not change.

[Cart review](../assets/apps/product-guide-cart-review-20261111.jpg) ·
[Approved sandbox change](../assets/apps/product-guide-sandbox-applied-20261111.jpg) ·
[Reproduce both actions](../product-guide-mcp-app.md#try-it-in-both-conversations).

### Product Guide variant selection

On 11 November, the public native Lumen shop opened the Product Guide
0.1.2 for a synthetic Meridian lamp. With Brass out of stock, the picker
offered Graphite. With both available, the host quoted €64.90 for Brass.
Changing to Graphite cleared that offer and required another review.
Core then quoted €79.90 and collection at €0.00 in Germany. The cart
stayed empty through both reviews. Confirmation added exactly one
Graphite unit at €79.90, which also appeared in the checkout summary.

The model text reported that it had received no catalogue results for
its recommendation, although the App view supplied the product reference
and the host resolved it successfully. The review above records the actual
host and Core behavior. The test cart was cleared and the synthetic product
family was deactivated afterward. The run stopped at the checkout summary.

[Review with the chosen variant](../assets/apps/product-guide-variant-review-20261111.jpg) ·
[Selected variant in checkout](../assets/apps/product-guide-variant-cart-20261111.jpg) ·
[Reproduce the test](../product-guide-mcp-app.md#try-it-in-both-conversations).

## Product guide in both live conversations

On 11 November, the installed Product Guide 0.1.2 retained all four synthetic
records after its upgrade. The real `gpt-6.1-sol` merchant agent opened the
private merchandising table for the Lumen lamp. Changing its priority selector
queried the same app action, reduced two notes to the priority note and copied
only the verified filter into an editable follow-up. No product was edited.

The public native storefront opened the separate care card. Its care selector
returned the special-care record and carried that selection into the customer
draft. Asking that public agent for internal merchandising notes returned no
private records or private table. These are separate audiences of one installed
package, using the shared Apps model and the existing MCP tool admission.
[Reproduce the guide and use it through JavaScript](../product-guide-mcp-app.md).

Core `cca098c5`, build `graceful-rail-9217`, passed the post-deployment comparison
against all eight exact hosted artifacts and their source/schema inputs. The
automatic Northflank workflow had already promoted it before candidate
preparation. This is post-deployment verification; automatic composition
admission before promotion remains open under FND-08.

Core `955bdc4d`, build `pleasant-walk-724`, repaired an App-frame colour
mismatch and the final answer's missing presentation context. Its candidate
passed against all eight existing artifacts before promotion. After deployment,
all eight exact artifact identities and the unchanged Core schema inputs matched
that candidate. The shared renderer pairs text and canvas colours and negotiates
the current host theme. The final model receives the same server-selected view
references as the host, separately from untrusted records.

The live browser retest rendered readable content in both views. The real
merchant agent opened two notes and the priority filter returned only the
priority record. The public answer acknowledged the attached care card, which
filtered to special care. Both views carried the verified selection into an
editable draft. The model identified the records as synthetic and did not
claim that it had operated the browser. No product or cart was changed.

![The private priority record in the merchant conversation](../assets/apps/product-guide-merchant-live-20261111.jpg)

![The public care card and selected care type in the shopper conversation](../assets/apps/product-guide-shopper-live-20261111.jpg)

The Lumen email app has received the tested order events, but sending remains
disabled in test mode and its sender and transport are unconfigured. That is the
remaining external mail-delivery prerequisite, not a successful mailbox receipt.

This record belongs to the earlier release that ran in the hosted pilot; see [status](../status.md) and [the Product Guide guide](../product-guide-mcp-app.md).
