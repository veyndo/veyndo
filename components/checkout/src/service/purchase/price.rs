//! The authoritative locked quote: product locks and channel admission, contract prices, locked settings, taxes, promotions, app hooks, the cashier discount and stock.
use super::{Locked, Priced};
use crate::domain::cart_quote::quote;
use crate::domain::tax_lines::tax_products;
use crate::ports::{LockedQuote, Purchase};
use crate::service::delivery::{dates_conn, enrich};
use crate::store::admin_order::admit_products;
use crate::store::legal_snapshot::snapshot as legal_snapshot;
use crate::store::product_locks;
use crate::store::product_snapshot::snapshot as product_snapshot;
use crate::store::selected_customer::check as check_customer;
use crate::store::tax_conditions::tax_settings_for_cart;
use sqlx::Row;
use veyndo_b2b::service::quote_contract::prices as contract_prices;
use veyndo_core::shop_scope::{channel, header};
use veyndo_core::{Error, RequestContext, Result, StatusCode, bad, conflict};
use veyndo_marketing::service::conditions::validate_condition;
use veyndo_marketing::store::promotions::promote;
use veyndo_pricing::catalog::Product;
use veyndo_pricing::service::conversion::amount;
use veyndo_pricing::verified_kernel;
use veyndo_shipping::domain::geography::validate_address;
use veyndo_shop::store::settings::scoped_locked;
use veyndo_shop::types::cart::{StoredCart, selection};

/// Locks the cart's products and their parents in id order and rechecks that each is active and visible in the channel.
async fn locked_products(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    h: &RequestContext,
    c: &StoredCart,
) -> Result<Vec<sqlx::postgres::PgRow>> {
    let ids: Vec<String> = c.data.items.iter().map(|i| i.id.clone()).collect();
    let parents = product_locks::parents(tx, &c.tenant, &ids).await?;
    let mut locks = ids;
    locks.extend(parents);
    locks.sort();
    locks.dedup();
    let rows = product_locks::lock(tx, &c.tenant, &locks).await?;
    // Recheck the new catalog admission under product locks: edits cannot deactivate/hide a SKU
    // between the initial request check and the authoritative inventory/price snapshot.
    if rows.iter().any(|r| !r.get::<bool, _>("active")) {
        return Err(Error(StatusCode::NOT_FOUND, "Product unavailable".into()));
    }
    let locked: Vec<String> = rows.iter().map(|r| r.get::<String, _>("id")).collect();
    let hidden = product_locks::hidden(tx, &c.tenant, &locked, channel(h)).await?;
    if hidden {
        return Err(Error(
            StatusCode::NOT_FOUND,
            "Product hidden in this sales channel".into(),
        ));
    }
    Ok(rows)
}

/// Every line still has the stock it asks for.
fn check_stock(c: &StoredCart, ps: &[Product]) -> Result<()> {
    for i in &c.data.items {
        let p = ps
            .iter()
            .find(|p| p.id == i.id)
            .ok_or(bad("Unknown product"))?;
        if !u64::try_from(p.stock)
            .is_ok_and(|stock| verified_kernel::stock_admissible(stock, u64::from(i.quantity)))
        {
            return Err(conflict("Insufficient stock"));
        }
    }
    Ok(())
}

/// Prices the locked cart; nothing it reads can change before the order commits.
pub(super) async fn run<P: Purchase>(
    port: &P,
    h: &RequestContext,
    locked: &mut Locked<P::Prepared>,
) -> Result<Priced> {
    let c = &locked.cart;
    let tx = &mut locked.tx;
    let rows = locked_products(tx, h, c).await?;
    let mut ps = product_snapshot(
        tx,
        c,
        rows,
        &locked.prepared.language_chain,
        &locked.prepared.locale_chain,
    )
    .await?;
    admit_products(tx, c).await?;
    contract_prices(tx, c, &mut ps).await?;
    // Configuration cannot change between this price calculation and order commit.
    let (config, settings_revision) =
        scoped_locked(tx, &c.tenant, &c.data.sales_channel, &validate_condition).await?;
    if c.data.admin_actor.is_some() {
        check_customer(tx, c).await?;
    }
    let code = config.currencies.selected(&c.data.currency)?.code.clone();
    if let Some(requested) = header(h, "x-commerce-currency")
        && requested != code
    {
        return Err(conflict("Checkout currency changed; review again"));
    }
    let config = port.methods_locked(tx, c, &code, config).await?;
    if h.pos.is_some() {
        port.register_locked(tx, h, c).await?;
    }
    let legal_snapshot = legal_snapshot(tx, c, &config, &ps).await?;
    let selected = selection(&c.data);
    for address in [&selected.address, &selected.billing_address]
        .into_iter()
        .flatten()
    {
        validate_address(
            &address.country,
            &address.country_state_id,
            &config.country_definitions,
        )?;
    }
    port.configurations_locked(tx, c, &locked.prepared.modules)
        .await?;
    if selected.shipping_method_id != "pickup"
        && selected.address.is_none()
        && !ps.iter().all(|p| p.extra["digital"] == true)
    {
        return Err(bad("Delivery address required"));
    }
    let taxes = tax_settings_for_cart(tx, c, &ps, &config).await?;
    let priced = tax_products(&ps, &selected, &taxes)?;
    let promoted = promote(
        tx,
        c,
        enrich(
            quote(c, &priced, &config)?,
            c,
            &priced,
            &config,
            settings_revision,
        )?,
    )
    .await?;
    let q = enrich(promoted, c, &priced, &config, settings_revision)?;
    let lockedq = LockedQuote {
        cart: c,
        products: &priced,
        settings: &config,
        revision: settings_revision,
    };
    let mut q = port
        .adjust_locked(tx, lockedq, q, &locked.prepared.modules)
        .await?;
    dates_conn(tx, &mut q).await?;
    check_stock(c, &ps)?;
    let currency = config.currencies.selected(&c.data.currency)?;
    let money = amount(
        q["price"]["totalPrice"]
            .as_f64()
            .ok_or(bad("Invalid total"))?,
        &currency.code,
        currency.scale,
    )?;
    let minor = money.minor();
    Ok(Priced {
        q,
        config,
        money,
        minor,
        selected,
        legal_snapshot,
    })
}
