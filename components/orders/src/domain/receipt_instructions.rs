//! Four-language invoice recipient footer from the immutable native receipt snapshot; cancellations never request payment.
use serde_json::Value;
pub fn lines(v: &Value, index: usize) -> Vec<String> {
    let i = &v["instructions"];
    if i["kind"] != "bank_transfer" {
        return vec![];
    }
    let word = |v: [&str; 4]| v[index.min(3)].to_owned();
    let mut lines = vec![
        String::new(),
        word([
            "Invoice payment details",
            "Angaben zum Rechnungskauf",
            "Informations du paiement sur facture",
            "Datos del pago por factura",
        ]),
    ];
    if v["simulated"] == true {
        lines.push(word([
            "Simulated bank details. Do not transfer money.",
            "Simulierte Bankdaten. Bitte kein Geld überweisen.",
            "Coordonnées simulées. Ne transférez pas d'argent.",
            "Datos simulados. No transfieras dinero.",
        ]));
    }
    for (key, labels) in [
        (
            "accountHolderName",
            ["Recipient", "Empfänger", "Bénéficiaire", "Destinatario"],
        ),
        ("bankName", ["Bank", "Bank", "Banque", "Banco"]),
        ("iban", ["IBAN", "IBAN", "IBAN", "IBAN"]),
        ("bic", ["BIC", "BIC", "BIC", "BIC"]),
        (
            "paymentReference",
            [
                "Payment reference",
                "Verwendungszweck",
                "Référence du paiement",
                "Referencia del pago",
            ],
        ),
    ] {
        lines.push(format!(
            "{}: {}",
            word(labels),
            i[key].as_str().unwrap_or("")
        ));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::lines;
    use serde_json::json;

    #[test]
    fn the_footer_names_the_recipient_in_the_document_language() {
        let v = json!({"simulated":true,"instructions":{"kind":"bank_transfer","accountHolderName":"Shop GmbH","bankName":"Example Bank","iban":"DE89370400440532013000","bic":"DEUTDEFF","paymentReference":"RF-1"}});
        let de = lines(&v, 1);
        assert_eq!(de[0], "");
        assert_eq!(de[1], "Angaben zum Rechnungskauf");
        assert_eq!(de[2], "Simulierte Bankdaten. Bitte kein Geld überweisen.");
        assert_eq!(
            de[3..],
            [
                "Empfänger: Shop GmbH",
                "Bank: Example Bank",
                "IBAN: DE89370400440532013000",
                "BIC: DEUTDEFF",
                "Verwendungszweck: RF-1"
            ]
        );
        let mut live = v.clone();
        live["simulated"] = json!(false);
        let en = lines(&live, 0);
        assert_eq!(en.len(), 7);
        assert_eq!(en[2], "Recipient: Shop GmbH");
        assert_eq!(lines(&live, 9)[1], "Datos del pago por factura");
    }

    #[test]
    fn other_payment_kinds_print_no_footer() {
        assert!(lines(&json!({}), 0).is_empty());
        assert!(lines(&json!({"instructions":{"kind":"payer_account"}}), 0).is_empty());
    }
}
