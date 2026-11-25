# Proof coverage of the commerce domain functions

Generated from `proof/domain-inventory.json` by `scripts/formal/domain_inventory.py --write`;
`scripts/formal.py --check` fails when a function in `domain/` of `core`, `pricing`, `inventory`, `checkout`, `payments`, `orders` is in neither state below.

**binds-proved-rule** means the function decides through extracted, Lean-proved kernel rules; the input mapping around the call is reviewed Rust, not proved. **unproved** functions carry one of the reasons listed at the end. Nothing here proves SQL, HTTP or providers.

`components/core` has no `domain/` module; its functions are infrastructure (store, cache, HTTP limits) and are not counted here. A `domain/` module added to it is checked like the others.

Open after the money refactor (another session): order total = lines + shipping + tax, money scaling and rounding, and extractor multiplication. The functions they concern are listed under `money-representation` and `float-calculation`.

| Component | Functions | Binds a proved rule | Unproved |
| --- | ---: | ---: | ---: |
| core | 0 | 0 | 0 |
| pricing | 44 | 4 | 40 |
| inventory | 6 | 1 | 5 |
| checkout | 24 | 3 | 21 |
| payments | 37 | 3 | 34 |
| orders | 33 | 4 | 29 |
| **all** | **144** | **15** | **129** |

## Functions that bind proved rules

| Function | Rules |
| --- | --- |
| `components/checkout/src/domain/admin_order.rs::admit_payment` | `admin_payment_admissible` |
| `components/checkout/src/domain/cart_quote.rs::quote` | `amount_sum`, `net_amount` |
| `components/checkout/src/domain/review_binding.rs::validate` | `checkout_review_admissible`, `revision_admissible` |
| `components/inventory/src/domain/lines.rs::InventoryLines::new` | `reservation_admissible` |
| `components/orders/src/domain/order_transitions.rs::payment` | `manual_payment_admissible` |
| `components/orders/src/domain/order_workflow.rs::admitted_edge` | `order_transition_admissible` |
| `components/orders/src/domain/order_workflow.rs::guard` | `cancellation_admissible`, `completion_admissible`, `order_edit_admissible` |
| `components/orders/src/domain/order_workflow.rs::workflow` | `delivery_split_admissible` |
| `components/payments/src/domain/receipt_guard.rs::receipt_matches` | `receipt_admissible` |
| `components/payments/src/domain/refunds.rs::checked_total` | `refund_admissible` |
| `components/payments/src/domain/state.rs::transition` | `payment_transition_admissible` |
| `components/pricing/src/domain/discount.rs::allocate` | `discount_cap` |
| `components/pricing/src/domain/external_order.rs::PaidOrder::validate` | `external_paid_total_admissible` |
| `components/pricing/src/domain/external_order.rs::SourceAmount::valid` | `external_amount_admissible` |
| `components/pricing/src/domain/money.rs::Currency::new` | `currency_scale_admissible` |

## Unproved functions by reason

### money-representation (13)

Money arithmetic, scaling, rounding or totals. Open until the money refactor moves pricing to whole minor units; another session proves price total = lines + shipping + tax, scaling and rounding afterwards.

- `components/payments/src/domain/amounts.rs`: `format_amount`, `parse_amount`
- `components/payments/src/domain/refunds.rs`: `used`
- `components/payments/src/domain/settlements.rs`: `captured`
- `components/pricing/src/domain/money.rs`: `Currency::factor`, `Money::checked_add`, `Money::decimal`, `Money::from_legacy_eur`, `Money::new`, `Money::parse`, `Money::rounded`, `from_minor`, `to_minor`

### float-calculation (10)

Floating-point price, tax or discount calculation ported from the original calculators; outside the bool/u64 extraction subset until the money refactor.

- `components/checkout/src/domain/cart_quote.rs`: `configuration_price`
- `components/checkout/src/domain/tax_lines.rs`: `tax_products`
- `components/pricing/src/domain/pricing.rs`: `calculate`, `cash_round`, `cast`, `math_round`, `proportional_tax_rules`, `sum`
- `components/pricing/src/domain/quote.rs`: `LinePriceUpdate::new`, `QuoteTotals::new`

### document-shape (22)

Reads, writes or projects JSON/serde documents; data mapping, not a decision rule.

- `components/checkout/src/domain/admin_order.rs`: `order`, `purchase`
- `components/checkout/src/domain/quote_view.rs`: `QuoteView::first_line`, `QuoteView::method`, `digital`, `flags`, `quote_view`
- `components/orders/src/domain/company_model.rs`: `public`, `resolve`
- `components/orders/src/domain/import_product.rs`: `projection`
- `components/orders/src/domain/import_relations.rs`: `identity`, `product`
- `components/orders/src/domain/order_fields.rs`: `order_fields`
- `components/orders/src/domain/order_workflow.rs`: `WorkflowOrder::deliveries`
- `components/payments/src/domain/attempt.rs`: `Attempt::ctx`, `context_of`
- `components/payments/src/domain/instructions.rs`: `snapshot`
- `components/payments/src/domain/receipt_ref.rs`: `reference`
- `components/payments/src/domain/settlements.rs`: `summary`
- `components/pricing/src/domain/catalog.rs`: `public_extra`
- `components/pricing/src/domain/quote.rs`: `CalculatedTax::from`, `QuoteTax::from`

### text-and-locale (6)

Text, labels, locale or formatting; no commerce invariant to prove.

