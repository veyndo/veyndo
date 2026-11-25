//! One native company command contract for Store API, merchant API and MCP: field bounds, feature admission and the dispatch to the B2B owners; actor identity never comes from arguments.
use crate::domain::contract::permission;
use crate::ports::Procurement;
use crate::service::{
    approval, lists, onboarding, quick_order, quote_contract, quotes, requisition,
};
use crate::store::access::{access, catalog_guard, identity, require};
use crate::store::{carts as cart_rows, units};
use crate::store::{catalog, organization};
use crate::types::model::Context;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{lenient, view};
use veyndo_core::shop_scope::tenant;
use veyndo_core::store::history_context::customer_context;
use veyndo_core::{Error, RequestContext, Result, State, StatusCode, bad, conflict};
use veyndo_identity::{merchant, permit};
use veyndo_shop::Settings;
use veyndo_shop::service::read_cache;
use veyndo_shop::types::b2b::Config;

/// The admitted company command fields; anything else is refused before dispatch.
pub const FIELDS: [&str; 14] = [
    "id",
    "action",
    "unitId",
    "companyId",
    "parentId",
    "customerId",
    "productIds",
    "cartRevision",
    "lines",
    "message",
    "purchaseOrder",
    "revision",
    "state",
    "data",
];

/// A command carries at most the admitted fields and stays below 64 kB.
pub fn bounded(v: &Value) -> bool {
    v.as_object()
        .is_some_and(|o| o.len() <= 14 && o.keys().all(|k| FIELDS.contains(&k.as_str())))
        && v.to_string().len() <= 64_000
}

/// A typed feature edit delegates to the existing revisioned settings writer, audit and outbox.
async fn save_features(
    host: &dyn Procurement,
    h: &RequestContext,
    settings: &Settings,
    v: &Value,
) -> Result<Value> {
    permit(h, "settings.write")?;
    merchant(h)?;
    let mut data = json!(settings);
    let input: CommandInput = view(v);
    let config: Config =
        serde_json::from_value(input.data).map_err(|_| bad("Invalid B2B feature configuration"))?;
    data["b2b"] = json!(config);
    host.save(h, json!({"revision":input.revision,"data":data}))
        .await
}

