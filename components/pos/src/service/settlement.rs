//! Exact cash admission and order annotation execute in the existing checkout transaction.
use crate::service::guard;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use veyndo_checkout::store::selected_customer;
use veyndo_core::document::{lenient, merge, view};
use veyndo_core::request_context::PosAdmission;
use veyndo_core::store::environments::live_parent;
use veyndo_core::{Error, RequestContext, Result, StatusCode, bad};
use veyndo_identity::permit;
use veyndo_pricing::verified_kernel;
use veyndo_shop::types::cart::{StoredCart, selection};
use veyndo_shop::types::settings::Settings;
pub fn validate_settlement(
    h: &RequestContext,
    c: &StoredCart,
    q: &Value,
    cfg: &Settings,
    minor: i64,
) -> Result<()> {
    guard::cart(h, c)?;
    let quote: ReviewedQuote = view(q);
    let cash = quote.payment_method.mode.as_deref() == Some("cash");
    if cash && h.pos.is_none() {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Cash requires authenticated POS checkout".into(),
        ));
    }
    if let Some(p) = &h.pos {
        permit(h, "pos.use")?;
        permit(h, "orders.write")?;
        if cash {
            permit(h, "payments.manage")?;
            let total = u64::try_from(minor).map_err(|_| bad("Invalid POS total"))?;
            if !verified_kernel::pos_cash_admissible(
                true,
                p.tender_minor.unwrap_or(0),
                total,
                quote.selection_needs_confirmation != Some(true),
            ) {
                return Err(bad(
                    "Cash tender is below the reviewed total or outside the supported range",
                ));
            }
        } else if p.tender_minor.is_some() {
            return Err(bad("Cash tender supplied for a non-cash payment"));
        }
        if selection(&c.data).shipping_method_id != "pickup"
            || !cfg
                .shipping
                .iter()
                .any(|s| s.id == "pickup" && s.active && s.price == 0.)
        {
            return Err(bad("POS checkout requires free collection"));
        }
    }
    Ok(())
}
pub async fn settle(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    h: &RequestContext,
    c: &StoredCart,
    order: &mut Value,
) -> Result<()> {
    let Some(p) = &h.pos else {
        return Ok(());
    };
    let main_locale = sqlx::query_scalar::<_, Value>(
        "SELECT data->'mainLocale' FROM commerce_settings WHERE tenant=$1",
    )
    .bind(&c.tenant)
    .fetch_one(&mut **tx)
    .await?;
    let receipt_message = sqlx::query_scalar::<_, Value>("SELECT coalesce(data->'pos'->'receiptMessage','{}'::jsonb) FROM sales_channels WHERE tenant=$1 AND id=$2").bind(&c.tenant).bind(&c.data.sales_channel).fetch_one(&mut **tx).await?;
    let cash = annotate(
        order,
        p,
        (&c.data.sales_channel, &c.data.currency),
        (main_locale, receipt_message),
    )?;
    let placed: PlacedOrder = view(order);
    if cash {
        veyndo_core::store::outbox::publish_on(&mut **tx, &c.tenant, "payment.state_changed", &json!({"orderId":placed.id,"state":"paid","kind":"payment","cashierId":p.actor,"salesChannelId":c.data.sales_channel})).await?;
    }
    let event = if placed.payment.state == "paid" {
        "pos.sale_completed"
    } else {
        "pos.sale_created"
    };
    veyndo_core::store::outbox::publish_on(&mut **tx, &c.tenant, event, &json!({"orderId":placed.id,"salesChannelId":c.data.sales_channel,"cashierId":p.actor,"paymentState":placed.payment.state,"totalMinor":placed.money.minor,"currency":c.data.currency})).await?;
    Ok(())
}

/// The quote facts a POS settlement is admitted on.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ReviewedQuote {
    #[serde(deserialize_with = "lenient")]
    payment_method: PaymentMode,
    #[serde(deserialize_with = "lenient")]
    selection_needs_confirmation: Option<bool>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct PaymentMode {
    #[serde(deserialize_with = "lenient")]
    mode: Option<String>,
}

