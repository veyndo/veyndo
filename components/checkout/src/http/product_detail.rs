//! Storefront product detail: family, context prices per quantity tier, gallery, properties, delivery window and review aggregates.
use crate::domain::cart_quote::{normalized_quantity, quote};
use crate::domain::tax_lines::tax_products;
use crate::store::cart_load;
use crate::store::tax_conditions::{conditional, tax_settings_for_cart};
use axum::Json;
use axum::extract::{Path, Query, State as Shared};
use serde_json::{Value, json};
use sqlx::Row;
use veyndo_catalog::ports::Catalog;
use veyndo_catalog::service::catalog_reads;
use veyndo_catalog::types::catalog_page::CatalogCriteria;
use veyndo_core::shop_scope::{channel, header, tenant};
use veyndo_core::{Error, RequestContext, Result, State, StatusCode};
use veyndo_marketing::service::channels;
use veyndo_pricing::catalog::Product;
use veyndo_pricing::service::conversion as currencies;
use veyndo_shop::Settings;
use veyndo_shop::domain::checkout_selection::resolve_selection;
use veyndo_shop::service::language::language_context;
use veyndo_shop::service::read_cache::settings;
use veyndo_shop::types::cart::{Cart, Item, StoredCart, selection};
use veyndo_shop::types::settings::CheckoutSelection;

/// Tax settings for a visitor: rule conditions are settled on the visitor's cart when one exists.
async fn visitor_taxes(
    c: &Catalog,
    cart: Option<&StoredCart>,
    chain: &[String],
    s: &Settings,
) -> Result<Settings> {
    if let Some(cart) = cart
        && conditional(s)
    {
        let ids = cart
            .data
            .items
            .iter()
            .map(|i| i.id.clone())
            .collect::<Vec<_>>();
        let ps = catalog_reads::for_cart(c, &cart.tenant, chain, &ids).await?;
        return tax_settings_for_cart(&mut *c.core.db.begin().await?, cart, &ps, s).await;
    }
    Ok(s.clone())
}

/// A one-line preview cart that prices a product at one quantity for the visitor's group and currency.
fn preview_cart(
    tenant: &str,
    id: &str,
    qty: u32,
    group: &str,
    currency: &str,
    cart: Option<&StoredCart>,
) -> StoredCart {
    let data = Cart {
        admin_actor: None,
        coupons: vec![],
        pos_actor: None,
        pos_discount: None,
        b2b: None,
        currency: currency.into(),
        sales_channel: "default".into(),
        app_configurations: cart
            .filter(|c| c.status == "open")
            .map(|c| c.data.app_configurations.clone())
            .unwrap_or_default(),
        items: vec![Item {
            id: id.into(),
            quantity: qty,
        }],
        group: group.into(),
        email: None,
        customer_id: None,
        company: None,
        session: String::new(),
        buyer: None,
        order: None,
        locale: String::new(),
        channel: "preview".into(),
        checkout: None,
    };
    StoredCart {
        id: "preview".into(),
        tenant: tenant.into(),
        token: String::new(),
        revision: 0,
        status: "preview".into(),
        data,
    }
}

/// The unit price at the minimum purchase and at every advanced-price tier of the visitor's group.
fn price_tiers(
    t: &str,
    p: &Product,
    priced: &[Product],
    group: &str,
    config: &Settings,
    currency: &str,
    cart: Option<&StoredCart>,
) -> Result<Vec<Value>> {
    let tier_group = if p.advanced_prices.iter().any(|tier| tier.rule_id == group) {
        group
    } else if config.is_business(group) {
        "business"
    } else {
        "consumer"
    };
    let mut quantities = vec![p.min_purchase];
    quantities.extend(
        p.advanced_prices
            .iter()
            .filter(|tier| tier.rule_id == tier_group)
            .map(|tier| tier.quantity_start),
    );
    quantities.sort();
    quantities.dedup();
    let mut tiers = vec![];
    for requested in quantities {
        let qty = normalized_quantity(p, requested)?;
        let preview = preview_cart(t, &p.id, qty, group, currency, cart);
        let q = quote(&preview, priced, config)?;
        tiers.push(json!({"quantity":qty,"price":q["lineItems"][0]["price"],"discountPercent":q["lineItems"][0]["discountPercent"]}));
    }
    Ok(tiers)
}

