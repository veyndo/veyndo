//! Generic authorized external order intake reuses native stock, customer snapshots, documents, history and outbox.
use crate::store::external_orders as stored;
use crate::store::{locks, packages};
use crate::types::events::ExternalOrderPlaced;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use veyndo_core::document::{lenient, view};

/// The imported order the cart that backs it records.
#[derive(serde::Serialize)]
struct CartOrder<'a> {
    order: &'a Value,
}
use veyndo_core::{
    RequestContext, Result, State, bad, conflict, hash, store::history_context, uid,
};
use veyndo_identity::{merchant, permit};
use veyndo_inventory::InventoryLines;
use veyndo_inventory::store::reservations;
use veyndo_kernel::verified_kernel;
use veyndo_pricing::external_order::{PaidOrder as Input, SourceAmount};
use veyndo_shipping::domain::geography;
use veyndo_shop::service::read_cache::conditions;
use veyndo_shop::types::cart::{Cart, StoredCart};
use veyndo_shop::types::settings::{Address, CheckoutSelection, Settings};

/// The legacy price projection of exact imported minor-unit amounts.
pub fn source_price(amount: &SourceAmount, scale: u8, quantity: u32) -> Value {
    let factor = 10_i64.pow(scale.into()) as f64;
    let gross = amount.gross_minor as f64 / factor;
    json!({"unitPrice":gross/f64::from(quantity),"totalPrice":gross,
          "totalMinor":amount.gross_minor.to_string(),"taxMinor":amount.tax_minor.to_string(),
          "calculatedTaxes":[{"tax":amount.tax_minor as f64/factor,"taxRate":f64::from(amount.tax_rate_basis_points)/100.,"price":gross}]})
}

fn validate_address(address: &Address, settings: &Settings) -> Result<()> {
    geography::validate_address(
        &address.country,
        &address.country_state_id,
        &settings.country_definitions,
    )
}
/// The app package must still be the admitted one; an order imported before answers with its stored snapshot.
async fn replayed(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    t: &str,
    (app, digest): (&str, &str),
    key: &str,
    fingerprint: &str,
) -> Result<Option<Value>> {
    // Lock the native package to preserve callback authorization across uninstall/upgrade races.
    let current = packages::active_digest_for_share(tx, t, app).await?;
    if current.as_deref() != Some(digest) {
        return Err(conflict("App package changed during import"));
    }
    if let Some(r) = stored::imported(tx, t, key).await? {
        if r.fingerprint != fingerprint {
            return Err(conflict(
                "External order already imported with another snapshot",
            ));
        }
        return Ok(Some(json!({"order":r.data,"replayed":true})));
    }
    Ok(None)
}
/// The sales channel fields an import admits the channel by, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default)]
struct ImportChannel {
    #[serde(deserialize_with = "lenient")]
    kind: Option<String>,
    #[serde(deserialize_with = "lenient")]
    active: Option<bool>,
    #[serde(deserialize_with = "lenient")]
    visibility: Option<String>,
    #[serde(deserialize_with = "lenient")]
    locales: Option<Vec<Value>>,
}

/// The JSON fields `order_lines` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct OrderLinesInput {
    product_ids: Value,
}

