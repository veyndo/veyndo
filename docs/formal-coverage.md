# Proof coverage of the commerce domain functions

Generated from `proof/domain-inventory.json` by `scripts/formal/domain_inventory.py --write`;
`scripts/formal.py --check` fails when a function in `domain/` of `core`, `pricing`, `inventory`, `checkout`, `payments`, `orders` is in neither state below.

**binds-proved-rule** means the function decides through extracted, Lean-proved kernel rules; the input mapping around the call is reviewed Rust, not proved. **unproved** functions carry one of the reasons listed at the end. Nothing here proves SQL, HTTP or providers.

`components/core` has no `domain/` module; its functions are infrastructure (store, cache, HTTP limits) and are not counted here. A `domain/` module added to it is checked like the others.

Money runs on whole minor units (`Minor`, `Money`) and exact decimals and rationals (`Decimal`, `Ratio`); since 26 November the calculator no longer reproduces Shopware's binary64 steps: it computes exactly and rounds each amount once, half away from zero, per currency exponent. JSON numbers cross one boundary conversion (`Decimal::from_wire`/`to_wire`). Proved and bound: line totals (`line_total`), rounding of a quotient to the minor unit (`rounded_quotient`, also the calculator's cash-interval rounding in `Rounding::price`), sums (`amount_sum`), net = gross - tax (`net_amount`), discount within its base (`discount_cap`), and the quote-total theorems over any list of lines.

| Component | Functions | Binds a proved rule | Unproved |
| --- | ---: | ---: | ---: |
| core | 0 | 0 | 0 |
| pricing | 106 | 9 | 97 |
| inventory | 6 | 1 | 5 |
| checkout | 23 | 3 | 20 |
| payments | 37 | 3 | 34 |
| orders | 35 | 4 | 31 |
| **all** | **207** | **20** | **187** |

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
| `components/pricing/src/domain/decimal.rs::quotient` | `rounded_quotient` |
| `components/pricing/src/domain/discount.rs::allocate` | `discount_cap` |
| `components/pricing/src/domain/external_order.rs::PaidOrder::validate` | `external_paid_total_admissible` |
| `components/pricing/src/domain/external_order.rs::SourceAmount::valid` | `external_amount_admissible` |
| `components/pricing/src/domain/money.rs::Currency::new` | `currency_scale_admissible` |
| `components/pricing/src/domain/money.rs::Minor::times` | `line_total` |
| `components/pricing/src/domain/money.rs::unsigned_sum` | `amount_sum` |
| `components/pricing/src/domain/pricing.rs::Rounding::price` | `line_total`, `rounded_quotient` |
| `components/pricing/src/domain/quote.rs::QuoteTotals::new` | `net_amount` |

## Unproved functions by reason

### money-representation (59)

Money representation and exact arithmetic outside the bool/u64 subset: minor-unit types, decimal parsing and text, exact rationals in i128, the exact calculator's rule mix, the wire conversion of JSON numbers. Rounding, products and sums inside it are the proved rules; the rest is unit and property tested.

- `components/checkout/src/domain/cart_quote.rs`: `configuration_price`
- `components/checkout/src/domain/tax_lines.rs`: `tax_products`
- `components/payments/src/domain/amounts.rs`: `format_amount`, `parse_amount`
- `components/payments/src/domain/refunds.rs`: `used`
- `components/payments/src/domain/settlements.rs`: `captured`
- `components/pricing/src/domain/decimal.rs`: `Decimal::checked_add`, `Decimal::cmp`, `Decimal::deserialize`, `Decimal::eq`, `Decimal::fmt`, `Decimal::from_wire`, `Decimal::normalized`, `Decimal::parse`, `Decimal::partial_cmp`, `Decimal::serialize`, `Decimal::text`, `Decimal::to_wire`, `Ratio::abs`, `Ratio::checked_add`, `Ratio::checked_div`, `Ratio::checked_mul`, `Ratio::checked_sub`, `Ratio::cmp`, `Ratio::deserialize`, `Ratio::negated`, `Ratio::new`, `Ratio::partial_cmp`, `Ratio::reduced`, `Ratio::rounded`, `Ratio::significant`, `Ratio::significant_away`, `Ratio::significant_with`, `gcd`, `power`, `wide`
- `components/pricing/src/domain/money.rs`: `Currency::factor`, `Minor::decimal`, `Minor::from_decimal`, `Minor::from_unsigned`, `Minor::ratio`, `Minor::serialize`, `Minor::significant`, `Minor::unsigned`, `Money::checked_add`, `Money::decimal`, `Money::from_decimal`, `Money::new`, `Money::parse`, `Money::rounded`
- `components/pricing/src/domain/pricing.rs`: `Rounding::new`, `Rounding::plain`, `calculate`, `merged_rules`, `percent`, `proportional_tax_rules`
- `components/pricing/src/domain/quote.rs`: `LinePriceUpdate::new`, `QuoteTax::at_scale`, `QuoteTotals::of_lines`

### document-shape (22)

Reads, writes or projects JSON/serde documents; data mapping, not a decision rule.

- `components/checkout/src/domain/admin_order.rs`: `order`
- `components/checkout/src/domain/quote_view.rs`: `QuoteView::first_line`, `QuoteView::method`, `digital`, `flags`, `quote_view`
- `components/orders/src/domain/company_model.rs`: `public`, `resolve`
- `components/orders/src/domain/import_product.rs`: `projection`
- `components/orders/src/domain/import_relations.rs`: `identity`, `product`
- `components/orders/src/domain/order_fields.rs`: `order_fields`
- `components/orders/src/domain/order_patches.rs`: `TrackingUrl::of`, `strip_cart_token`
- `components/orders/src/domain/order_workflow.rs`: `WorkflowOrder::deliveries`
- `components/payments/src/domain/attempt.rs`: `Attempt::ctx`, `context_of`
- `components/payments/src/domain/instructions.rs`: `snapshot`
- `components/payments/src/domain/receipt_ref.rs`: `reference`
- `components/payments/src/domain/settlements.rs`: `summary`
- `components/pricing/src/domain/catalog.rs`: `public_extra`
- `components/pricing/src/domain/quote.rs`: `QuoteTax::from`

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

