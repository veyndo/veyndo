//! Explicit UCP consent reuses native cart-bound privacy receipts; source documents never define consent policy or authority.
use crate::foundation::App;
use crate::ucp::UCP_VERSION;
use crate::ucp::document::{UcpCapability, UcpDocument, UcpMessage};
use crate::ucp::messages;
use crate::ucp::views::{
    BuyerConsent, ConsentDecision, ConsentReceipt, ConsentSlot, OrderSnapshot, SessionInput,
    keep_present,
};
use crate::{commerce, legal, marketing};
use axum::extract::State;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use serde_json::json;
use std::collections::BTreeMap;
use std::collections::HashMap;
use veyndo_core::Result;
use veyndo_core::bad;
use veyndo_core::conflict;
use veyndo_core::document::{lenient, view};
use veyndo_core::request_context::RequestContext;
use veyndo_core::shop_scope::tenant;
use veyndo_shop::types::cart::StoredCart;
pub(crate) const CAPABILITY: &str = "dev.ucp.shopping.buyer_consent";
const PURPOSES: &[(&str, &str)] = &[
    ("dev.ucp.consent.analytics", "analytics"),
    ("dev.ucp.consent.preferences", "personalization"),
    ("dev.ucp.consent.marketing", "marketing"),
    ("dev.ucp.consent.sale_or_sharing", "saleOrSharing"),
];
const EMAIL: &str = "dev.ucp.consent.marketing.email";
const SMS: &str = "dev.ucp.consent.marketing.sms";
const MARKETING: &str = "dev.ucp.consent.marketing";
/// The JSON fields `admit` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct AdmitInput {
    buyer: Value,
}

pub(super) fn admit(v: &Value, negotiated: bool) -> Result<()> {
    let input: AdmitInput = view(v);
    if !negotiated && input.buyer.get("consent").is_some() {
        return Err(conflict(
            "capabilities_incompatible: negotiate buyer_consent before supplying choices",
        ));
    }
    Ok(())
}
pub(super) fn buyer(v: &Value, current: Option<&Value>) -> Result<Option<Value>> {
    let previous = current.and_then(|b| view::<ConsentSlot>(b).consent);
    let Some(incoming) = view::<SessionInput>(v).buyer else {
        return Ok(previous.map(|consent| json!({"consent":consent})));
    };
    if incoming.is_null() {
        return Ok(None);
    }
    let Some(fields) = incoming
        .as_object()
        .filter(|_| incoming.to_string().len() <= 16000)
    else {
        return Err(bad("Buyer must be a bounded object"));
    };
    let mut buyer = fields.clone();
    if let Some(choices) = view::<ConsentSlot>(&incoming).consent {
        let choices = choices
            .as_object()
            .filter(|v| v.len() <= 4)
            .ok_or(bad("Invalid buyer consent"))?;
        for (purpose, decision) in choices {
            if !PURPOSES.iter().any(|(id, _)| id == purpose) {
                return Err(bad("Unknown consent purpose"));
            }
            validate(decision, purpose == MARKETING)?;
        }
        let mut merged = previous
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default();
        merged.extend(choices.clone());
        buyer.insert("consent".into(), Value::Object(merged));
    } else if let Some(previous) = previous {
        buyer.insert("consent".into(), previous);
    }
    Ok(Some(Value::Object(buyer)))
}
/// The JSON fields `validate` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ValidateInput {
    #[serde(deserialize_with = "lenient")]
    granted: Option<bool>,
    source: Value,
    #[serde(deserialize_with = "keep_present")]
    segments: Option<Value>,
}

