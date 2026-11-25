//! Typed pure commerce hooks share the compiled sandbox cache and receive explicit immutable snapshots, never SQL or credentials.
use crate::domain::approval;
use crate::service::configuration_runtime::prepare_source;
use crate::store::commerce_hooks::rows;
use crate::store::records;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;
use veyndo_app_model::manifest::Manifest;
use veyndo_apps::component_runtime::{
    self as wit, AppRecord, CartItem, CartSnapshot, Money, ProductSnapshot, Snapshot,
};
use veyndo_apps::sandbox::Sandbox;
use veyndo_checkout::service::app_adjustments::apply as app_adjustment;
use veyndo_checkout::service::delivery::enrich;
use veyndo_core::document::{lenient, view};
use veyndo_core::{Result, State, bad, conflict, hash};
use veyndo_pricing::catalog::Product;
use veyndo_pricing::service::conversion as currencies;
use veyndo_shop::Settings;
use veyndo_shop::types::cart::StoredCart;

/// The compiled hook modules of a shop's active packages, keyed by their source digest.
pub struct Prepared {
    /// The sandboxes by source digest.
    pub modules: HashMap<String, Arc<Sandbox>>,
}
/// Compiles the hook modules before the checkout takes its locks.
pub async fn prepare(a: &State, t: &str) -> Result<Prepared> {
    let mut conn = a.db.begin().await?;
    let ms = rows(&mut conn, t).await?;
    conn.rollback().await?;
    let mut modules = HashMap::new();
    for row in ms {
        let m: Manifest = serde_json::from_value(row).map_err(|_| bad("Invalid hook package"))?;
        let c = m.commerce_hooks.unwrap();
        modules.insert(hash(&c.source), prepare_source(c.source).await?);
    }
    Ok(Prepared { modules })
}
/// An amount of the quote in exact minor units.
fn money(amount: f64, code: &str, scale: u8) -> Result<Money> {
    Ok(Money {
        minor: currencies::amount(amount, code, scale)?.minor(),
        currency: code.into(),
        scale,
    })
}
/// The quote an app commerce hook snapshot is built from.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct HookQuote {
    #[serde(deserialize_with = "lenient")]
    price: HookPrice,
    #[serde(deserialize_with = "lenient")]
    line_items: Option<Vec<Value>>,
}

