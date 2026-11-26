# Orders component

Order state, fulfilment, receipts, documents, company master data and merchant
order work (`veyndo-orders`). Order transitions, splits, carrier documents and
receipts change the order document with typed patches
(`domain::order_patches` and per-file `#[derive(Serialize)]` structs merged through
`veyndo_core::document::merge`). `store::placement` holds the order rows the
checkout purchase transaction places and replays on its connection. Orders owns
`company_logos` and `company_overrides` with `receipt_settings`: it is the only
component with SQL on them.

## Remaining foreign tables

| Table | Why it stays |
| --- | --- |
| `carts` | The order mirror into `carts.data.order` is written in the same order mutation statements, and order list, detail, account and download reads join the cart in the statement that filters and pages them (buyer email, channel, token). Checkout builds on orders, so a direct call would be a cycle; a port would have to reach every merchant handler. |
| `order_downloads`, `product_assets` | The download list and file reads join the grant and asset with the order in one statement. |
| `sales_channels` | Two existence checks of company data (`company::records`, `company_logos::admit_public`); orders does not depend on marketing and these handlers have no port. |

## Open JSON

| Place | Reason |
| --- | --- |
| The order document (`orders.data`) | The order contract carries the frozen quote, legal snapshot, deliveries and app fields; transitions patch only the fields they own. |
| Deliveries (`deliveries[i]`), split parcels | Shipping-owned allocation documents (`veyndo_shipping::domain::split`). |
| Company master data (`company_model::resolve`, `public`) | Field lists are data-driven (`LOCALIZED`, the public key list); localized fields are maps per locale. |
| Receipt snapshots (`order_receipts.snapshot`) | Immutable fiscal documents; `paymentInstructions` and `delivery` are copied in as received. |
| Activity `data` and the workflow projection | Audit entries and lifecycle projections whose shape belongs to each change kind. |
