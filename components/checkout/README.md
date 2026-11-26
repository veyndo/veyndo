# Checkout component

Cart, quote, checkout transaction and order placement (`veyndo-checkout`). The
purchase transaction locks the idempotency key, cart, company unit, products and
settings, prices the cart, and commits the order, stock, cart and payment attempt
in one transaction. Statements on other components' tables run on that same
connection through the owner's store (`veyndo_orders::store::placement`,
`veyndo_catalog::store::purchase_products`, `veyndo_customers::store::purchase_customers`,
`veyndo_shop::store::purchase_consent`, `veyndo_marketing::store::channel_admission`) or
through the `PurchaseRecords` port (`src/ports/checkout.rs`: the business policy digest
and the experiment policy outcome).

## Remaining foreign tables

`customer_sessions`, `customers`, `tenants` and `shop_environments` are read by
one statement, `store::buyer_sessions::live_buyer`: the buyer of a checkout grant
is an active customer with a current session of an active live shop. The four
owners are checked atomically in one snapshot; splitting it would change what
one admission read sees.

## Open JSON

Order and quote documents are built from typed patches (`veyndo_core::document::merge`
with `#[derive(Serialize)]` structs, nested through `document_slot::field`). These
values stay `serde_json::Value` because their content is open by contract:

| Place | Reason |
| --- | --- |
| The quote (`cart_quote::quote`, `delivery::enrich`) and the order's `cart` | The priced quote is the storefront contract; app hooks, promotions and the cashier adjust it through the server ports and pass fields through unchanged. |
| Quote line `configuration` / `appConfigurations` | Snapshots of app configurations; their fields belong to each app. |
| `legal` on the order (`legal_snapshot::snapshot`) | Frozen legal documents, acknowledgement and consent receipt exactly as stored by shop. |
| `currencyContext`, `deliveries`, `paymentMethod` on the order | Copied from the quote as it was priced. |
| Localized method views (`method_text::localized_*`) | Shop-owned method projections; checkout only adds price and country fields. |
| `checkoutService.transport` and the session receipt `paymentAttemptId` | Passed through from configuration and the order's payment as stored. |