fn validate(v: &Value, segments: bool) -> Result<()> {
    let input: ValidateInput = view(v);
    let object = v.as_object().ok_or(bad("Invalid consent decision"))?;
    if input.granted.is_none()
        || input.source != "platform"
        || object
            .keys()
            .any(|k| !matches!(k.as_str(), "granted" | "source" | "segments"))
    {
        return Err(bad(
            "Consent requires an explicit platform choice; policy metadata is server-owned",
        ));
    }
    if let Some(values) = &input.segments {
        let values = values
            .as_object()
            .filter(|v| segments && v.len() <= 2)
            .ok_or(bad("Invalid consent segments"))?;
        for (key, value) in values {
            if key != EMAIL && key != SMS {
                return Err(bad("Unknown consent segment"));
            }
            validate(value, false)?;
        }
    }
    Ok(())
}
pub(super) fn missing(buyer: &Value) -> bool {
    legal::missing_contact(buyer)
}
/// Only explicitly supplied purposes may grant consent; omissions never revive an expired or withdrawn choice.
pub(super) async fn prepare(
    a: &App,
    h: &RequestContext,
    v: &Value,
) -> Result<Option<(String, HashMap<String, bool>)>> {
    let Some(buyer) = view::<SessionInput>(v).buyer else {
        return Ok(None);
    };
    let explicit = buyer.is_null();
    let Some(choices) = view::<ConsentSlot>(&buyer)
        .consent
        .or(explicit.then_some(Value::Null))
    else {
        return Ok(None);
    };
    let choices = choices.as_object().cloned().unwrap_or_default();
    let (s, _) = commerce::scoped_config(a, &tenant(h)?, marketing::channel_id(h)).await?;
    let patch = PURPOSES
        .iter()
        .filter(|(id, native)| {
            *native != "saleOrSharing" && (explicit || choices.contains_key(*id))
        })
        .map(|(id, native)| {
            let decision = ConsentDecision::of(choices.get(*id));
            let granted = decision.granted()
                || (*id == MARKETING
                    && [EMAIL, SMS]
                        .iter()
                        .any(|segment| decision.segment(segment).granted()));
            ((*native).to_owned(), granted)
        })
        .collect();
    Ok(Some((legal::policy_version(&s.legal), patch)))
}
/// One projected consent choice.
#[derive(Serialize)]
struct ConsentChoice {
    granted: bool,
    source: &'static str,
    description: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    segments: Option<BTreeMap<&'static str, ConsentChoice>>,
}
/// Projects the consent choices from the native receipt and the buyer's decisions.
fn projected_choices(
    receipt: &ConsentReceipt,
    original: &BuyerConsent,
    locale: &str,
) -> BTreeMap<&'static str, ConsentChoice> {
    let mut consent = BTreeMap::new();
    for (id, native) in PURPOSES {
        let decision = original.decision(id);
        let active = receipt.active(native);
        let source = if decision.source == "platform" && receipt.decided == Some(true) {
            "platform"
        } else {
            "business"
        };
        let mut choice = ConsentChoice {
            granted: active && decision.granted(),
            source,
            description: messages::consent(native, locale),
            segments: None,
        };
        if *id == MARKETING {
            let segments = [(EMAIL, "email"), (SMS, "sms")].map(|(segment, contact)| {
                let granted = decision
                    .segment(segment)
                    .granted
                    .unwrap_or(decision.granted());
                (
                    segment,
                    ConsentChoice {
                        granted: active && granted,
                        source,
                        description: messages::consent(contact, locale),
                        segments: None,
                    },
                )
            });
            choice.segments = Some(BTreeMap::from(segments));
        }
        consent.insert(*id, choice);
    }
    consent
}
pub(super) async fn project(
    a: &App,
    h: &RequestContext,
    c: &StoredCart,
    doc: &mut UcpDocument,
) -> Result<()> {
    let snapshot = c
        .data
        .order
        .as_ref()
        .map(|order| view::<OrderSnapshot>(order).legal);
    let (receipt, original): (ConsentReceipt, BuyerConsent) = if let Some(snapshot) = snapshot {
        (
            view(&snapshot.consent),
            view(&json!({"consent":snapshot.buyer_consent})),
        )
    } else {
        (
            view(
                &legal::consent_receipt(State(a.core.clone()), h.clone())
                    .await?
                    .0,
            ),
            c.data.buyer.as_ref().map(view).unwrap_or_default(),
        )
    };
    let consent = serde_json::to_value(projected_choices(&receipt, &original, &c.data.locale))
        .expect("Consent choices serialize");
    let buyer = doc.buyer.get_or_insert(Value::Null);
    if !buyer.is_object() {
        *buyer = json!({});
    }
    if let Some(fields) = buyer.as_object_mut() {
        fields.insert("consent".into(), consent);
    }
    doc.ucp
        .capabilities
        .insert(CAPABILITY, UcpCapability::version(UCP_VERSION));
    if c.status == "open" && missing(buyer) {
        doc.status = Some("incomplete");
        doc.messages.push(UcpMessage {
            kind: "warning",
            code: "missing_consent_data",
            content: messages::consent("missing", &c.data.locale),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The projection as it was built from indexed JSON before the typed choices.
    fn legacy(receipt: &Value, original: &Value, locale: &str) -> Value {
        let mut consent = json!({});
        for (id, native) in PURPOSES {
            let decision = &original["consent"][*id];
            let active = receipt["decided"] == true && receipt["choices"][*native] == true;
            let explicit = decision["source"] == "platform" && receipt["decided"] == true;
            let mut choice = json!({"granted":active && decision["granted"]==true,"source":if explicit{"platform"}else{"business"},"description":messages::consent(native,locale)});
            if *id == MARKETING {
                for (segment, contact) in [(EMAIL, "email"), (SMS, "sms")] {
                    let granted = decision["segments"][segment]["granted"]
                        .as_bool()
                        .unwrap_or(decision["granted"] == true);
                    choice["segments"][segment] = json!({"granted":active && granted,"source":if explicit{"platform"}else{"business"},"description":messages::consent(contact,locale)});
                }
            }
            consent[*id] = choice;
        }
        consent
    }

    #[test]
    fn typed_choices_match_the_indexed_projection() {
        let receipts = [
            json!(null),
            json!({"decided":true,"choices":{"analytics":true,"marketing":true,"personalization":false}}),
            json!({"decided":false,"choices":{"analytics":true}}),
            json!({"decided":true,"choices":"all"}),
        ];
        let buyers = [
            json!(null),
            json!({"consent":null}),
            json!({"consent":{"dev.ucp.consent.analytics":{"granted":true,"source":"platform"}}}),
            json!({"consent":{MARKETING:{"granted":false,"source":"platform","segments":{EMAIL:{"granted":true,"source":"platform"}}}}}),
            json!({"consent":{MARKETING:{"granted":true,"segments":{SMS:{"granted":false},EMAIL:"x"}},"dev.ucp.consent.preferences":7}}),
        ];
        for receipt in &receipts {
            for buyer in &buyers {
                for locale in ["en-GB", "de-DE"] {
                    let typed = projected_choices(&view(receipt), &view(buyer), locale);
                    assert_eq!(
                        serde_json::to_value(typed).unwrap(),
                        legacy(receipt, buyer, locale)
                    );
                }
            }
        }
    }

    #[test]
    fn buyer_merges_choices_like_indexed_assignment() {
        let current = json!({"email":"a@b.c","consent":{"dev.ucp.consent.analytics":{"granted":true,"source":"platform"}}});
        let merged = buyer(
            &json!({"buyer":{"name":"B","consent":{"dev.ucp.consent.preferences":{"granted":false,"source":"platform"}}}}),
            Some(&current),
        )
        .unwrap();
        assert_eq!(
            merged,
            Some(
                json!({"name":"B","consent":{"dev.ucp.consent.analytics":{"granted":true,"source":"platform"},"dev.ucp.consent.preferences":{"granted":false,"source":"platform"}}})
            )
        );
        assert_eq!(
            buyer(&json!({}), Some(&current)).unwrap(),
            Some(json!({"consent":current["consent"]}))
        );
        assert_eq!(
            buyer(&json!({"buyer":{"name":"C"}}), Some(&current)).unwrap(),
            Some(json!({"name":"C","consent":current["consent"]}))
        );
        assert_eq!(buyer(&json!({"buyer":null}), Some(&current)).unwrap(), None);
        assert!(buyer(&json!({"buyer":"x"}), None).is_err());
        assert!(buyer(&json!({"buyer":{"consent":null}}), None).is_err());
        assert!(
            buyer(
                &json!({"buyer":{"consent":{MARKETING:{"granted":true,"source":"platform","segments":{EMAIL:{"granted":true,"source":"platform"}}}}}}),
                None
            )
            .is_ok()
        );
    }
}