/// The placed order facts a POS sale reads.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct PlacedOrder {
    id: Value,
    sales_channel_id: Value,
    #[serde(deserialize_with = "lenient")]
    payment: PlacedPayment,
    #[serde(deserialize_with = "lenient")]
    money: PlacedMoney,
    #[serde(deserialize_with = "lenient")]
    cart: PlacedCart,
    pos: Value,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct PlacedPayment {
    #[serde(deserialize_with = "lenient")]
    method: PaymentMode,
    state: Value,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct PlacedMoney {
    minor: Value,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct PlacedCart {
    #[serde(deserialize_with = "lenient")]
    price: PlacedPrice,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct PlacedPrice {
    currency: Value,
}

/// The POS note of an order: who sold it where, the receipt text and, for cash, the counted tender.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SaleNote<'a> {
    cashier_id: &'a str,
    sales_channel_id: &'a str,
    main_locale: Value,
    receipt_message: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    completion_event: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cash: Option<CashNote<'a>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CashNote<'a> {
    tender_minor: String,
    change_minor: String,
    received_minor: String,
    currency: &'a str,
}

/// A payment received in cash: no electronic provider charge was performed.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CashPayment {
    provider: &'static str,
    state: &'static str,
    cash_received: bool,
}

#[derive(Serialize)]
struct PosUpdate<'a> {
    pos: SaleNote<'a>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Completion {
    completion_event: bool,
}

/// Writes the POS note into a placed order and, for cash, the received payment; true when cash was received.
fn annotate(
    order: &mut Value,
    p: &PosAdmission,
    (channel, currency): (&str, &str),
    (main_locale, receipt_message): (Value, Value),
) -> Result<bool> {
    let placed: PlacedOrder = view(order);
    let mut note = SaleNote {
        cashier_id: &p.actor,
        sales_channel_id: channel,
        main_locale,
        receipt_message,
        completion_event: None,
        cash: None,
    };
    let cash = placed.payment.method.mode.as_deref() == Some("cash");
    if cash {
        let total = placed
            .money
            .minor
            .as_i64()
            .and_then(|v| u64::try_from(v).ok())
            .ok_or(bad("Invalid POS total"))?;
        let tender = p.tender_minor.ok_or(bad("Cash tender required"))?;
        note.completion_event = Some(true);
        // No electronic provider charge was performed. Cash is explicit and audited separately.
        note.cash = Some(CashNote {
            tender_minor: tender.to_string(),
            change_minor: (tender - total).to_string(),
            received_minor: total.to_string(),
            currency,
        });
    }
    merge(order, PosUpdate { pos: note })?;
    if cash {
        let payment = order
            .as_object_mut()
            .map(|o| o.entry("payment").or_insert(Value::Null));
        merge(
            payment.ok_or(bad("Invalid POS order"))?,
            CashPayment {
                provider: "cash",
                state: "paid",
                cash_received: true,
            },
        )?;
    }
    Ok(cash)
}
/// The shared configuration lock precedes this channel lock in both saves and checkout.
pub async fn check_locked(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    h: &RequestContext,
    c: &StoredCart,
) -> Result<()> {
    if live_parent(tx, &c.tenant).await?.is_some() {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "POS settlement is unavailable in staging".into(),
        ));
    }
    selected_customer::check(tx, c).await?;
    if c.data.pos_discount.is_some() {
        permit(h, "payments.manage")?;
    }
    let data: Value =
        sqlx::query_scalar("SELECT data FROM sales_channels WHERE tenant=$1 AND id=$2 FOR SHARE")
            .bind(&c.tenant)
            .bind(&c.data.sales_channel)
            .fetch_one(&mut **tx)
            .await?;
    let channel: PosChannel = view(&data);
    if channel.kind.as_deref() != Some("pos")
        || channel.active != Some(true)
        || channel.pos.country.as_deref() != Some(selection(&c.data).country.as_str())
        || h.pos.is_none()
    {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "POS channel is paused or its location changed".into(),
        ));
    }
    Ok(())
}

