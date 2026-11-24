//! Typed read views of the stored order document and of an issued receipt snapshot; open subtrees (prices, line items) stay JSON where they are copied verbatim.
use serde::Deserialize;
use serde_json::{Map, Value};
use veyndo_core::document::{lenient, lenient_list};

/// The stored order document as the read projections and documents read it.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct OrderDocument {
    pub id: Value,
    pub order_number: Value,
    #[serde(deserialize_with = "lenient")]
    pub cart: StoredQuote,
    #[serde(deserialize_with = "lenient")]
    pub money: Money,
    #[serde(deserialize_with = "lenient")]
    pub payment: Payment,
    pub billing_address: Value,
    pub shipping_address: Value,
    pub billing_address_id: Value,
    pub shipping_address_id: Value,
    #[serde(deserialize_with = "lenient")]
    pub pos: PosNote,
}

/// The cart quote frozen into the order.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct StoredQuote {
    /// The quote price, copied verbatim into the read fields.
    pub price: Value,
    /// The shipping costs, copied verbatim into the read fields.
    pub shipping_costs: Value,
    /// The line items, copied verbatim into the read fields.
    pub line_items: Value,
    #[serde(deserialize_with = "lenient")]
    pub currency_context: CurrencyContext,
    #[serde(deserialize_with = "lenient")]
    pub checkout: Checkout,
}

/// The typed amounts of a quote price.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct QuotePrice {
    pub total_price: Value,
    pub net_price: Value,
    pub tax: Value,
    pub tax_status: Value,
    pub currency: Value,
    pub currency_scale: Value,
}

/// A priced subtree that only needs its total.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Totalled {
    pub total_price: Value,
}

/// The currency conversion the quote used.
#[derive(Deserialize, Default)]
#[serde(default)]
pub struct CurrencyContext {
    pub factor: Value,
}

/// The checkout selection kept with the quote.
#[derive(Deserialize, Default)]
#[serde(default)]
pub struct Checkout {
    pub address: Value,
}

/// The exact minor amount and currency of the order.
#[derive(Deserialize, Default)]
#[serde(default)]
pub struct Money {
    pub minor: Value,
    #[serde(deserialize_with = "lenient")]
    pub currency: MoneyCurrency,
}

/// Currency code and scale of the exact amount.
#[derive(Deserialize, Default)]
#[serde(default)]
pub struct MoneyCurrency {
    pub code: Value,
    pub scale: Value,
}

/// The payment state the ledger keeps on the order.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Payment {
    pub attempt_id: Value,
    #[serde(deserialize_with = "lenient")]
    pub method: PaymentMethod,
    pub state: Value,
    pub provider: Value,
    pub real_money_charged: Value,
}

/// The selected payment method.
#[derive(Deserialize, Default)]
#[serde(default)]
pub struct PaymentMethod {
    pub id: Value,
}

/// The POS annotation of a register sale.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct PosNote {
    pub receipt_message: Option<Map<String, Value>>,
    pub main_locale: Value,
}

/// One line of the frozen quote as documents print it.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Line {
    pub id: Value,
    pub referenced_id: Value,
    pub label: Value,
    pub quantity: Value,
    #[serde(deserialize_with = "lenient")]
    pub price: LinePrice,
}

/// The price of one line with its taxes.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct LinePrice {
    pub total_price: Value,
    #[serde(deserialize_with = "lenient_list")]
    pub calculated_taxes: Vec<LineTax>,
}

/// One calculated tax of a line.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct LineTax {
    pub tax_rate: Value,
    pub tax: Value,
}

/// A postal address as documents print it, in print order.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Address {
    pub name: Value,
    pub company: Value,
    pub department: Value,
    pub street: Value,
    pub additional_address_line1: Value,
    pub additional_address_line2: Value,
    pub postal_code: Value,
    pub city: Value,
    pub country: Value,
    pub vat_id: Value,
}
impl Address {
    /// The printed fields in their document order.
    pub fn fields(&self) -> [&Value; 10] {
        [
            &self.name,
            &self.company,
            &self.department,
            &self.street,
            &self.additional_address_line1,
            &self.additional_address_line2,
            &self.postal_code,
            &self.city,
            &self.country,
            &self.vat_id,
        ]
    }
}

/// An issued receipt snapshot: its kind, number, seller, language, delivery scope and the frozen order.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct ReceiptSnapshot {
    pub kind: Value,
    pub number: Value,
    pub issued_at: Value,
    pub locale: Value,
    pub reference_number: Value,
    #[serde(deserialize_with = "lenient")]
    pub seller: Seller,
    #[serde(deserialize_with = "lenient")]
    pub order: OrderDocument,
    #[serde(deserialize_with = "lenient")]
    pub delivery: Delivery,
    pub payment_instructions: Value,
}

/// The seller details printed on a document.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Seller {
    pub name: Value,
    pub address: Value,
    pub tax_id: Value,
    pub vat_id: Value,
    pub registration_number: Value,
    pub register_court: Value,
    pub managing_directors: Value,
    pub legal_representatives: Value,
}

/// The positions a delivery note carries.
#[derive(Deserialize, Default)]
#[serde(default)]
pub struct Delivery {
    #[serde(deserialize_with = "lenient_list")]
    pub positions: Vec<DeliveryPosition>,
}

/// One delivered position.
#[derive(Deserialize, Default)]
#[serde(default)]
pub struct DeliveryPosition {
    pub id: Value,
    pub quantity: Value,
}

/// The line items of a quote, each read leniently.
#[derive(Deserialize, Default)]
#[serde(transparent)]
pub struct Lines(#[serde(deserialize_with = "lenient_list")] pub Vec<Line>);