/// Approved reviews and their aggregate over the whole family, independently of variant pagination.
async fn reviews(a: &State, t: &str, family: &str) -> Result<Value> {
    let rows=sqlx::query("SELECT r.id,r.author,r.rating,r.title,r.content,r.verified,r.demo,(r.source_import IS NOT NULL) AS imported,r.created_at::text AS time FROM product_reviews r WHERE r.tenant=$1 AND r.approved AND (r.product_id=$2 OR EXISTS(SELECT 1 FROM products p WHERE p.tenant=r.tenant AND p.id=r.product_id AND p.parent_id=$2)) ORDER BY r.created_at DESC LIMIT 50").bind(t).bind(family).fetch_all(&a.db).await?;
    let stats=sqlx::query("SELECT count(*) AS count,coalesce(avg(r.rating),0)::double precision AS rating FROM product_reviews r WHERE r.tenant=$1 AND r.approved AND (r.product_id=$2 OR EXISTS(SELECT 1 FROM products p WHERE p.tenant=r.tenant AND p.id=r.product_id AND p.parent_id=$2))").bind(t).bind(family).fetch_one(&a.db).await?;
    Ok(
        json!({"count":stats.get::<i64, _>("count"),"average":stats.get::<f64, _>("rating"),"elements":rows.iter().map(|r|json!({"id":r.get::<String,_>("id"),"author":r.get::<String,_>("author"),"rating":r.get::<i32,_>("rating"),"title":r.get::<String,_>("title"),"content":r.get::<String,_>("content"),"verifiedPurchase":r.get::<bool,_>("verified"),"demo":r.get::<bool,_>("demo"),"imported":r.get::<bool,_>("imported"),"time":r.get::<String,_>("time")})).collect::<Vec<_>>()}),
    )
}

/// A product as JSON with its context price in the requested currency.
fn priced_json(
    p: &Product,
    priced: &[Product],
    config: &Settings,
    currency: &veyndo_pricing::types::currency::Definition,
) -> Result<Value> {
    let mut v = json!(p);
    v["price"] = json!(currencies::product_price(
        priced.iter().find(|v| v.id == p.id).unwrap_or(p),
        &config.currencies,
        currency
    )?);
    v["currency"] = json!(currency.code);
    Ok(v)
}

/// `GET /store-api/product/{id}`: the product with its family, prices, delivery window and reviews.
pub async fn product_detail(
    Shared(c): Shared<Catalog>,
    h: RequestContext,
    Path(id): Path<String>,
    Query(mut criteria): Query<CatalogCriteria>,
) -> Result<Json<Value>> {
    let a = &c.core;
    let t = tenant(&h)?;
    channels::admit_product(a, &h, &id).await?;
    let (_, chain) = language_context(a, &h).await?;
    criteria.channel_id = channel(&h).into();
    criteria.b2b_unit = veyndo_b2b::store::catalog::scope(a, &h).await?;
    criteria.product_ids = channels::catalog_scope(a, &h).await?;
    let (ps, family, next_cursor) = catalog_reads::family(&c, &t, &chain, &id, &criteria).await?;
    let p = ps
        .iter()
        .find(|p| p.id == id)
        .ok_or(Error(StatusCode::NOT_FOUND, "Product not found".into()))?;
    let visible = channels::filter_channel(a, &h, ps.clone()).await?;
    let cart = if header(&h, "sw-context-token").is_some() {
        Some(cart_load::load(a, &h).await?)
    } else {
        None
    };
    let group = cart
        .as_ref()
        .map(|c| c.data.group.as_str())
        .unwrap_or("consumer");
    let selected = cart
        .as_ref()
        .map(|c| selection(&c.data))
        .unwrap_or_else(CheckoutSelection::defaults);
    let (config, _) = settings(a, &t, channel(&h)).await?;
    let requested = cart
        .as_ref()
        .map(|c| c.data.currency.clone())
        .unwrap_or_else(|| header(&h, "x-commerce-currency").unwrap_or("").into());
    let currency = config.currencies.selected(&requested)?;
    let selected = resolve_selection(selected, group, &config);
    let taxes = visitor_taxes(&c, cart.as_ref(), &chain, &config).await?;
    let priced = tax_products(&ps, &selected, &taxes)?;
    let tiers = price_tiers(
        &t,
        p,
        &priced,
        group,
        &config,
        &currency.code,
        cart.as_ref(),
    )?;
    let reviews = reviews(a, &t, &family).await?;
    let method = config.shipping.iter().find(|v| {
        v.id == selected.shipping_method_id && v.active && v.countries.contains(&selected.country)
    });
    let delivery=method.map(|method|json!({"method":method,"minDays":if method.id=="pickup"{0}else{p.delivery_days.max(method.min_days)},"maxDays":if method.id=="pickup"{0}else{p.delivery_days.max(method.max_days)}}));
    let own = priced.iter().find(|v| v.id == id).unwrap_or(p);
    let mut product = priced_json(own, &priced, &config, currency)?;
    product["variantLabel"] = json!(
        p.options
            .as_object()
            .map(|o| o
                .values()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join(" / "))
            .unwrap_or_default()
    );
    let variants = visible
        .iter()
        .filter(|v| v.id == family || v.parent_id.as_deref() == Some(&family))
        .map(|p| priced_json(p, &priced, &config, currency))
        .collect::<Result<Vec<_>>>()?;
    Ok(Json(
        json!({"currencyContext":currencies::context(&config.currencies,currency),"availableCurrencies":config.currencies.enabled,"product":product,"familyId":family,"variants":variants,"variantsPagination":{"nextCursor":next_cursor,"hasMore":next_cursor.is_some(),"limit":criteria.page_size()?},"calculatedPrices":tiers,"delivery":delivery,"taxStatus":if config.is_business(group){"net"}else{"gross"},"country":selected.country,"reviews":reviews}),
    ))
}