/// The currency, scale and position total of the quote.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct HookPrice {
    #[serde(deserialize_with = "lenient")]
    currency: Option<String>,
    #[serde(deserialize_with = "lenient")]
    currency_scale: Option<u64>,
    position_price: Value,
}
/// The immutable cart, product and record snapshot a hook sees.
fn snapshot(
    c: &StoredCart,
    ps: &[Product],
    q: &Value,
    records: HashMap<(String, String), AppRecord>,
) -> Result<Snapshot> {
    let quote: HookQuote = view(q);
    let code = quote
        .price
        .currency
        .as_deref()
        .ok_or(bad("Quote currency missing"))?;
    let scale = quote
        .price
        .currency_scale
        .ok_or(bad("Quote scale missing"))? as u8;
    let mut products = HashMap::new();
    let mut items = vec![];
    for line in quote
        .line_items
        .as_ref()
        .ok_or(bad("Quote items missing"))?
    {
        let id = line["id"].as_str().ok_or(bad("Quote item ID missing"))?;
        let p = ps
            .iter()
            .find(|p| p.id == id)
            .ok_or(bad("Product snapshot missing"))?;
        let price = money(
            line["price"]["unitPrice"]
                .as_f64()
                .ok_or(bad("Quote unit price missing"))?,
            code,
            scale,
        )?;
        products.insert(
            id.into(),
            ProductSnapshot {
                id: id.into(),
                stock: u32::try_from(p.stock).map_err(|_| bad("Invalid stock"))?,
                price: price.clone(),
            },
        );
        items.push(CartItem {
            id: id.into(),
            quantity: c
                .data
                .items
                .iter()
                .find(|i| i.id == id)
                .ok_or(bad("Cart item missing"))?
                .quantity,
            price,
        });
    }
    Ok(Snapshot {
        cart: CartSnapshot {
            items,
            subtotal: money(
                quote
                    .price
                    .position_price
                    .as_f64()
                    .ok_or(bad("Quote subtotal missing"))?,
                code,
                scale,
            )?,
        },
        products,
        records,
    })
}
/// Runs the price, discount, shipping and validation hooks in fixed order on the quote and records their outcomes.
pub async fn apply(
    conn: &mut sqlx::PgConnection,
    c: &StoredCart,
    ps: &[Product],
    mut q: Value,
    s: &Settings,
    revision: i64,
    prepared: &Prepared,
) -> Result<Value> {
    let ms = rows(conn, &c.tenant).await?;
    let mut audit = vec![];
    for row in ms {
        let m: Manifest = serde_json::from_value(row).map_err(|_| bad("Invalid hook package"))?;
        let contract = m
            .commerce_hooks
            .as_ref()
            .ok_or(bad("Hook contract missing"))?;
        let guest = prepared
            .modules
            .get(&hash(&contract.source))
            .cloned()
            .ok_or(conflict("Commerce hooks changed; review checkout again"))?;
        let mut records = HashMap::new();
        for r in &contract.records {
            let value = records::get(conn, &c.tenant, &m.id, &r.entity, &r.id, false)
                .await?
                .ok_or(conflict("Hook record missing"))?;
            let revision = value["revision"]
                .as_u64()
                .ok_or(bad("Hook record revision missing"))?;
            let mut fields = value;
            fields.as_object_mut().unwrap().remove("tenant");
            records.insert(
                (r.entity.clone(), r.id.clone()),
                AppRecord {
                    id: r.id.clone(),
                    revision,
                    fields_json: fields.to_string(),
                },
            );
        }
        let mut record_revisions: Vec<_> = records
            .iter()
            .map(|((entity, id), r)| json!({"entity":entity,"id":id,"revision":r.revision}))
            .collect();
        record_revisions.sort_by_key(|r| {
            (
                r["entity"].as_str().unwrap().to_owned(),
                r["id"].as_str().unwrap().to_owned(),
            )
        });
        // Fixed phase order, independent of merchant JSON ordering. Each phase sees the preceding result.
        for kind in ["price", "discount", "shipping", "validation"] {
            if !contract.hooks.iter().any(|h| h == kind) {
                continue;
            }
            let snap = snapshot(c, ps, &q, records.clone())?;
            let hook = match kind {
                "price" => wit::Kind::Price,
                "discount" => wit::Kind::Discount,
                "shipping" => wit::Kind::Shipping,
                _ => wit::Kind::Validation,
            };
            let input=json!({"taxStatus":q["price"]["taxStatus"],"salesChannel":c.data.sales_channel,"customerGroup":c.data.group,"shippingCosts":q["shippingCosts"]}).to_string();
            let active_guest = guest.clone();
            let out =
                tokio::task::spawn_blocking(move || active_guest.evaluate(snap, hook, &input))
                    .await
                    .map_err(|_| bad("Commerce hook worker failed"))?
                    .map_err(bad)?;
            if !out.accepted {
                return Err(bad(format!(
                    "Commerce hook {} rejected the cart: {}",
                    m.id, out.reason_code
                )));
            }
            app_adjustment(&mut q, kind, out.adjustment.minor)?;
            if kind != "validation" {
                q = enrich(q, c, ps, s, revision)?;
            }
            audit.push(json!({"app":m.id,"version":m.version,"digest":approval::canonical_digest(&m),"hook":kind,"minor":out.adjustment.minor,"currency":out.adjustment.currency,"scale":out.adjustment.scale,"reasonCode":out.reason_code,"recordRevisions":record_revisions}));
        }
    }
    q["appHookOutcomes"] = json!(audit);
    Ok(q)
}
