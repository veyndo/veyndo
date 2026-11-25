//! The cashier gateway shared by HTTP and MCP: a trusted cashier context for one register, then the existing cart, checkout, payment, catalogue and receipt owners.
use crate::domain::input::Input;
use crate::ports::Register;
use crate::service::{customer, discount};
use crate::store::{carts, register};
use serde_json::{Value, json};
use veyndo_app_model::declarations::identifier;
use veyndo_catalog::types::catalog_page::CatalogCriteria;
use veyndo_checkout::service::checkout_options;
use veyndo_checkout::store::cart_create::new_cart_context;
use veyndo_core::request_context::PosAdmission as Admission;
use veyndo_core::shop_scope::header;
use veyndo_core::{Error, RequestContext, Result, State, StatusCode, bad};
use veyndo_identity::{merchant, permit};
use veyndo_intelligence::http::legal;
use veyndo_marketing::service::{channels, coupons};
use veyndo_marketing::types::channel::Channel;
use veyndo_orders::domain::receipt_text;
use veyndo_payments::http::status::read_sale;
use veyndo_payments::store::commands;
use veyndo_shop::service::language::language_context;
use veyndo_shop::service::read_cache::settings;

/// The POS channels of the shop with their revision and the main language for the register picker.
pub async fn channels(a: &State, h: &RequestContext) -> Result<Value> {
    let t = merchant(h)?;
    permit(h, "pos.use")?;
    let rows = register::channels(a, &t).await?;
    let config = settings(a, &t, "default").await?.0;
    Ok(
        json!({"channels":rows.iter().map(|r|json!({"id":r.id,"data":r.data,"revision":r.revision})).collect::<Vec<_>>(),"mainLocale":config.main_locale}),
    )
}

/// The register's checkout options, privacy policy and currencies.
async fn register_context(reg: &dyn Register, a: &State, h: &RequestContext) -> Result<Value> {
    Ok(json!({
        "options": checkout_options::options(a, reg.quotes(), h).await?,
        "policy": legal::invoke(a, h, "privacy.policy", &json!({})).await?,
        "currencies": reg.currency(h, "currency.list", &json!({})).await?,
    }))
}

/// A cash tender is a plain minor-unit amount of at most 13 digits.
pub fn tender(value: Option<&str>) -> Result<Option<u64>> {
    value
        .map(|v| {
            if v.is_empty() || v.len() > 13 || !v.bytes().all(|b| b.is_ascii_digit()) {
                return Err(bad("Invalid cash tender"));
            }
            v.parse::<u64>().map_err(|_| bad("Invalid cash tender"))
        })
        .transpose()
}