### constructor-or-accessor (34)

Builds or reads a value without a decision of its own.

- `components/checkout/src/domain/session_review.rs`: `Review::session`
- `components/inventory/src/domain/lines.rs`: `InventoryLines::ids`, `InventoryLines::quantities`
- `components/pricing/src/domain/cart.rs`: `default_channel`
- `components/pricing/src/domain/customer/mod.rs`: `country_default`
- `components/pricing/src/domain/decimal.rs`: `Decimal::integer`, `Decimal::is_zero`, `Decimal::new`, `Decimal::ratio`, `Decimal::scale`, `Decimal::units`, `Ratio::denominator`, `Ratio::integer`, `Ratio::is_negative`, `Ratio::is_zero`, `Ratio::numerator`
- `components/pricing/src/domain/money.rs`: `Currency::code`, `Currency::eur`, `Currency::scale`, `Currency::try_from`, `Minor::minor`, `Minor::new`, `Minor::same_scale`, `Minor::scale`, `Minor::zero`, `Money::amount`, `Money::currency`, `Money::minor`
- `components/pricing/src/domain/pricing.rs`: `PriceInput::default`, `decimals`, `hundred`, `interval`, `yes`
- `components/pricing/src/domain/validation.rs`: `ValidationError::new`

### catalogue-data (3)

Returns fixed catalogue data (states, providers, schema); reviewed data, no rule.

- `components/orders/src/domain/creation_schema.rs`: `schema`
- `components/orders/src/domain/order_machine.rs`: `default_machine`
- `components/payments/src/domain/state.rs`: `code`

### signed-arithmetic (6)

Signed (i64) quantity or amount arithmetic outside the u64 subset; unit tested.

- `components/payments/src/domain/settlements.rs`: `state`
- `components/pricing/src/domain/context.rs`: `fix_quantity`
- `components/pricing/src/domain/money.rs`: `Minor::at_scale`, `Minor::checked_add`, `Minor::checked_sub`, `Minor::sum`

## Notes on single functions

- `components/checkout/src/domain/cart_quote.rs::quote`: Line totals are the calculator's proved line_total in whole minor units; the quote adds them with amount_sum and splits net and tax with net_amount. The Lean quote-total theorems state these sums for any list of lines.
- `components/checkout/src/domain/tax_lines.rs::tax_products`: The net-preserving rate factor (100 + rate) / (100 + product rate) is exact; a converted snapshot price is kept at fifteen significant digits so its binary64 number reads back as that decimal.
- `components/inventory/src/domain/lines.rs::InventoryLines::new`: The new order holds nothing yet (held = 0); the reservation run theorems cover any later reserve/release sequence gated by the same rule.
- `components/orders/src/domain/order_machine.rs::OrderMachine::validate`: Graph validation of a configured machine (identifiers, labels, no edge out of a terminal state, reachability). The order run theorems do not rely on it: admitted_edge rechecks the listed target and the terminal source.
- `components/orders/src/domain/order_transitions.rs::delivery`: Delivery state strings; terminal orders are refused before it runs by guard (order_edit_admissible).
- `components/orders/src/domain/order_workflow.rs::terminal`: String membership; its result is the terminal input of the proved order_edit_admissible and order_transition_admissible.
- `components/pricing/src/domain/decimal.rs::Decimal::from_wire`: The one boundary conversion from a JSON number: the shortest decimal that reads back as the number (Rust f64 display digits); unit and property tested.
- `components/pricing/src/domain/decimal.rs::quotient`: Refuses operands outside the rule's range instead of letting it saturate; Ratio::rounded rounds every exact amount into minor units through it.
- `components/pricing/src/domain/discount.rs::allocate`: The cap is proved; the proportional integer allocation loop and its rounding are unit tested, not proved.
- `components/pricing/src/domain/money.rs::Minor::times`: Sign and overflow refusal around the proved product; the calculator's line totals and every quantity multiplication of a minor amount use it.
- `components/pricing/src/domain/money.rs::unsigned_sum`: Refuses an overflowing sum instead of letting the proved sum saturate.
- `components/pricing/src/domain/pricing.rs::Rounding::plain`: One rounding to the precision through Ratio::rounded, i.e. the proved rounded_quotient (bound in decimal.rs::quotient).
- `components/pricing/src/domain/pricing.rs::Rounding::price`: Cash rounding in whole minor units: rounded_quotient of the amount by the interval, times the interval by line_total (Lean cash_round_nearest_multiple); operands outside the rules' range are refused. The plain rounding before it is Rounding::plain.
- `components/pricing/src/domain/pricing.rs::calculate`: Exact Ratio arithmetic over decimals and rationals; every amount is rounded once, half away from zero, through Rounding::plain (proved rounded_quotient) and Rounding::price (proved rounded_quotient and line_total), the line total is the proved line_total (Minor::times). The rule mix and gross/net mapping are property tested (tests/properties.rs, tests/decimal_ties.rs), not extracted.
- `components/pricing/src/domain/quote.rs::LinePriceUpdate::new`: The unit price is total / quantity rounded once through Ratio::rounded (proved rounded_quotient).
- `components/pricing/src/domain/quote.rs::QuoteTotals::new`: Net quotes add the tax with the proved sum (through unsigned_sum), gross quotes take the proved net; negative amounts are refused.
