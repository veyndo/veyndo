# Payments component

Provider-independent payment ledger, captures, refunds and provider workers
(`veyndo-payments`). The ledger updates the order payment state, settlement
summary, events and stock release in one transaction. Records of other
components go through the owner's store on the ledger connection
(`veyndo_inventory::store::attempt_reservations`, `veyndo_shop::store::settings`,
`veyndo_shop::store::hosted_bindings`) or through ports implemented in
`src/ports/payments.rs` and `src/ports/checkout.rs`: `LedgerRecords` (cart order
snapshot, experiment reward), `MerchantRecords` (payment history, sales channels)
and `Buyers` (the customer of a funding session).

## Remaining foreign tables

| Table | Why it stays |
| --- | --- |
| `orders` | Attempt reads join the order to select the current attempt or the sale in one statement; the ledger reads and writes the order document under its row lock. |
| `carts` | `commands::customer` admits a buyer by cart token in the statement that joins attempt, order and cart. |
| `app_packages` | Provider discovery joins installed packages with provider accounts (`DISTINCT ON`) in one statement; installation reads manifests under the same lock. |

## Open JSON

| Place | Reason |
| --- | --- |
| `payment_attempts.provider_context` | The provider contract: capabilities, checkout, funding and receipts per provider; the ledger writes its own fields (`authorizationId`, `captureReceipts`, `providerRefunds`, …) through typed patches. |
| The settlement summary merged into `order.payment` | Computed by `domain::settlements::summary` from the attempt; copied field by field. |
| Adapter request bodies (`service::remote`) and provider evidence | Passed through to and from the pinned adapter service; validated by `receipt_ref` and `details_model` before use. |
| Job `request` input | The operation input a merchant or the worker queued; the worker adds only `ledgerJobKey`. |
| The selected method `raw` on a pending payment | The quote's payment method as the buyer selected it. |