/// One cashier operation on a register channel, after the POS and catalogue rights.
pub async fn invoke(
    reg: &dyn Register,
    a: &State,
    original: &RequestContext,
    op: &str,
    v: &Value,
) -> Result<Value> {
    let input: Input = serde_json::from_value(v.clone()).map_err(|_| bad("Invalid POS input"))?;
    let t = merchant(original)?;
    permit(original, "pos.use")?;
    permit(original, "catalog.read")?;
    if op == "channels" {
        return channels(a, original).await;
    }
    let channel = input
        .channel_id
        .clone()
        .filter(|id| identifier(id))
        .ok_or(bad("POS channel required"))?;
    let channel = channel.as_str();
    let mut h = original.clone();
    h.insert(
        "sw-sales-channel-id",
        channel.parse().map_err(|_| bad("Invalid POS channel"))?,
    );
    let actor = header(&h, "x-rac-user")
        .ok_or(bad("Cashier identity required"))?
        .to_owned();
    let tender = tender(input.tender_minor.as_deref())?;
    h.pos = Some(Admission {
        actor: actor.clone(),
        tender_minor: tender,
    });
    let data = channels::channel(a, &h, channel, &language_context(a, &h).await?.0)
        .await?
        .ok_or(bad("POS channel unavailable"))?;
    if data.kind != "pos" || !data.active {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "POS channel is paused or unavailable".into(),
        ));
    }
    if let Some(token) = &input.cart_token {
        h.insert(
            "sw-context-token",
            token.parse().map_err(|_| bad("Invalid POS cart token"))?,
        );
    }
    if ["customers", "customer", "discount", "receipt"].contains(&op) {
        // Bound the additional cashier futures so the shared MCP dispatcher retains a small stack frame.
        return Box::pin(workspace(reg, a, h, (channel, op), v, &input)).await;
    }
    match op {
        "currency" | "coupons" => {
            permit(&h, "orders.write")?;
            reg.carts().load(&h).await?;
            let revision = input.revision.ok_or(bad("Cart revision required"))?;
            if op == "currency" {
                reg.currency(
                    &h,
                    "currency.select",
                    &json!({"revision":revision,"currency":input.currency}),
                )
                .await
            } else {
                coupons::select(
                    reg.carts(),
                    a,
                    &h,
                    &json!({"revision":revision,"codes":input.codes}),
                )
                .await
            }
        }
        "payment" | "reconcile" => {
            let c = reg.carts().load(&h).await?;
            let order = c.data.order.as_ref().ok_or(bad("POS sale not completed"))?;
            let id = order["payment"]["attemptId"]
                .as_str()
                .ok_or(bad("Sale has no provider attempt"))?;
            commands::customer(a, &h, id).await?;
            if op == "reconcile" {
                permit(&h, "payments.manage")?;
                commands::enqueue(
                    a,
                    &h,
                    id,
                    "reconcile",
                    input
                        .request_key
                        .as_deref()
                        .ok_or(bad("Request key required"))?,
                    &json!({}),
                )
                .await?;
            }
            read_sale(a, &t, id, &c.id).await
        }
        "context" => {
            let mut v = register_context(reg, a, &h).await?;
            v["channel"] = json!(data);
            Ok(v)
        }
        "detail" => {
            let id = input.product_id.ok_or(bad("Product required"))?;
            let criteria = CatalogCriteria {
                after: input.after,
                limit: Some(40),
                ..Default::default()
            };
            reg.detail(&h, id, criteria).await
        }
        "create" | "cart" | "replace" | "select" | "accept" | "complete" => {
            Box::pin(cart_operation(reg, a, h, (&t, &data), op, input)).await
        }
        "held" | "sales" | "catalog" => {
            Box::pin(listing(reg, a, h, (&t, channel), op, input)).await
        }
        _ => Err(bad("Unknown POS operation")),
    }
}
/// The cashier's cart: opening, reading, changing, selecting, accepting and completing a sale.
async fn cart_operation(
    reg: &dyn Register,
    a: &State,
    mut h: RequestContext,
    (t, data): (&str, &Channel),
    op: &str,
    input: Input,
) -> Result<Value> {
    let admission = h.pos.clone().ok_or(bad("Cashier identity required"))?;
    let (actor, tender) = (admission.actor, admission.tender_minor);
    match op {
        "create" => {
            permit(&h, "orders.write")?;
            let mut c = new_cart_context(a, &h, "", "pos").await?;
            c.data.pos_actor = Some(actor);
            let selection = c.data.checkout.as_mut().unwrap();
            selection.country = data
                .pos
                .as_ref()
                .ok_or(bad("POS location required"))?
                .country
                .clone();
            selection.shipping_method_id = "pickup".into();
            selection.payment_method_id = "pos-cash".into();
            carts::set_data(a, &json!(c.data), t, &c.id).await?;
            let mut v = register_context(reg, a, &h).await?;
            v["cart"] = reg.carts().view(&c).await?;
            v["channel"] = json!(data);
            Ok(v)
        }
        "cart" => reg.carts().view(&reg.carts().load(&h).await?).await,
        "replace" => {
            permit(&h, "orders.write")?;
            let c = reg
                .carts()
                .replace(
                    &h,
                    input.items.ok_or(bad("POS items required"))?,
                    Some(input.revision.ok_or(bad("Cart revision required"))?),
                )
                .await?;
            reg.carts().view(&c).await
        }
        "select" => {
            permit(&h, "orders.write")?;
            reg.carts().load(&h).await?;
            let selected = input.checkout.ok_or(bad("Checkout selection required"))?;
            if selected.country != data.pos.as_ref().unwrap().country
                || selected.shipping_method_id != "pickup"
            {
                return Err(bad(
                    "POS checkout requires its location country and collection",
                ));
            }
            reg.select(&h, json!({"revision":input.revision,"checkout":selected}))
                .await
        }
        "accept" => {
            permit(&h, "orders.write")?;
            reg.carts().load(&h).await?;
            legal::invoke(a,&h,"legal.accept",&json!({"policyVersion":input.policy_version,"terms":input.terms,"digitalImmediate":input.digital_immediate})).await
        }
        "complete" => {
            permit(&h, "orders.write")?;
            if tender.is_some() {
                permit(&h, "payments.manage")?;
            }
            let c = reg.carts().load(&h).await?;
            if c.data.pos_actor.is_none() {
                return Err(bad("Not a POS cart"));
            }

            h.insert(
                "x-commerce-cart-revision",
                input
                    .revision
                    .ok_or(bad("Cart revision required"))?
                    .to_string()
                    .parse()
                    .unwrap(),
            );
            h.insert(
                "x-commerce-total-minor",
                input
                    .total_minor
                    .as_deref()
                    .ok_or(bad("POS reviewed total required"))?
                    .parse()
                    .map_err(|_| bad("Invalid POS reviewed total"))?,
            );
            Box::pin(
                reg.checkout(
                    &h,
                    input
                        .request_key
                        .as_deref()
                        .ok_or(bad("POS request key required"))?,
                ),
            )
            .await
        }
        _ => Err(bad("Unknown POS operation")),
    }
}
/// Held carts, recent sales and the catalogue page of a register.
async fn listing(
    reg: &dyn Register,
    a: &State,
    h: RequestContext,
    (t, channel): (&str, &str),
    op: &str,
    input: Input,
) -> Result<Value> {
    let actor = h
        .pos
        .as_ref()
        .map(|admission| admission.actor.clone())
        .ok_or(bad("Cashier identity required"))?;
    match op {
        "held" => {
            let rows = carts::held(a, t, channel, &actor).await?;
            Ok(
                json!({"elements":rows.iter().map(|r|json!({"token":r.token,"revision":r.revision,"items":r.items})).collect::<Vec<_>>() }),
            )
        }
        "sales" => {
            permit(&h, "orders.read")?;
            let rows = register::sales(a, t, channel).await?;
            Ok(
                json!({"elements":rows.iter().map(|r|json!({"id":r.id,"number":r.number,"money":r.money,"paymentState":r.payment,"pos":r.pos,"createdAt":r.created})).collect::<Vec<_>>() }),
            )
        }
        "catalog" => {
            let mut criteria = CatalogCriteria {
                search: input.query,
                after: input.after,
                limit: Some(40),
                ..Default::default()
            };
            if let Some(barcode) = input.barcode {
                if barcode.is_empty() || barcode.len() > 100 {
                    return Err(bad("Invalid barcode"));
                }
                let ids = register::barcode(a, t, &barcode).await?;
                if ids.len() != 1 {
                    return Err(bad("Barcode is missing or ambiguous in this shop"));
                }
                channels::admit_product(a, &h, &ids[0]).await?;
                criteria.product_ids = Some(ids);
            }
            reg.catalog(&h, criteria).await
        }
        _ => Err(bad("Unknown POS operation")),
    }
}
/// Cashier workspace commands: CRM lookup and attachment, native discounts and the immutable receipt projection.
async fn workspace(
    reg: &dyn Register,
    a: &State,
    h: RequestContext,
    (channel, op): (&str, &str),
    v: &Value,
    input: &Input,
) -> Result<Value> {
    match op {
        "customers" => {
            customer::find(
                a,
                &h,
                input.query.as_deref().unwrap_or(""),
                input.barcode.as_deref(),
            )
            .await
        }
        "customer" => {
            if !v.get("customerId").is_some_and(|id| {
                id.is_null()
                    || id
                        .as_str()
                        .is_some_and(|id| !id.is_empty() && id.len() <= 100)
            }) {
                return Err(bad("Customer ID required; null removes it"));
            }
            reg.carts().load(&h).await?;
            customer::select(
                reg.carts(),
                a,
                &h,
                input.customer_id.as_deref(),
                input.revision.ok_or(bad("Cart revision required"))?,
            )
            .await
        }
        "discount" => {
            reg.carts().load(&h).await?;
            discount::save(
                reg.carts(),
                a,
                &h,
                v.get("discount")
                    .ok_or(bad("Discount required; null removes it"))?,
                input.revision.ok_or(bad("Cart revision required"))?,
            )
            .await
        }
        "receipt" => {
            let snapshot = reg
                .receipt(
                    &h,
                    input.order_id.as_deref().ok_or(bad("Order required"))?,
                    input.receipt_id.as_deref().ok_or(bad("Receipt required"))?,
                )
                .await?;
            if snapshot["order"]["salesChannelId"] != channel
                || !snapshot["order"]["pos"].is_object()
            {
                return Err(Error(
                    StatusCode::NOT_FOUND,
                    "POS receipt unavailable".into(),
                ));
            }
            Ok(json!({"number":snapshot["number"],"lines":receipt_text::lines(&snapshot)}))
        }
        _ => Err(bad("Unknown cashier operation")),
    }
}

#[cfg(test)]
mod tests {
    use super::tender;

    #[test]
    fn cash_tender_is_a_bounded_minor_amount() {
        assert_eq!(tender(None).unwrap(), None);
        assert_eq!(tender(Some("1250")).unwrap(), Some(1250));
        for bad in ["", "-1", "1.5", "12345678901234"] {
            assert!(tender(Some(bad)).is_err(), "{bad}");
        }
    }
}