/// Locks the products of the order and their parents and builds the order lines from the external snapshot.
async fn order_lines(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    t: &str,
    input: &Input,
    ids: &[String],
    channel: &Value,
    main_locale: &str,
    scale: u8,
) -> Result<Vec<Value>> {
    let args: OrderLinesInput = view(channel);
    let mut locks = stored::parents(tx, t, ids).await?;
    locks.extend(ids.to_vec());
    locks.sort();
    locks.dedup();
    let rows = stored::lock_products(tx, t, &locks).await?;
    if rows.iter().any(|r| !r.active) {
        return Err(bad("Inactive imported product"));
    }
    let hidden = stored::hidden_in_channel(tx, t, &input.sales_channel_id, &locks).await?;
    if hidden {
        return Err(bad("Imported product is hidden in this channel"));
    }
    let selected = args
        .product_ids
        .as_array()
        .ok_or(bad("Invalid channel selection"))?;
    let chain = vec![
        input.locale.clone(),
        input
            .locale
            .split('-')
            .next()
            .unwrap_or(&input.locale)
            .to_owned(),
        main_locale.to_owned(),
        main_locale
            .split('-')
            .next()
            .unwrap_or(main_locale)
            .to_owned(),
    ];
    // Products and parents are already locked. Resolve all admitted lines in one
    // read, retaining the external snapshot's locale-before-variant precedence.
    let mut names: HashMap<String, String> = stored::labels(tx, t, ids, &chain)
        .await?
        .into_iter()
        .map(|l| (l.id, l.name))
        .collect();
    let products: HashMap<&str, _> = rows.iter().map(|r| (r.id.as_str(), r)).collect();
    let mut lines = Vec::new();
    for l in &input.lines {
        let r = products
            .get(l.product_id.as_str())
            .ok_or(bad("Imported product does not belong to this shop"))?;
        let parent = &r.parent_id;
        if !selected.is_empty()
            && !selected
                .iter()
                .any(|s| s == &l.product_id || parent.as_ref().is_some_and(|p| s == p))
        {
            return Err(bad("Imported product is outside channel catalog"));
        }
        if !u64::try_from(r.stock)
            .is_ok_and(|stock| verified_kernel::stock_admissible(stock, u64::from(l.quantity)))
        {
            return Err(conflict(
                "Insufficient stock for imported order; review required",
            ));
        }
        let label = names
            .remove(&l.product_id)
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| r.name.clone());
        lines.push(json!({"id":l.product_id,"referencedId":l.product_id,"label":label,"quantity":l.quantity,"productNumber":r.product_number,"price":source_price(&l.amount,scale,l.quantity)}));
    }
    Ok(lines)
}
/// Imports one paid order an admitted app reports; the same snapshot replays the stored order.
pub async fn import_external(
    a: &State,
    h: &RequestContext,
    app: &str,
    digest: &str,
    v: Value,
) -> Result<Value> {
    permit(h, "orders.write")?;
    permit(h, "payments.manage")?;
    permit(h, "customers.pii")?;
    let t = merchant(h)?;
    let mut input: Input =
        serde_json::from_value(v).map_err(|_| bad("Invalid external order snapshot"))?;
    input.validate()?;
    if veyndo_core::store::environments::parent(a, &t)
        .await?
        .is_some()
    {
        return Err(bad("External orders are disabled in sandboxes"));
    }
    let key = format!(
        "external:{}",
        hash(&format!(
            "{app}:{}:{}",
            input.sales_channel_id, input.external_id
        ))
    );
    let fingerprint = hash(&json!(input).to_string());
    let mut tx = a.db.begin().await?;
    history_context::context(&mut tx, h, "external_order").await?;
    locks::advisory(&mut tx, &format!("{t}:{key}"), 0).await?;
    if let Some(replay) = replayed(&mut tx, &t, (app, digest), &key, &fingerprint).await? {
        return Ok(replay);
    }
    let channel: Value = stored::lock_channel(&mut tx, &t, &input.sales_channel_id)
        .await?
        .ok_or(bad("Unknown external sales channel"))?;
    let admitted: ImportChannel = view(&channel);
    if admitted.kind.as_deref() != Some("marketplace")
        || admitted.active != Some(true)
        || admitted.visibility.as_deref() == Some("private")
    {
        return Err(conflict("External sales channel must be active and public"));
    }
    let (settings, _) = veyndo_shop::store::settings::scoped_locked(
        &mut tx,
        &t,
        &input.sales_channel_id,
        &conditions,
    )
    .await?;
    let currency = settings.currencies.selected(&input.currency)?.clone();
    if !admitted
        .locales
        .as_ref()
        .is_some_and(|ls| ls.iter().any(|l| l == &input.locale))
    {
        return Err(bad("External order language is not enabled in channel"));
    }
    validate_address(&input.billing_address, &settings)?;
    validate_address(&input.shipping_address, &settings)?;
    let ids = input
        .lines
        .iter()
        .map(|l| l.product_id.clone())
        .collect::<Vec<_>>();
    let lines = order_lines(
        &mut tx,
        &t,
        &input,
        &ids,
        &channel,
        &settings.main_locale,
        currency.scale,
    )
    .await?;
    let factor = 10_i64.pow(currency.scale.into()) as f64;
    let tax =
        input.lines.iter().map(|l| l.amount.tax_minor).sum::<i64>() + input.shipping.tax_minor;
    let totals = json!({"currency":currency.code,"currencyScale":currency.scale,"totalMinor":input.total_minor.to_string(),"totalPrice":input.total_minor as f64/factor,"netPrice":(input.total_minor-tax) as f64/factor,"taxStatus":"gross","tax":tax as f64/factor,"taxMinor":tax.to_string()});
    let method = json!({"id":"external-marketplace","mode":"external_recorded","name":{"en":"Marketplace payment","de":"Marktplatz-Zahlung","fr":"Paiement marketplace","es":"Pago marketplace"}});
    let delivery = json!({"id":uid(),"state":"open","shippingMethodId":"external-marketplace","trackingCodes":[],"shippingCosts":source_price(&input.shipping,currency.scale,1)});
    let currency_context = json!({"code":currency.code,"scale":currency.scale,"factor":currency.rate,"strategy":"external_snapshot"});
    let quote = json!({"lineItems":lines,"price":totals,"shippingCosts":source_price(&input.shipping,currency.scale,1),"paymentMethod":method,"deliveries":[delivery],"currencyContext":currency_context});
    let data:Cart=serde_json::from_value(json!({"items":ids.iter().zip(&input.lines).map(|(id,l)|json!({"id":id,"quantity":l.quantity})).collect::<Vec<_>>(),"group":"retail","email":input.email,"company":null,"session":key,"buyer":null,"order":null,"currency":currency.code,"sales_channel":input.sales_channel_id,"locale":input.locale,"channel":"marketplace"})).map_err(|_|bad("Invalid external cart"))?;
    let c = StoredCart {
        id: uid(),
        tenant: t.clone(),
        token: uid(),
        data,
        revision: 1,
        status: "completed".into(),
    };
    let selection:CheckoutSelection=serde_json::from_value(json!({"country":input.shipping_address.country,"address":input.shipping_address,"billingAddress":input.billing_address,"shippingMethodId":"external-marketplace","paymentMethodId":"external-marketplace"})).map_err(|_|bad("Invalid source addresses"))?;
    let customer =
        veyndo_checkout::store::customer_snapshot::snapshot(&mut tx, &c, &selection).await?;
    let id = uid();
    let prefix = veyndo_orders::store::company::order_prefix(&mut tx, &t).await?;
    let number = veyndo_orders::domain::company_model::order_number(&prefix, &id);
    let mut order = json!({"id":id,"orderNumber":number,"cart":quote,"state":"placed","revision":1,"channel":"marketplace","customerGroup":"retail","deliveries":[delivery],"payment":{"method":method,"provider":"external_marketplace","state":"captured","realMoneyCharged":false,"externallyPaid":true},"money":{"minor":input.total_minor,"currency":{"code":currency.code,"scale":currency.scale}},"external":{"appId":app,"orderId":input.external_id},"currencyContext":currency_context});
    order
        .as_object_mut()
        .unwrap()
        .extend(customer.as_object().unwrap().clone());
    crate::store::native_lifecycle::bind_order(&mut tx, &t, &mut order).await?;
    veyndo_orders::domain::order_fields::order_fields(&mut order);
    let mut cart = json!(c.data);
    veyndo_core::document::merge(&mut cart, CartOrder { order: &order })?;
    stored::insert_cart(&mut tx, &c.id, &t, &c.token, &cart).await?;
    stored::insert_order(
        &mut tx,
        stored::NewOrder {
            id: &id,
            tenant: &t,
            cart: &c.id,
            key: &key,
            fingerprint: &fingerprint,
            data: &order,
        },
    )
    .await?;
    let inventory = InventoryLines::new(
        input
            .lines
            .iter()
            .map(|l| (l.product_id.as_str(), l.quantity)),
    )?;
    reservations::allocate(&mut tx, &t, &inventory, &id).await?;
    reservations::consume(&mut tx, &t, &inventory).await?;
    veyndo_catalog::store::order_downloads::grant(&mut tx, &t, &ids, &id).await?;
    let placed = ExternalOrderPlaced {
        order_id: &id,
        total_minor: input.total_minor,
        sales_channel_id: &input.sales_channel_id,
        app_id: app,
    };
    veyndo_core::events::emit(&mut *tx, &t, &placed).await?;
    tx.commit().await?;
    // Customer/order reads must never expose the private completed-cart token.
    Ok(json!({"order":order,"replayed":false}))
}
