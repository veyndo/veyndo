//! Typed recipient instructions of a payment method and their immutable admission into the attempt context.
use crate::domain::attempt::Attempt;
use serde_json::{Value, json};
use veyndo_core::{Result, bad, conflict};

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Instructions {
    kind: String,
    payment_reference: String,
    iban: String,
    bic: String,
    bank_name: String,
    account_holder_name: String,
}
impl Instructions {
    pub fn validate(&self) -> Result<()> {
        let plain = |s: &str, max: usize| {
            !s.trim().is_empty() && s.len() <= max && !s.chars().any(char::is_control)
        };
        if self.kind != "bank_transfer"
            || !plain(&self.payment_reference, 140)
            || !self
                .payment_reference
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b" _./-".contains(&b))
            || !plain(&self.bank_name, 200)
            || !plain(&self.account_holder_name, 200)
            || ![8, 11].contains(&self.bic.len())
            || !self
                .bic
                .bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
        {
            return Err(bad("Invalid recipient payment instructions"));
        }
        let s = &self.iban;
        if s.len() < 15
            || s.len() > 34
            || !s
                .bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
            || !s.as_bytes()[..2].iter().all(u8::is_ascii_uppercase)
            || !s.as_bytes()[2..4].iter().all(u8::is_ascii_digit)
        {
            return Err(bad("Invalid recipient bank account"));
        }
        let mut remainder = 0u32;
        for b in s.bytes().skip(4).chain(s.bytes().take(4)) {
            remainder = if b.is_ascii_digit() {
                (remainder * 10 + u32::from(b - b'0')) % 97
            } else {
                (remainder * 100 + u32::from(b - b'A' + 10)) % 97
            };
        }
        if remainder != 1 {
            return Err(bad("Invalid recipient bank account checksum"));
        }
        Ok(())
    }
}
pub fn validated(value: &Value) -> Result<Value> {
    let value: Instructions = serde_json::from_value(value.clone())
        .map_err(|_| bad("Invalid recipient instructions response"))?;
    value.validate()?;
    serde_json::to_value(value).map_err(|_| bad("Invalid recipient instructions"))
}
pub fn admit(p: &Attempt, context: &mut Value, receipt: &Value) -> Result<bool> {
    let Some(value) = receipt.get("instructions") else {
        return Ok(false);
    };
    if !p.context["capabilities"]
        .as_array()
        .is_some_and(|caps| caps.iter().any(|cap| cap == "instructions"))
    {
        return Err(bad(
            "Payment method has no recipient-instructions capability",
        ));
    }
    let value = validated(value)?;
    if let Some(saved) = context.get("recipientInstructions") {
        if *saved != value {
            return Err(conflict("Accepted payment instructions are immutable"));
        }
        return Ok(false);
    }
    context["recipientInstructions"] = value;
    Ok(true)
}
pub fn snapshot(p: &Attempt, value: &Value) -> Result<Value> {
    Ok(
        json!({"instructions":validated(value)?,"amountMinor":p.amount,"currency":p.currency,"currencyScale":p.currency_scale,"simulated":p.environment!="live"}),
    )
}
#[cfg(test)]
mod tests {
    use super::{Instructions, admit};
    use crate::domain::attempt::Attempt;
    use serde_json::json;
    #[test]
    fn instructions_validate_bank_checksum_and_exclude_extra_fields() {
        let v = json!({"kind":"bank_transfer","paymentReference":"RF-123","iban":"DE89370400440532013000","bic":"DEUTDEFFXXX","bankName":"Fixture bank","accountHolderName":"Fixture recipient"});
        assert!(
            serde_json::from_value::<Instructions>(v.clone())
                .unwrap()
                .validate()
                .is_ok()
        );
        for (key, value) in [
            ("iban", json!("DE000000000000000")),
            ("paymentReference", json!("private@example.test")),
            ("kind", json!("payer_account")),
            ("bic", json!("bad")),
        ] {
            let mut next = v.clone();
            next[key] = value;
            assert!(
                serde_json::from_value::<Instructions>(next)
                    .unwrap()
                    .validate()
                    .is_err()
            );
        }
        let mut next = v;
        next["payer"] = json!("private");
        assert!(serde_json::from_value::<Instructions>(next).is_err());
    }
    #[test]
    fn recipient_is_validated_frozen_once_and_never_inferred_from_missing_evidence() {
        let mut p = Attempt {
            id: "a".into(),
            provider: "example".into(),
            context: json!({"capabilities":["instructions"]}),
            tenant: "shop".into(),
            order: "o".into(),
            amount: 123,
            currency: "EUR".into(),
            currency_scale: 2,
            state: "ready".into(),
            provider_order: Some("remote".into()),
            capture: None,
            refunded: 0,
            adapter_version: "1.0.0".into(),
            environment: "sandbox".into(),
            bn_code: String::new(),
        };
        let receipt = json!({"instructions":{"kind":"bank_transfer","paymentReference":"RF-1","iban":"DE89370400440532013000","bic":"DEUTDEFFXXX","bankName":"Example","accountHolderName":"Recipient"}});
        let mut context = p.context.clone();
        assert!(!admit(&p, &mut context, &json!({})).unwrap());
        assert!(admit(&p, &mut context, &receipt).unwrap());
        assert!(!admit(&p, &mut context, &receipt).unwrap());
        let frozen = context.clone();
        let mut changed = receipt.clone();
        changed["instructions"]["paymentReference"] = json!("RF-2");
        assert!(admit(&p, &mut context, &changed).is_err());
        assert_eq!(context, frozen);
        let mut invalid = receipt.clone();
        invalid["instructions"]["iban"] = json!("DE00370400440532013000");
        assert!(admit(&p, &mut json!({}), &invalid).is_err());
        p.context = json!({});
        assert!(admit(&p, &mut json!({}), &receipt).is_err());
    }
}
