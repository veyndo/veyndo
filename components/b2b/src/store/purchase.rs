//! Serializes unit spending and binds an approved purchase to the authoritative native checkout quote.
use crate::store::access::{access, catalog_guard, guard_conn, require};
use crate::types::model::{Access, Context};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use veyndo_core::document::{lenient, view};
use veyndo_core::{Error, RequestContext, Result, State, StatusCode, bad, conflict, hash};
use veyndo_kernel::verified_kernel;
use veyndo_shop::types::cart::StoredCart;
pub struct Purchase {
    pub context: Context,
    pub company_id: String,
    pub currency: String,
    pub period: String,
    pub total: i64,
    pub approval: Option<String>,
}
pub fn needs_approval(policy: &Access, total: u64) -> bool {
    [
        policy.unit.approval_above_minor,
        policy.member.approval_above_minor,
    ]
    .into_iter()
    .flatten()
    .any(|limit| total > limit)
}
pub fn fingerprint(c: &StoredCart, q: &Value, version: &str) -> String {
    let q: FingerprintedQuote = view(q);
    let lines = q.line_items.map(|lines| {
        lines
            .iter()
            .map(view::<FingerprintedLine>)
            .map(|line| json!({"id":line.id,"quantity":line.quantity,"price":line.price}))
            .collect::<Vec<_>>()
    });
    hash(&json!({"cart":c.id,"revision":c.revision,"context":c.data.b2b,"items":c.data.items,"checkout":c.data.checkout,"currency":c.data.currency,"coupons":c.data.coupons,"appConfigurations":c.data.app_configurations,"buyer":c.data.buyer,"policy":version,"price":json!({"total":q.price.total_price,"net":q.price.net_price,"tax":q.price.tax,"currency":q.price.currency,"scale":q.price.currency_scale}),"lineItems":lines,"shipping":q.shipping_method,"payment":q.payment_method,"currencyContext":q.currency_context}).to_string())
}

