//! Four-language document labels and authoritative snapshot-to-print projection over the typed receipt snapshot.
use crate::types::order_document::{
    Address, Line, Lines, QuotePrice, ReceiptSnapshot, Seller, Totalled,
};
use serde_json::Value;
use veyndo_core::document::view;
fn text(v: &Value) -> String {
    v.as_str().unwrap_or("").into()
}
/// The seller's legal details that are filled in.
fn seller_details(seller: &Seller, index: usize) -> Vec<String> {
    let word = |w: [&str; 4]| w[index].to_string();
    let mut lines = vec![];
    for (value, labels) in [
        (
            &seller.vat_id,
            ["VAT ID", "Umsatzsteuer-ID", "Numéro TVA", "Número IVA"],
        ),
        (
            &seller.registration_number,
            [
                "Register number",
                "Registernummer",
                "Numéro registre",
                "Número registro",
            ],
        ),
        (
            &seller.register_court,
            [
                "Register court",
                "Registergericht",
                "Tribunal registre",
                "Tribunal registro",
            ],
        ),
        (
            &seller.managing_directors,
            [
                "Managing directors",
                "Geschäftsführer",
                "Gérants",
                "Administradores",
            ],
        ),
        (
            &seller.legal_representatives,
            [
                "Representatives",
                "Vertretungsberechtigte",
                "Représentants",
                "Representantes",
            ],
        ),
    ] {
        let value = text(value);
        if !value.is_empty() {
            lines.push(format!("{}: {value}", word(labels)));
        }
    }
    lines
}
/// The positions of the document; a delivery note lists only what its delivery carries.
fn positions(
    s: &ReceiptSnapshot,
    (items, scoped): (&[Line], bool),
    (sign, decimals, currency): (f64, usize, &str),
) -> Vec<String> {
    let mut lines = vec![];
    for item in items {
        let quantity = if s.kind == "delivery_note" && scoped {
            let Some(position) = s
                .delivery
                .positions
                .iter()
                .find(|p| p.id == item.referenced_id || p.id == item.id)
            else {
                continue;
            };
            position.quantity.clone()
        } else {
            item.quantity.clone()
        };
        lines.push(format!(
            "{} x {} [{}]",
            quantity,
            text(&item.label),
            text(&item.referenced_id)
        ));
        if s.kind != "delivery_note" {
            lines.push(format!(
                "  {:.decimals$} {currency}",
                sign * item.price.total_price.as_f64().unwrap_or(0.)
            ));
        }
    }
    lines
}
/// Net, tax, total, shipping and the tax per rate.
fn totals(
    (price, shipping, items): (&QuotePrice, &Totalled, &[Line]),
    index: usize,
    (sign, decimals, currency): (f64, usize, &str),
) -> Vec<String> {
    let word = |w: [&str; 4]| w[index].to_string();
    let mut lines = vec![];
    for (value, label) in [
        (&price.net_price, word(["Net", "Netto", "Net", "Neto"])),
        (&price.tax, word(["Tax", "Steuer", "Taxe", "Impuesto"])),
        (
            &price.total_price,
            word(["Total", "Gesamt", "Total", "Total"]),
        ),
    ] {
        lines.push(format!(
            "{label}: {:.decimals$} {currency}",
            sign * value.as_f64().unwrap_or(0.)
        ));
    }
    lines.push(format!(
        "{}: {:.decimals$} {currency}",
        word(["Shipping", "Versand", "Livraison", "Envío"]),
        sign * shipping.total_price.as_f64().unwrap_or(0.)
    ));
    for item in items {
        for tax in &item.price.calculated_taxes {
            lines.push(format!(
                "{} {}%: {:.decimals$} {currency}",
                word(["VAT", "MwSt.", "TVA", "IVA"]),
                tax.tax_rate,
                sign * tax.tax.as_f64().unwrap_or(0.)
            ));
        }
    }
    lines
}
/// The POS receipt message in the receipt language, its base language or the shop's main language.
fn pos_message(s: &ReceiptSnapshot) -> Option<String> {
    let values = s.order.pos.receipt_message.as_ref()?;
    let main = s.order.pos.main_locale.as_str().unwrap_or("en-GB");
    values
        .get(s.locale.as_str().unwrap_or("en"))
        .or_else(|| {
            values
                .iter()
                .find(|(key, _)| key.split('-').next() == s.locale.as_str())
                .map(|(_, value)| value)
        })
        .or_else(|| values.get(main))
        .and_then(Value::as_str)
        .map(str::to_owned)
}
/// The printable lines of an issued receipt snapshot in its language.
pub fn lines(v: &Value) -> Vec<String> {
    let s: ReceiptSnapshot = view(v);
    let lang = s.locale.as_str().unwrap_or("en");
    let index = match lang {
        "de" => 1,
        "fr" => 2,
        "es" => 3,
        _ => 0,
    };
    let word = |w: [&str; 4]| w[index].to_string();
    let o = &s.order;
    let price: QuotePrice = view(&o.cart.price);
    let items: Lines = view(&o.cart.line_items);
    let currency = price.currency.as_str().unwrap_or("EUR");
    let decimals = price.currency_scale.as_u64().unwrap_or(2) as usize;
    let title = match s.kind.as_str().unwrap_or("") {
        "invoice" => word(["Invoice", "Rechnung", "Facture", "Factura"]),
        "delivery_note" => word([
            "Delivery note",
            "Lieferschein",
            "Bon de livraison",
            "Albaran",
        ]),
        _ => word([
            "Cancellation invoice",
            "Stornorechnung",
            "Facture d'annulation",
            "Factura de anulacion",
        ]),
    };
    let mut lines = vec![
        format!("{title} {}", text(&s.number)),
        text(&s.issued_at),
        text(&s.seller.name),
        text(&s.seller.address),
        format!(
            "{}: {}",
            word([
                "Tax ID",
                "Steuernummer",
                "Identifiant fiscal",
                "Identificación fiscal"
            ]),
            text(&s.seller.tax_id)
        ),
        String::new(),
        format!(
            "{}: {}",
            word(["Order", "Bestellung", "Commande", "Pedido"]),
            text(&o.order_number)
        ),
    ];
    lines.extend(seller_details(&s.seller, index));
    if let Some(reference) = s.reference_number.as_str() {
        lines.push(format!(
            "{}: {reference}",
            word(["Reference", "Bezug", "Référence", "Referencia"])
        ));
    }
    let address = if s.kind == "delivery_note" {
        &o.shipping_address
    } else {
        &o.billing_address
    };
    let address = if address.is_object() {
        address
    } else {
        &o.cart.checkout.address
    };
    let address: Address = view(address);
    for field in address.fields() {
        let line = text(field);
        if !line.is_empty() {
            lines.push(line);
        }
    }
    lines.push(String::new());
    let sign = if s.kind == "cancellation" { -1. } else { 1. };
    let scoped = v.get("delivery").is_some();
    lines.extend(positions(
        &s,
        (&items.0, scoped),
        (sign, decimals, currency),
    ));
    if s.kind != "delivery_note" {
        let shipping: Totalled = view(&o.cart.shipping_costs);
        lines.extend(totals(
            (&price, &shipping, &items.0),
            index,
            (sign, decimals, currency),
        ));
    }
    if let Some(message) = pos_message(&s) {
        lines.push(message);
    }
    if s.kind == "invoice" {
        lines.extend(super::receipt_instructions::lines(
            &s.payment_instructions,
            index,
        ));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::lines;
    use serde_json::{Value, json};

    /// The untyped projection this module replaced; the typed one must print the same lines.
    mod legacy {
        use serde_json::Value;
        fn text(v: &Value) -> String {
            v.as_str().unwrap_or("").into()
        }
        /// The seller's legal details that are filled in.
        fn seller_details(v: &Value, index: usize) -> Vec<String> {
            let word = |w: [&str; 4]| w[index].to_string();
            let mut lines = vec![];
            for (key, labels) in [
                (
                    "vatId",
                    ["VAT ID", "Umsatzsteuer-ID", "Numéro TVA", "Número IVA"],
                ),
                (
                    "registrationNumber",
                    [
                        "Register number",
                        "Registernummer",
                        "Numéro registre",
                        "Número registro",
                    ],
                ),
                (
                    "registerCourt",
                    [
                        "Register court",
                        "Registergericht",
                        "Tribunal registre",
                        "Tribunal registro",
                    ],
                ),
                (
                    "managingDirectors",
                    [
                        "Managing directors",
                        "Geschäftsführer",
                        "Gérants",
                        "Administradores",
                    ],
                ),
                (
                    "legalRepresentatives",
                    [
                        "Representatives",
                        "Vertretungsberechtigte",
                        "Représentants",
                        "Representantes",
                    ],
                ),
            ] {
                let value = text(&v["seller"][key]);
                if !value.is_empty() {
                    lines.push(format!("{}: {value}", word(labels)));
                }
            }
            lines
        }
        /// The positions of the document; a delivery note lists only what its delivery carries.
        fn positions(v: &Value, (sign, decimals, currency): (f64, usize, &str)) -> Vec<String> {
            let c = &v["order"]["cart"];
            let mut lines = vec![];
            if let Some(items) = c["lineItems"].as_array() {
                for item in items {
                    let quantity = if v["kind"] == "delivery_note" && v.get("delivery").is_some() {
                        let Some(position) = v["delivery"]["positions"].as_array().and_then(|ps| {
                            ps.iter()
                                .find(|p| p["id"] == item["referencedId"] || p["id"] == item["id"])
                        }) else {
                            continue;
                        };
                        position["quantity"].clone()
                    } else {
                        item["quantity"].clone()
                    };
                    lines.push(format!(
                        "{} x {} [{}]",
                        quantity,
                        text(&item["label"]),
                        text(&item["referencedId"])
                    ));
                    if v["kind"] != "delivery_note" {
                        lines.push(format!(
                            "  {:.decimals$} {currency}",
                            sign * item["price"]["totalPrice"].as_f64().unwrap_or(0.)
                        ));
                    }
                }
            }
            lines
        }
        /// Net, tax, total, shipping and the tax per rate.
        fn totals(
            c: &Value,
            index: usize,
            (sign, decimals, currency): (f64, usize, &str),
        ) -> Vec<String> {
            let word = |w: [&str; 4]| w[index].to_string();
            let mut lines = vec![];
            for (key, label) in [
                ("netPrice", word(["Net", "Netto", "Net", "Neto"])),
                ("tax", word(["Tax", "Steuer", "Taxe", "Impuesto"])),
                ("totalPrice", word(["Total", "Gesamt", "Total", "Total"])),
            ] {
                lines.push(format!(
                    "{label}: {:.decimals$} {currency}",
                    sign * c["price"][key].as_f64().unwrap_or(0.)
                ));
            }
            lines.push(format!(
                "{}: {:.decimals$} {currency}",
                word(["Shipping", "Versand", "Livraison", "Envío"]),
                sign * c["shippingCosts"]["totalPrice"].as_f64().unwrap_or(0.)
            ));
            if let Some(items) = c["lineItems"].as_array() {
                for item in items {
                    if let Some(taxes) = item["price"]["calculatedTaxes"].as_array() {
                        for tax in taxes {
                            lines.push(format!(
                                "{} {}%: {:.decimals$} {currency}",
                                word(["VAT", "MwSt.", "TVA", "IVA"]),
                                tax["taxRate"],
                                sign * tax["tax"].as_f64().unwrap_or(0.)
                            ));
                        }
                    }
                }
            }
            lines
        }
        pub(super) fn lines(v: &Value) -> Vec<String> {
            let lang = v["locale"].as_str().unwrap_or("en");
            let index = match lang {
                "de" => 1,
                "fr" => 2,
                "es" => 3,
                _ => 0,
            };
            let word = |w: [&str; 4]| w[index].to_string();
            let o = &v["order"];
            let c = &o["cart"];
            let currency = c["price"]["currency"].as_str().unwrap_or("EUR");
            let decimals = c["price"]["currencyScale"].as_u64().unwrap_or(2) as usize;
            let title = match v["kind"].as_str().unwrap_or("") {
                "invoice" => word(["Invoice", "Rechnung", "Facture", "Factura"]),
                "delivery_note" => word([
                    "Delivery note",
                    "Lieferschein",
                    "Bon de livraison",
                    "Albaran",
                ]),
                _ => word([
                    "Cancellation invoice",
                    "Stornorechnung",
                    "Facture d'annulation",
                    "Factura de anulacion",
                ]),
            };
            let mut lines = vec![
                format!("{title} {}", text(&v["number"])),
                text(&v["issuedAt"]),
                text(&v["seller"]["name"]),
                text(&v["seller"]["address"]),
                format!(
                    "{}: {}",
                    word([
                        "Tax ID",
                        "Steuernummer",
                        "Identifiant fiscal",
                        "Identificación fiscal"
                    ]),
                    text(&v["seller"]["taxId"])
                ),
                String::new(),
                format!(
                    "{}: {}",
                    word(["Order", "Bestellung", "Commande", "Pedido"]),
                    text(&o["orderNumber"])
                ),
            ];
            lines.extend(seller_details(v, index));
            if let Some(reference) = v["referenceNumber"].as_str() {
                lines.push(format!(
                    "{}: {reference}",
                    word(["Reference", "Bezug", "Référence", "Referencia"])
                ));
            }
            let address = if v["kind"] == "delivery_note" {
                &o["shippingAddress"]
            } else {
                &o["billingAddress"]
            };
            let address = if address.is_object() {
                address
            } else {
                &c["checkout"]["address"]
            };
            for key in [
                "name",
                "company",
                "department",
                "street",
                "additionalAddressLine1",
                "additionalAddressLine2",
                "postalCode",
                "city",
                "country",
                "vatId",
            ] {
                let line = text(&address[key]);
                if !line.is_empty() {
                    lines.push(line);
                }
            }
            lines.push(String::new());
            let cancellation = v["kind"] == "cancellation";
            let sign = if cancellation { -1. } else { 1. };
            lines.extend(positions(v, (sign, decimals, currency)));
            if v["kind"] != "delivery_note" {
                lines.extend(totals(c, index, (sign, decimals, currency)));
            }
            if let Some(values) = v["order"]["pos"]["receiptMessage"].as_object() {
                let main = v["order"]["pos"]["mainLocale"].as_str().unwrap_or("en-GB");
                let message = values
                    .get(v["locale"].as_str().unwrap_or("en"))
                    .or_else(|| {
                        values
                            .iter()
                            .find(|(key, _)| key.split('-').next() == v["locale"].as_str())
                            .map(|(_, value)| value)
                    })
                    .or_else(|| values.get(main))
                    .and_then(Value::as_str);
                if let Some(message) = message {
                    lines.push(message.into());
                }
            }
            if v["kind"] == "invoice" {
                lines.extend(crate::domain::receipt_instructions::lines(
                    &v["paymentInstructions"],
                    index,
                ));
            }
            lines
        }

        #[cfg(test)]
        mod currency_tests {
            use super::*;
            use serde_json::json;
            #[test]
            fn document_uses_invoice_currency_and_precision() {
                for (code, scale, total, expected) in [
                    ("JPY", 0, 1234., "1234 JPY"),
                    ("KWD", 3, 7.123, "7.123 KWD"),
                ] {
                    let value = json!({"locale":"en","kind":"invoice","order":{"cart":{"price":{"currency":code,"currencyScale":scale,"totalPrice":total},"lineItems":[]}}});
                    assert!(lines(&value).iter().any(|s| s.contains(expected)));
                }
            }
        }
    }

    fn order() -> Value {
        json!({"orderNumber":"10001","billingAddress":{"name":"Ada","street":"Main 1","postalCode":"10115","city":"Berlin","country":"DE","vatId":"DE1"},"shippingAddress":{"name":"Bob","city":"Hamburg"},
            "cart":{"price":{"netPrice":10.0,"tax":1.9,"totalPrice":11.9,"currency":"EUR","currencyScale":2},"shippingCosts":{"totalPrice":4.5},
            "lineItems":[{"id":"l1","referencedId":"p1","label":"Shirt","quantity":2,"price":{"totalPrice":7.0,"calculatedTaxes":[{"taxRate":19,"tax":1.12}]}},
                         {"id":"l2","referencedId":"p2","label":"Cap","quantity":1,"price":{"totalPrice":4.9,"calculatedTaxes":"x"}},
                         "broken"],
            "checkout":{"address":{"name":"Fallback"}}},
            "pos":{"receiptMessage":{"de-DE":"Danke","en-GB":"Thanks"},"mainLocale":"en-GB"}})
    }

    #[test]
    fn typed_projection_prints_exactly_what_the_untyped_one_printed() {
        let seller = json!({"name":"Shop","address":"Road 2","taxId":"T1","vatId":"V1","registerCourt":"Court"});
        let mut cases = vec![];
        for (kind, locale) in [
            ("invoice", "de"),
            ("invoice", "en"),
            ("cancellation", "fr"),
            ("delivery_note", "es"),
            ("unknown", "xx"),
        ] {
            cases.push(json!({"kind":kind,"locale":locale,"number":"R-1","issuedAt":"2026-11-24","seller":seller,"order":order(),"referenceNumber":"R-0",
                "paymentInstructions":{"instructions":{"kind":"bank_transfer"},"simulated":true}}));
        }
        cases.push(json!({"kind":"delivery_note","locale":"de","order":order(),"delivery":{"positions":[{"id":"p1","quantity":1},"broken"]}}));
        cases.push(json!({"kind":"delivery_note","locale":"de","order":order(),"delivery":null}));
        cases.push(json!({"kind":"delivery_note","locale":"en","order":order(),"delivery":{"positions":"x"}}));
        let mut bare = order();
        bare["billingAddress"] = json!(null);
        bare["cart"]["lineItems"] = json!({"not":"a list"});
        bare["pos"] = json!("x");
        cases.push(json!({"kind":"invoice","locale":"es","order":bare,"seller":null}));
        cases.push(json!({"kind":"invoice","order":{"cart":null}}));
        cases.push(json!(null));
        cases.push(json!({"kind":"invoice","locale":"de","order":{"pos":{"receiptMessage":{"de":"Hallo"}},"cart":{}}}));
        for case in cases {
            assert_eq!(lines(&case), legacy::lines(&case), "{case}");
        }
    }
}
