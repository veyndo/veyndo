# Shared merchant payment controls

`OrderPayment` is the payment ledger panel of an order. The payments component
contributes it to the order page through the Studio slot `admin.order.payment`
(`studio-extension.ts`), so the orders UI never imports payment code. It owns the
provider commands (capture, refund, void) with exact-once request keys, observes their
durable job, shows progress and errors, and asks the order page to reload when a
command settled.

`PaymentActions` displays the server settlement summary in orders and payment
management. It collects exact capture/refund amounts, an owned capture selection
and final-capture choice. It never calculates a new invoice, marks an order paid
or dispatches directly to a provider. `OrderPayment` preserves request identity and
observes the existing durable job.

`payment-amount` parses decimal input into exact safe integer minor units and
owns presentation DTOs only. Current permissions, provider capabilities, account
ownership, immutable receipts and pending reservations remain server decisions.
EN/DE/FR/ES text comes from the shared payment dictionaries; styles inherit the
native Studio theme and collapse into one column at narrow widths.

`PaymentDetails` lazily reads the shared native payment details endpoint. Provider
observations and durable payment-only history remain separate; no command is
issued by loading or refreshing this section. It is available with `payments.read`,
including members who cannot manage payments or read customer PII. Historical
orders without a capture summary retain their exact native refund form.