/// The facts of a sales channel a POS settlement depends on.
#[derive(Deserialize, Default)]
#[serde(default)]
struct PosChannel {
    #[serde(deserialize_with = "lenient")]
    kind: Option<String>,
    #[serde(deserialize_with = "lenient")]
    active: Option<bool>,
    #[serde(deserialize_with = "lenient")]
    pos: PosLocation,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct PosLocation {
    #[serde(deserialize_with = "lenient")]
    country: Option<String>,
}

/// Provider confirmation rejoins the same POS lifecycle, once, inside its receipt transaction.
pub async fn payment_captured(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant: &str,
    order: &mut Value,
    state: &str,
) -> Result<()> {
    let placed: PlacedOrder = view(order);
    if state != "captured"
        || !placed.pos.is_object()
        || placed.pos.get("completionEvent") == Some(&Value::Bool(true))
    {
        return Ok(());
    }
    if let Some(pos) = order.get_mut("pos") {
        merge(
            pos,
            Completion {
                completion_event: true,
            },
        )?;
    }
    veyndo_core::store::outbox::publish_on(&mut **tx, tenant, "pos.sale_completed", &json!({"orderId":placed.id,"salesChannelId":placed.sales_channel_id,"paymentState":state,"totalMinor":placed.money.minor,"currency":placed.cart.price.currency})).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::annotate;
    use serde_json::{Value, json};
    use veyndo_core::Result;
    use veyndo_core::bad;
    use veyndo_core::request_context::PosAdmission;

    /// The previous untyped annotation, kept as the oracle of the typed one.
    fn legacy(
        order: &mut Value,
        p: &PosAdmission,
        channel: &str,
        currency: &str,
        locale: Value,
        message: Value,
    ) -> Result<bool> {
        order["pos"] = json!({"cashierId":p.actor,"salesChannelId":channel});
        order["pos"]["mainLocale"] = locale;
        order["pos"]["receiptMessage"] = message;
        let cash = order["payment"]["method"]["mode"] == "cash";
        if cash {
            let total = order["money"]["minor"]
                .as_i64()
                .and_then(|v| u64::try_from(v).ok())
                .ok_or(bad("Invalid POS total"))?;
            let tender = p.tender_minor.ok_or(bad("Cash tender required"))?;
            order["payment"]["provider"] = json!("cash");
            order["payment"]["state"] = json!("paid");
            order["payment"]["cashReceived"] = json!(true);
            order["pos"]["completionEvent"] = json!(true);
            order["pos"]["cash"] = json!({"tenderMinor":tender.to_string(),"changeMinor":(tender-total).to_string(),"receivedMinor":total.to_string(),"currency":currency});
        }
        Ok(cash)
    }

    #[test]
    fn typed_annotation_writes_what_the_untyped_one_wrote() {
        let orders = [
            json!({"id":"o1","money":{"minor":1250},"payment":{"method":{"mode":"cash"},"state":"pending","attemptId":"a"},"pos":{"old":true}}),
            json!({"id":"o2","money":{"minor":1250},"payment":{"method":{"mode":"card"},"state":"authorized"}}),
            json!({"id":"o3","money":{"minor":"1250"},"payment":{"method":{"mode":"cash"}}}),
            json!({"id":"o4","money":{"minor":-5},"payment":{"method":{"mode":"cash"}}}),
            json!({"id":"o5","money":[1250],"payment":{"method":["cash"]}}),
            json!({"id":"o6"}),
        ];
        for tender in [Some(2000), None] {
            let p = PosAdmission {
                actor: "cashier".into(),
                tender_minor: tender,
            };
            for order in &orders {
                let (mut typed, mut old) = (order.clone(), order.clone());
                let a = annotate(
                    &mut typed,
                    &p,
                    ("pos-1", "EUR"),
                    (json!("de-DE"), json!({"de":"Danke"})),
                );
                let b = legacy(
                    &mut old,
                    &p,
                    "pos-1",
                    "EUR",
                    json!("de-DE"),
                    json!({"de":"Danke"}),
                );
                assert_eq!(a.as_ref().ok(), b.as_ref().ok(), "{order}");
                // A refused settlement rolls back its transaction; only an admitted one's document is kept.
                if a.is_ok() {
                    assert_eq!(typed, old, "{order}");
                }
                assert_eq!(a.err().map(|e| e.1), b.err().map(|e| e.1), "{order}");
            }
        }
    }
}