/// The input of a procurement command: configuration, company, unit, purchase order and expected revision.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct CommandInput {
    data: Value,
    revision: Value,
    #[serde(deserialize_with = "lenient")]
    id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    unit_id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    purchase_order: Option<String>,
}
/// One company command after its merchant scope or the customer session, then the shop's B2B feature switch.
pub async fn invoke(
    host: &dyn Procurement,
    a: &State,
    h: &RequestContext,
    name: &str,
    v: &Value,
) -> Result<Value> {
    if !bounded(v) {
        return Err(bad("Invalid B2B command fields"));
    }
    if let Some(scope) = permission(name) {
        merchant(h)?;
        permit(h, scope)?;
    } else {
        host.customer(h).await?;
    }
    let (settings, revision) = read_cache::settings(a, &tenant(h)?, "default").await?;
    settings
        .b2b
        .admit(name.strip_prefix("b2b.").unwrap_or(""))?;
    match name
        .strip_prefix("b2b.")
        .ok_or(bad("Unknown company command"))?
    {
        "settings.save" => save_features(host, h, &settings, v).await,
        "configuration" => {
            let s = json!(settings);
            let methods = |key: &str| {
                json!(s[key].as_array().map(|v|v.iter().map(|m|json!({"id":m["id"],"name":m["name"],"translations":m["translations"],"active":m["active"]})).collect::<Vec<_>>()).unwrap_or_default())
            };
            Ok(
                json!({"features":s["b2b"],"revision":revision,"mainLocale":s["mainLocale"],"locales":s["locales"],"currencies":{"defaultCurrency":s["currencies"]["defaultCurrency"],"enabled":s["currencies"]["enabled"]},"shipping":methods("shipping"),"payments":methods("payments")}),
            )
        }
        "company.create" => onboarding::create(a, h, v).await,
        "companies" => {
            let mut value = organization::list(a, &tenant(h)?).await?;
            if let Some(rows) = value["companies"].as_array_mut() {
                host.companies(h, rows).await?;
            }
            Ok(value)
        }
        "company" => {
            permit(h, "customers.pii")?;
            let mut value = organization::detail(
                a,
                &tenant(h)?,
                &view::<CommandInput>(v).id.ok_or(bad("Company required"))?,
            )
            .await?;
            host.companies(h, std::slice::from_mut(&mut value)).await?;
            Ok(value)
        }
        op @ ("company.save" | "unit.save") => organization::save(a, h, op, v).await,
        "member.save" => organization::member(a, h, v).await,
        "catalog.save" => catalog::save(a, h, v).await,
        op @ ("lists" | "list.save" | "list.delete" | "list.apply") => {
            lists::invoke(a, (host.carts(), host.lists()), h, op, v).await
        }
        op @ ("quotes" | "quotes.merchant" | "quote.request" | "quote.offer" | "quote.message"
        | "quote.reject" | "quote.get" | "quote.current" | "quote.transition") => {
            quotes::invoke(a, (host.carts(), host.lifecycle()), h, op, v).await
        }
        "quote.accept" => quote_contract::select(host.carts(), a, h, v, true).await,
        "quote.release" => quote_contract::select(host.carts(), a, h, v, false).await,
        "requisition.import" => {
            let t = merchant(h)?;
            permit(h, "b2b.manage")?;
            requisition::import(a, h, &t, &settings, v).await
        }
        "unit.orders" => unit_orders(host, a, h, v).await,
        "quick-order" => quick_order::apply(host.carts(), a, h, v).await,
        "contexts" => contexts(a, h, &settings).await,
        "select" => select(host, a, h, v).await,
        "approval.request" => approval::request(host.carts(), a, h, v).await,
        "approvals" => approval::inbox(a, h, v).await,
        "approval.decide" => approval::decide(a, h, v).await,
        _ => Err(bad("Unknown B2B command")),
    }
}
async fn contexts(a: &State, h: &RequestContext, settings: &Settings) -> Result<Value> {
    let t = tenant(h)?;
    let mut tx = a.db.begin().await?;
    let customer = identity(&mut tx, h).await?;
    customer_context(&mut tx, &customer).await?;
    let rows = units::memberships(&mut tx, &t, &customer).await?;
    Ok(
        json!({"mainLocale":settings.main_locale,"locales":settings.locales,"elements":rows.into_iter().map(|r|json!({"id":r.id,"companyId":r.company_id,"data":r.data,"company":r.company,"member":r.member})).collect::<Vec<_>>()}),
    )
}
async fn select(host: &dyn Procurement, a: &State, h: &RequestContext, v: &Value) -> Result<Value> {
    let mut c = host.carts().load(h).await?;
    if c.data.pos_actor.is_some() {
        return Err(bad("POS carts cannot select procurement units"));
    }
    let input: CommandInput = view(v);
    let unit = input.unit_id.as_deref().ok_or(bad("Unit required"))?;
    let po = input.purchase_order.as_deref().unwrap_or("");
    if po.len() > 140 || po.chars().any(char::is_control) {
        return Err(bad("Invalid purchase order reference"));
    }
    let mut tx = a.db.begin().await?;
    c = cart_rows::lock_by_token(&mut tx, &c.tenant, &c.token)
        .await?
        .ok_or(sqlx::Error::RowNotFound)?;
    host.cashier(h, &c)?;
    if c.status != "open" || input.revision.as_i64() != Some(c.revision) {
        return Err(conflict("Cart changed or is terminal"));
    }
    if c.data.b2b.as_ref().is_some_and(|b| b.quote_id.is_some()) {
        return Err(conflict(
            "Release the accepted quote before changing purchasing units",
        ));
    }
    let customer = identity(&mut tx, h).await?;
    customer_context(&mut tx, &customer).await?;
    if c.data.customer_id.as_deref() != Some(&customer) {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Sign in to your own cart before selecting a company".into(),
        ));
    }
    let policy = access(&mut tx, &c.tenant, unit, &customer).await?;
    require(&policy, "buy")?;
    c.data.b2b = Some(Context {
        unit_id: unit.into(),
        customer_id: customer,
        purchase_order: po.into(),
        quote_id: None,
    });
    catalog_guard(&mut tx, &c, &policy).await?;
    let (config, _) = host
        .locked(&mut tx, &c.tenant, &c.data.sales_channel)
        .await?;
    config.currencies.selected(&policy.unit.currency)?;
    // Keep customer-group pricing under the native configured group owner.
    if !config.is_business("business") {
        return Err(bad(
            "Configure an enabled business customer group before company purchases",
        ));
    }
    c.data.group = "business".into();
    c.data.currency = policy.unit.currency;
    c.data.company = Some(
        policy
            .company
            .name
            .get(&config.main_locale)
            .or_else(|| {
                policy
                    .company
                    .name
                    .get(config.main_locale.split('-').next().unwrap_or(""))
            })
            .cloned()
            .unwrap_or_default(),
    );
    c.revision += 1;
    cart_rows::save(&mut tx, &c.tenant, &c.id, &json!(c.data), c.revision).await?;
    tx.commit().await?;
    host.carts().view(&c).await
}

