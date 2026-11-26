# Payments: captures, refunds, saved methods, invoices and history

This page is the merchant and integrator guide to payments after checkout. The
checkout side and the provider boundary are in [checkout](checkout.md); building a
provider is in [the payment provider API](payment-provider-api.md). The provider-
independent ledger lives in the `payments` component
([`components/payments`](../components/payments/)); its HTTP routes are in
[`src/payments/routes.rs`](../src/payments/routes.rs). The private Shopware
Payments connector (repository `veyndo/shopware-payments`) is one provider of this
ledger; its recorded journeys are in the
[provider acceptance matrix](shopware-payments-acceptance.md).

Amounts are exact: decimal input is converted to whole minor units, including
zero-decimal currencies, and excess precision is rejected
([multi-currency commerce](currencies.md)).

## Capture and refund

Veyndo keeps the original order amount fixed and records confirmed captures
separately. A EUR 100 authorization can be captured as EUR 40 followed by EUR 60.
Each capture has its own refundable balance. Every financial operation uses the
same native job, current permission and verified receipt path.

### Configure when payment is collected

Open **Settings → Payment methods** and choose an installed provider method. For
methods that support authorization, **When to collect payment** offers the
provider default, immediate capture, or authorization followed by capture from the
order. The setting belongs to the revisioned shop/channel settings and their
release controls. Changes apply to new checkouts; existing payments keep their
original method, account, environment, version and intent. Installing a provider
contract does not configure the provider service or enable methods.

### Work on a payment

Open the order's payment section. The same controls appear in payment management.

1. Check **Captured**, **Refunded** and **Still authorized**.
2. Enter an exact amount and choose **Capture amount**. Partial capture requires a
   method with `multi_capture`, confirmed authorization and an available balance.
3. Select **Close authorization after this capture** only when the remaining
   authorization should no longer be used. Captured funds remain refundable.
4. To refund a payment with several captures, select the capture and enter the
   amount. The system checks both that capture's balance and the overall balance.
5. Use **Void authorization** to close an unused remainder. Captured funds and sold
   inventory are kept; a whole-order cancellation is not implied.

Only members with `payments.manage` can issue these commands; read access uses
`payments.read`. HTTP (`POST /api/payments/{id}/{operation}`), MCP, app actions
and Flows enqueue the same durable jobs. The controls observe the original job
(`GET /api/payments/jobs/{id}`) instead of issuing repeated financial commands
while its outcome is pending.

### Recovery and state

`partially_captured` records funds below the invoice amount. It does not mark the
whole invoice paid, unlock paid downloads or complete the order. A refund of all
captured funds is terminal only when the capture is closed. An open authorization
can still collect its remaining amount after an earlier capture was refunded.

Uncertain or pending captures reserve their requested amount; another capture is
refused until the outcome is resolved. A verified provider observation can bind a
capture to its original native request and complete that job. Replaying a receipt
cannot add money again or change its immutable amount. Browser callbacks cannot
supply capture amounts or select captures: they request native reconciliation, and
merchant commands own collection and refund allocation.

The ledger uses the payment attempt, job and receipt-claim tables of the `payments`
component; there is no parallel financial ledger and no table per payment. The
capture budget and state-transition decisions are proved in Lean
([proof coverage](proof-coverage.md)); SQL, network calls and provider behaviour
are outside those proofs.

## Transaction details and history

Open **Transaction details & history** in an order's payment section or in payment
management. Members need `payments.read`. Authorizations, captures, refunds,
provider fees, platform fees and net received amounts appear next to the shop's
recorded payment transitions and operations. **Refresh details** reads them again
without issuing a financial command.

`GET /api/payments/{id}/details` and MCP `merchant.payment.details` use the same
native operation and the current `payments.read` permission. A provider observation
uses the payment's original account, order reference, environment and adapter
version; the optional `details` capability of a provider contract supplies it.
Fees are exact minor-unit amounts in their reported currency. An absent fee or net
amount stays absent; the interface never invents one.

The shop history comes from durable order history, projected to payment state and
recorded capture/refund totals. Reading neither reconciles the ledger nor creates
capture, refund or reconciliation jobs. Results contain the latest 100 native
transitions and 100 operations; they are not an accounting export.

An owned payment ID never grants access to another shop. Payment inspection neither
requires nor grants customer PII access. The provider service projects only a
whitelist of receipt fields; payer information, card and bank fields, provider
credentials and full order snapshots are excluded. The core validates the typed
projection, dates, receipt identities, currency precision, collection bounds and
fee categories.

## Saved payment methods

