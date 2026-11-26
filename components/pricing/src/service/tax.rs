//! The tax rate of a delivery destination: the most specific active rule, else the country rate, else the class default.
use crate::decimal::Decimal;
use crate::types::tax::{Destination, DestinationRule, TaxConfig};
use veyndo_core::{Result, bad};
use veyndo_kernel::verified_kernel;

pub fn destination_rate(t: &TaxConfig, c: &Destination, today: &str) -> Result<Decimal> {
    let state = c.state;
    let postal = c.postal_code;
    let mut matches = t
        .rules
        .iter()
        .filter(|r| {
            let date = r.active_from.as_deref().is_none_or(|d| d <= today)
                && r.active_until.as_deref().is_none_or(|d| d >= today);
            let zip = (r.postal_codes.is_empty() || r.postal_codes.iter().any(|x| x == postal))
                && (r.postal_prefixes.is_empty()
                    || r.postal_prefixes.iter().any(|x| postal.starts_with(x)))
                && r.postal_from.as_ref().is_none_or(|a| {
                    postal.bytes().all(|x| x.is_ascii_digit())
                        && postal.len() == a.len()
                        && postal >= a.as_str()
                        && postal <= r.postal_to.as_deref().unwrap_or("")
                });
            verified_kernel::destination_tax_admissible(
                r.condition.is_none(),
                r.country == c.country,
                r.states.is_empty() || r.states.iter().any(|x| x == state),
                zip,
                date,
            )
        })
        .collect::<Vec<_>>();
    matches.sort_by(|a, b| {
        b.priority
            .cmp(&a.priority)
            .then_with(|| b.specificity().cmp(&a.specificity()))
            .then_with(|| a.id.cmp(&b.id))
    });
    matches
        .first()
        .map(|r| r.rate)
        .or_else(|| t.rates.get(c.country).copied())
        .or(t.default_rate)
        .ok_or(bad("Country tax rate missing"))
}
impl DestinationRule {
    fn specificity(&self) -> usize {
        usize::from(!self.states.is_empty())
            + 2 * usize::from(
                !self.postal_codes.is_empty()
                    || !self.postal_prefixes.is_empty()
                    || self.postal_from.is_some(),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::destination_rate;
    use crate::types::tax::{Destination, DestinationRule, TaxConfig};
    use serde_json::json;
    fn tax() -> TaxConfig {
        serde_json::from_value(
            json!({"id":"standard","rates":{"US":5},"defaultRate":19,"rules":[]}),
        )
        .unwrap()
    }
    fn rule(id: &str, rate: f64, priority: i32) -> DestinationRule {
        serde_json::from_value(
            json!({"id":id,"country":"US","rate":rate,"priority":priority,"states":["US-CA"]}),
        )
        .unwrap()
    }
    #[test]
    fn destination_order_boundaries_and_fail_closed_condition() {
        let mut t = tax();
        let mut c = Destination {
            country: "US",
            state: "US-CA",
            postal_code: "90210",
        };
        t.rules = vec![rule("state", 7.25, 0), rule("postal", 9.5, 0)];
        t.rules[1].postal_from = Some("90000".into());
        t.rules[1].postal_to = Some("91999".into());
        assert_eq!(
            destination_rate(&t, &c, "2026-10-05")
                .unwrap()
                .to_wire()
                .unwrap(),
            9.5
        );
        t.rules[0].priority = 1;
        assert_eq!(
            destination_rate(&t, &c, "2026-10-05")
                .unwrap()
                .to_wire()
                .unwrap(),
            7.25
        );
        t.rules[0].condition = Some(json!({"type":"alwaysValid"}));
        assert_eq!(
            destination_rate(&t, &c, "2026-10-05")
                .unwrap()
                .to_wire()
                .unwrap(),
            9.5
        );
        t.rules[1].active_until = Some("2026-10-04".into());
        assert_eq!(
            destination_rate(&t, &c, "2026-10-05")
                .unwrap()
                .to_wire()
                .unwrap(),
            5.
        );
        c.country = "ZZ";
        assert_eq!(
            destination_rate(&t, &c, "2026-10-05")
                .unwrap()
                .to_wire()
                .unwrap(),
            19.
        );
        t.default_rate = None;
        assert!(destination_rate(&t, &c, "2026-10-05").is_err());
    }
    #[test]
    fn validity_inclusive_and_alpha_postcode_cannot_enter_numeric_range() {
        let mut t = tax();
        t.rules = vec![rule("range", 9.5, 0)];
        t.rules[0].postal_from = Some("90000".into());
        t.rules[0].postal_to = Some("91999".into());
        t.rules[0].active_from = Some("2026-10-05".into());
        t.rules[0].active_until = Some("2026-10-05".into());
        let mut c = Destination {
            country: "US",
            state: "US-CA",
            postal_code: "91000",
        };
        assert_eq!(
            destination_rate(&t, &c, "2026-10-05")
                .unwrap()
                .to_wire()
                .unwrap(),
            9.5
        );
        c.postal_code = "910AA";
        assert_eq!(
            destination_rate(&t, &c, "2026-10-05")
                .unwrap()
                .to_wire()
                .unwrap(),
            5.
        );
    }
}