/// The orders of one purchasing unit for an employee with the explicit `companyOrders` grant, with the receipt list of a single order.
pub async fn unit_orders(
    host: &dyn Procurement,
    a: &State,
    h: &RequestContext,
    v: &Value,
) -> Result<Value> {
    let t = tenant(h)?;
    let input: CommandInput = view(v);
    let unit = input.unit_id.as_deref().ok_or(bad("Unit required"))?;
    let labels = host.labels(&t).await?;
    let mut tx = a.db.begin().await?;
    let customer = identity(&mut tx, h).await?;
    let policy = access(&mut tx, &t, unit, &customer).await?;
    require(&policy, "companyOrders")?;
    let id = input.id.as_deref();
    let rows = units::orders(&mut tx, &t, unit, id).await?;
    if id.is_some() && rows.is_empty() {
        return Err(Error(
            StatusCode::NOT_FOUND,
            "Order not found in this unit".into(),
        ));
    }
    let elements: Vec<Value> = rows
        .iter()
        .map(|r| {
            let mut p = host.order(&r.data, &r.created);
            p["b2b"] = r.b2b.clone().unwrap_or(Value::Null);
            p
        })
        .collect();
    let receipts = if let Some(id) = id {
        units::receipts(&mut tx, &t, id).await?
    } else {
        vec![]
    };
    let elements:Vec<Value>=elements.into_iter().map(|mut p|{p["stateLabels"]=labels.clone();if let Some(id)=id {p["receipts"]=json!(receipts.iter().map(|r|json!({"id":r.id,"kind":r.kind,"number":r.number,"locale":r.locale,"createdAt":r.created,"pdfPath":format!("/store-api/b2b/units/{unit}/orders/{id}/receipts/{}/pdf",r.id)})).collect::<Vec<_>>());}p}).collect();
    Ok(json!({"elements":elements}))
}

#[cfg(test)]
mod tests {
    use super::bounded;
    use serde_json::json;

    #[test]
    fn commands_admit_only_bounded_known_fields() {
        assert!(bounded(&json!({"unitId":"u","revision":1})));
        assert!(!bounded(&json!({"unitId":"u","email":"x"})));
        assert!(!bounded(&json!("unit")));
        assert!(!bounded(&json!({"message":"x".repeat(64_001)})));
    }
}