/// The quote values a company purchase approval is bound to.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct FingerprintedQuote {
    #[serde(deserialize_with = "lenient")]
    price: FingerprintedPrice,
    #[serde(deserialize_with = "lenient")]
    line_items: Option<Vec<Value>>,
    shipping_method: Value,
    payment_method: Value,
    currency_context: Value,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct FingerprintedPrice {
    total_price: Value,
    net_price: Value,
    tax: Value,
    currency: Value,
    currency_scale: Value,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct FingerprintedLine {
    id: Value,
    quantity: Value,
    price: Value,
}
pub async fn usage(
    conn: &mut sqlx::PgConnection,
    c: &StoredCart,
    policy: &Access,
) -> Result<(String, u64)> {
    let context = c.data.b2b.as_ref().ok_or(bad("Company context required"))?;
    let period: String =
        sqlx::query_scalar("SELECT date_trunc($1,now() AT TIME ZONE 'UTC')::date::text")
            .bind(&policy.unit.budget_period)
            .fetch_one(&mut *conn)
            .await?;
    let spent:i64=sqlx::query_scalar("SELECT coalesce(sum(amount_minor),0)::bigint FROM b2b_budget_allocations WHERE tenant=$1 AND unit_id=$2 AND period_start=$3::date AND currency=$4 AND NOT released").bind(&c.tenant).bind(&context.unit_id).bind(&period).bind(&policy.unit.currency).fetch_one(conn).await?;
    Ok((
        period,
        u64::try_from(spent).map_err(|_| bad("Invalid company budget usage"))?,
    ))
}
pub async fn check_locked(
    conn: &mut sqlx::PgConnection,
    h: &RequestContext,
    c: &StoredCart,
    q: &Value,
    total: u64,
) -> Result<Option<Purchase>> {
    let Some(context) = &c.data.b2b else {
        return Ok(None);
    };
    // All allocation/period policy writers hold this exact unit lock. Parallel orders cannot overspend.
    sqlx::query("SELECT id FROM b2b_units WHERE tenant=$1 AND id=$2 FOR UPDATE")
        .bind(&c.tenant)
        .bind(&context.unit_id)
        .fetch_one(&mut *conn)
        .await?;
    let policy = guard_conn(conn, h, c)
        .await?
        .ok_or(bad("Company context required"))?;
    require(&policy, "buy")?;
    catalog_guard(conn, c, &policy).await?;
    let (period, spent) = usage(conn, c, &policy).await?;
    if q["price"]["currency"] != policy.unit.currency {
        return Err(conflict(
            "Company budget currency differs; select the unit currency",
        ));
    }
    if let Some(limit) = policy.unit.budget_minor
        && !verified_kernel::b2b_budget_admissible(limit, spent, total)
    {
        return Err(conflict(
            "Company unit budget exceeded; ask your purchasing administrator",
        ));
    }
    let selection = veyndo_shop::types::cart::selection(&c.data);
    if (!policy.unit.allowed_shipping.is_empty()
        && !policy
            .unit
            .allowed_shipping
            .contains(&selection.shipping_method_id))
        || (!policy.unit.allowed_payments.is_empty()
            && !policy
                .unit
                .allowed_payments
                .contains(&selection.payment_method_id))
    {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Shipping or payment method is restricted by the purchasing unit".into(),
        ));
    }
    if policy.unit.require_purchase_order && context.purchase_order.trim().is_empty() {
        return Err(bad("Company purchase order reference required"));
    }
    let approval = if needs_approval(&policy, total) {
        let digest = fingerprint(c, q, &policy.version);
        let r=sqlx::query("SELECT id,data FROM b2b_approvals WHERE tenant=$1 AND cart_id=$2 AND fingerprint=$3 AND state='approved' FOR UPDATE").bind(&c.tenant).bind(&c.id).bind(digest).fetch_optional(&mut *conn).await?.ok_or(conflict("Purchase requires approval for this current cart and company policy"))?;
        let data: Value = r.get("data");
        let approver = data["approverId"]
            .as_str()
            .ok_or(bad("Missing purchase approver"))?;
        let approver_policy = access(conn, &c.tenant, &context.unit_id, approver).await?;
        if !verified_kernel::b2b_approval_admissible(
            approver != context.customer_id,
            approver_policy.member.allows("approve"),
            true,
            true,
        ) {
            return Err(conflict(
                "Purchase approver no longer has current authority",
            ));
        }
        Some(r.get("id"))
    } else {
        None
    };
    Ok(Some(Purchase {
        context: context.clone(),
        company_id: policy.company_id,
        currency: policy.unit.currency,
        period,
        total: i64::try_from(total).map_err(|_| bad("Company amount overflow"))?,
        approval,
    }))
}
pub async fn record(
    conn: &mut sqlx::PgConnection,
    c: &StoredCart,
    id: &str,
    purchase: Option<&Purchase>,
) -> Result<()> {
    let Some(p) = purchase else { return Ok(()) };
    sqlx::query("INSERT INTO b2b_budget_allocations(tenant,order_id,unit_id,period_start,currency,amount_minor) VALUES($1,$2,$3,$4::date,$5,$6)").bind(&c.tenant).bind(id).bind(&p.context.unit_id).bind(&p.period).bind(&p.currency).bind(p.total).execute(&mut *conn).await?;
    if let Some(approval) = &p.approval {
        sqlx::query("UPDATE b2b_approvals SET state='consumed',revision=revision+1,data=data||jsonb_build_object('orderId',$3::text) WHERE tenant=$1 AND id=$2 AND state='approved'").bind(&c.tenant).bind(approval).bind(id).execute(&mut *conn).await?;
    }
    crate::store::organization::event(conn,&c.tenant,"b2b.purchase_completed",json!({"orderId":id,"companyId":p.company_id,"unitId":p.context.unit_id,"currency":p.currency,"totalMinor":p.total,"approvalId":p.approval})).await
}
pub async fn preview(a: &State, c: &StoredCart, q: &mut Value) -> Result<()> {
    let Some(context) = &c.data.b2b else {
        return Ok(());
    };
    let mut tx = a.db.begin().await?;
    let policy = access(&mut tx, &c.tenant, &context.unit_id, &context.customer_id).await?;
    catalog_guard(&mut tx, c, &policy).await?;
    let (period, spent) = usage(&mut tx, c, &policy).await?;
    if q["price"]["currency"] != policy.unit.currency {
        return Err(conflict(
            "Select the purchasing unit currency before reviewing its budget",
        ));
    }
    let total = veyndo_pricing::service::conversion::amount(
        q["price"]["totalPrice"]
            .as_f64()
            .ok_or(bad("Invalid company quote"))?,
        &policy.unit.currency,
        veyndo_pricing::types::currency::scale(&policy.unit.currency)
            .ok_or(bad("Unknown unit currency"))?,
    )?
    .minor() as u64;
    let approval = sqlx::query(
        "SELECT id,state FROM b2b_approvals WHERE tenant=$1 AND cart_id=$2 AND fingerprint=$3",
    )
    .bind(&c.tenant)
    .bind(&c.id)
    .bind(fingerprint(c, q, &policy.version))
    .fetch_optional(&mut *tx)
    .await?;
    q["b2b"] = json!({"companyId":policy.company_id,"company":policy.company.name,"unitId":context.unit_id,"unit":policy.unit.name,"purchaseOrder":context.purchase_order,"purchaseOrderRequired":policy.unit.require_purchase_order,"quoteId":context.quote_id,"costCenter":policy.unit.cost_center,"currency":policy.unit.currency,"budgetPeriod":period,"budgetMinor":policy.unit.budget_minor,"spentMinor":spent,"quotesAllowed":policy.member.allows("quotes"),"approvalRequired":needs_approval(&policy,total),"approval":approval.map(|r|json!({"id":r.get::<String,_>("id"),"state":r.get::<String,_>("state")})),"budgetAvailable":policy.unit.budget_minor.is_none_or(|limit|verified_kernel::b2b_budget_admissible(limit,spent,total))});
    Ok(())
}

/// Called only after the shared order state machine admits an actual cancellation.
pub async fn release_budget(conn: &mut sqlx::PgConnection, t: &str, order: &str) -> Result<()> {
    // The same unit row serializes allocation and release across concurrent orders.
    sqlx::query("SELECT u.id FROM b2b_units u JOIN b2b_budget_allocations b ON b.tenant=u.tenant AND b.unit_id=u.id WHERE b.tenant=$1 AND b.order_id=$2 FOR UPDATE OF u")
        .bind(t).bind(order).fetch_optional(&mut *conn).await?;
    sqlx::query("UPDATE b2b_budget_allocations SET released=true WHERE tenant=$1 AND order_id=$2 AND NOT released")
        .bind(t).bind(order).execute(conn).await?;
    Ok(())
}

/// Checkout takes the unit write lock before product locks, avoiding shared-lock upgrades.
pub async fn lock_unit(conn: &mut sqlx::PgConnection, c: &StoredCart) -> Result<()> {
    if let Some(context) = &c.data.b2b {
        sqlx::query("SELECT id FROM b2b_units WHERE tenant=$1 AND id=$2 FOR UPDATE")
            .bind(&c.tenant)
            .bind(&context.unit_id)
            .fetch_one(conn)
            .await?;
    }
    Ok(())
}