Signed-in customers can save a supported payment method for their next purchase.
Consent starts unchecked. After successful provider confirmation the method appears
in the customer account, and a later checkout can select it without copying payment
credentials into the storefront.

1. Sign in and select a payment method that supports saving.
2. Select **Save this payment method for future purchases** if desired.
3. Complete the provider's approval and the purchase. Saving can finish later than
   the payment itself.
4. On the next purchase, select the saved method. The provider can still require
   approval or authentication.
5. Revoke it in the customer account. Revocation blocks future use immediately and
   queues cleanup in the provider service. Completed orders, captures and refunds
   are unchanged. Revocation does not claim that the token was deleted at the PSP.

This consent covers customer-present reuse. It does not authorize subscription
charges or merchant-initiated recurring debits; those are not implemented.

| Entry | Authority and behaviour |
| --- | --- |
| `GET /store-api/account/payment-methods` | Current owning customer; redacted methods and installed capability discovery |
| `POST /store-api/account/payment-methods/{id}/revoke` | Same customer, opaque ID and stable `requestKey` |
| MCP `checkout.payment.methods` | Same native list and customer identity |
| MCP `checkout.payment.method.revoke` | Same native revoke, with `approve: true` |
| Checkout `paymentFunding` | Either `{save: true}` or `{save: false, id: "opaque-id"}`; validated again at purchase |
| Event `payment.instrument.revoked` | Outbox/Flow registry; only the opaque instrument ID |

`payment_instruments` is one table for all shops and providers. It holds opaque
handles, redacted display metadata, the original consent and the
customer/account/environment binding, under forced row security. The provider
service keeps the encrypted tokens in its own private store; they never reach API
or MCP results, frontend state, Flow payloads or app packages. A cart token, a
matching email, a merchant permission or another customer's session does not grant
reuse. At most 32 active handles are admitted per customer account. The Store API
payment status reports `savedPaymentMethodStatus` (pending, saved, failed, reused)
separately from the financial state, so a failed save does not look like a failed
purchase.

## Invoice payment and recipient details

Providers can offer a deferred invoice flow. The customer enters the details the
provider requires (for example date of birth and telephone number) inside the
provider's short-lived payment form; the provider order is created only after
submission. Recipient details (bank name, recipient, IBAN, BIC and payment
reference) can arrive while the capture is pending; they are shown without marking
the payment received. The native payment status stays authoritative.

| Entry | Rights |
| --- | --- |
| `GET /store-api/payments/{id}/instructions`, MCP `checkout.payment.instructions` | The original checkout capability |
| `GET /api/payments/{id}/instructions`, MCP `merchant.payment.instructions` | Current `payments.read`, `orders.read` and `customers.pii` |

The core validates a typed recipient projection, the bank checksum and reference
bounds, and caches validated instructions immutably in the payment context. The
first validated recipient receipt emits `payment.instructions.available` in the
same transaction; reconciliation and restarts cannot emit it twice.

Native invoice generation freezes these details in the immutable receipt snapshot
and prints a localized payment footer. Issuing an invoice before the provider has
supplied the details returns a conflict before an invoice number is allocated.
Delivery notes and cancellation receipts never request payment.

The Email Delivery app offers an **Invoice payment details** template and an opt-in
automatic notification; its `{paymentInstructions}` variable uses the native
amount, currency scale, recipient and reference. The Flow Builder can target
`Email Delivery → send_payment_instructions` on the same event. The app capability
`payments.instructions.read` requires current `payments.read`, `orders.read` and
`customers.pii`. Stored Flow jobs omit bank instructions; execution reloads them
after checking current rights. Choose either the automatic notification or a Flow
for the same message to avoid duplicate sends ([email delivery](email-delivery.md)).

![German invoice message templates in the Studio email component](assets/payment-invoice-20261103/email-templates-desktop.png)

A stable private handle exists before the provider order. A committed payload digest
prevents a lost response from being retried with changed input, and the same request
key recovers the original submission. Before submission, cancellation prevents any
provider create; after submission, unconfirmed outcomes keep their payment and stock
reservation until the provider confirms settlement or a terminal outcome without
funds.

## Tests

The `payments` and `payment_providers` suites run the core and provider fixtures
against isolated PostgreSQL: dropped capture/refund responses, multiple captures,
per-capture refunds, pending-capture recovery, voiding the remainder, historical
contract versions, malformed or duplicate provider receipts, transaction details,
saved-method save/reuse/revoke and invoice instructions. They use local provider
fixtures; external PSP accounts are covered only by the dated journeys in the
[provider acceptance matrix](shopware-payments-acceptance.md) and
[the saved-method evidence](evidence/payment-saved-methods-2026-11-03.md).