- `components/orders/src/domain/company_model.rs`: `address`
- `components/orders/src/domain/import_product.rs`: `text`
- `components/orders/src/domain/order_machine.rs`: `labels`
- `components/payments/src/domain/diagnostics.rs`: `code`, `response`
- `components/pricing/src/domain/validation.rs`: `ValidationError::fmt`

### rendering (10)

Renders documents (receipt text or PDF); output layout, not a decision rule.

- `components/orders/src/domain/logo_image.rs`: `normalize`
- `components/orders/src/domain/receipt_instructions.rs`: `lines`
- `components/orders/src/domain/receipt_pdf.rs`: `render`, `winansi`
- `components/orders/src/domain/receipt_text.rs`: `lines`, `pos_message`, `positions`, `seller_details`, `text`, `totals`

### validation-of-strings (18)

Validates identifiers, URLs, names or other strings and collections; outside the bool/u64 subset, covered by unit and HTTP tests.

- `components/checkout/src/domain/session_review.rs`: `key`
- `components/inventory/src/domain/supply.rs`: `Config::validate`
- `components/orders/src/domain/company_model.rs`: `validate`
- `components/payments/src/domain/amounts.rs`: `wire_currency`
- `components/payments/src/domain/details_model.rs`: `Amount::validate`, `Details::validate`, `date`, `status`
- `components/payments/src/domain/instructions.rs`: `Instructions::validate`, `validated`
- `components/payments/src/domain/return_urls.rs`: `build`, `urls`
- `components/payments/src/domain/service_config.rs`: `approval_transport`, `approval_url`, `validate_service`
- `components/pricing/src/domain/customer/address.rs`: `Address::validate`
- `components/pricing/src/domain/customer/contact.rs`: `Contact::validate`, `valid_birthday`

### decision-outside-subset (29)

A pure decision over strings, enums or collections that the bool/u64 extractor cannot express; reviewed and unit tested, not extracted.

- `components/checkout/src/domain/cart_quote.rs`: `normalized_quantity`
- `components/checkout/src/domain/session_payment.rs`: `attempt`, `has_saved_funding`, `provider`, `test_environment`
- `components/checkout/src/domain/session_review.rs`: `Review::admit_cart`, `Review::check_prepared_payment`, `Review::check_quote`, `Review::check_replay`, `receipt`
- `components/inventory/src/domain/reorder.rs`: `admit`, `within_ceiling`
- `components/orders/src/domain/order_machine.rs`: `OrderMachine::validate`
- `components/orders/src/domain/order_transitions.rs`: `cancellation_blocker`, `delivery`
- `components/orders/src/domain/order_workflow.rs`: `WorkflowOrder::external`, `external`, `terminal`
- `components/payments/src/domain/attempt.rs`: `AttemptContext::can`
- `components/payments/src/domain/capture_intent.rs`: `intent`
- `components/payments/src/domain/instructions.rs`: `admit`
- `components/payments/src/domain/receipt_ref.rs`: `ProviderReceipt::confirmed`, `identity`, `is`
- `components/payments/src/domain/refunds.rs`: `capture`
- `components/payments/src/domain/settlements.rs`: `closed`, `multi`
- `components/pricing/src/domain/context.rs`: `language_chain`, `select_tier`

### constructor-or-accessor (16)

Builds or reads a value without a decision of its own.

- `components/checkout/src/domain/session_review.rs`: `Review::session`
- `components/inventory/src/domain/lines.rs`: `InventoryLines::ids`, `InventoryLines::quantities`
- `components/pricing/src/domain/cart.rs`: `default_channel`
- `components/pricing/src/domain/customer/mod.rs`: `country_default`
- `components/pricing/src/domain/money.rs`: `Currency::code`, `Currency::eur`, `Currency::scale`, `Currency::try_from`, `Money::currency`, `Money::minor`
- `components/pricing/src/domain/pricing.rs`: `PriceInput::default`, `decimals`, `interval`, `yes`
- `components/pricing/src/domain/validation.rs`: `ValidationError::new`

### catalogue-data (3)

Returns fixed catalogue data (states, providers, schema); reviewed data, no rule.

- `components/orders/src/domain/creation_schema.rs`: `schema`
- `components/orders/src/domain/order_machine.rs`: `default_machine`
- `components/payments/src/domain/state.rs`: `code`

### signed-arithmetic (2)

Signed (i64) quantity or amount arithmetic outside the u64 subset; unit tested.

- `components/payments/src/domain/settlements.rs`: `state`
- `components/pricing/src/domain/context.rs`: `fix_quantity`

## Notes on single functions

- `components/checkout/src/domain/cart_quote.rs::quote`: Line totals still come from the floating-point calculator; only minor-unit addition and net/gross are proved here (money refactor pending).
- `components/inventory/src/domain/lines.rs::InventoryLines::new`: The new order holds nothing yet (held = 0); the reservation run theorems cover any later reserve/release sequence gated by the same rule.
- `components/orders/src/domain/order_machine.rs::OrderMachine::validate`: Graph validation of a configured machine (identifiers, labels, no edge out of a terminal state, reachability). The order run theorems do not rely on it: admitted_edge rechecks the listed target and the terminal source.
- `components/orders/src/domain/order_transitions.rs::delivery`: Delivery state strings; terminal orders are refused before it runs by guard (order_edit_admissible).
- `components/orders/src/domain/order_workflow.rs::terminal`: String membership; its result is the terminal input of the proved order_edit_admissible and order_transition_admissible.
- `components/pricing/src/domain/discount.rs::allocate`: The cap is proved; the proportional integer allocation loop and its rounding are unit tested, not proved.
