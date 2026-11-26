//! Health and localized catalogue HTTP routes.
use crate::cart_storage::load_cart;
use crate::catalog_page::product_page;
use crate::foundation::App;
use crate::localization::language_context;
use crate::{b2b, categories, commerce, currencies, marketing};
use axum::Json;
use axum::extract::State;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_json::json;
use std::collections::HashMap;
use veyndo_catalog::types::catalog_page::CatalogCriteria;
use veyndo_checkout::domain::cart_quote::quote;
use veyndo_core::Result;
use veyndo_core::bad;
use veyndo_core::document::{lenient, merge, view};
use veyndo_core::request_context::RequestContext;
use veyndo_core::shop_scope::header;
use veyndo_core::shop_scope::tenant;
use veyndo_shop::types::cart::Cart;
use veyndo_shop::types::cart::Item;
use veyndo_shop::types::cart::StoredCart;

pub(crate) async fn health(State(a): State<App>) -> Result<Json<Value>> {
    crate::store::process::ping(&a.db).await?;
    Ok(Json(
        json!({"status":"ok","database":"postgresql","knowledge":"PostgreSQL relations + Qdrant","model":*a.models.model,"payment":"simulated","version":env!("CARGO_PKG_VERSION")}),
    ))
}
pub(crate) async fn catalog_request(
    State(a): State<App>,
    h: RequestContext,
    body: axum::body::Bytes,
) -> Result<Json<Value>> {
    // Drain the POST body before returning a large response. Otherwise clients
    // sending headers and body separately can observe a reset/truncated response.
    let criteria = if body.is_empty() {
        CatalogCriteria::default()
    } else {
        serde_json::from_slice(&body).map_err(|_| bad("Invalid catalog criteria"))?
    };
    catalog_page(State(a), h, criteria).await
}

pub(crate) async fn catalog_page(
    State(a): State<App>,
    h: RequestContext,
    criteria: CatalogCriteria,
) -> Result<Json<Value>> {
    Ok(Json(json!(catalog_view(a, h, criteria).await?)))
}

/// One page of priced catalogue elements in the selected currency.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CatalogPage {
    currency_context: Value,
    available_currencies: Value,
    /// Each priced product document as the storefront receives it.
    pub(crate) elements: Vec<Value>,
    total: Option<usize>,
    next_cursor: Value,
    has_more: bool,
    limit: Value,
    locale: Value,
    language_id_chain: Value,
}

/// The price fields a catalogue element adds to its product document.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ElementPrice<'a, S: Serialize> {
    price: veyndo_pricing::decimal::Decimal,
    currency: &'a str,
    currency_scale: S,
    #[serde(rename = "calculated_price")]
    calculated_price: Value,
}

/// The first quoted line of a one-item preview cart.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct PreviewQuote {
    #[serde(deserialize_with = "lenient")]
    line_items: Vec<Value>,
}

/// The price of a quoted line, passed on exactly as quoted.
#[derive(Deserialize, Default)]
#[serde(default)]
struct QuotedLine {
    price: Value,
}

/// Builds the catalogue page `catalog_page` serializes.
pub(crate) async fn catalog_view(
    a: App,
    h: RequestContext,
    mut criteria: CatalogCriteria,
) -> Result<CatalogPage> {
    criteria.page_size()?;
    let (locale, chain) = language_context(&a, &h).await?;
    let t = tenant(&h)?;
    if let Some(id) = &criteria.category_id {
        categories::admit(&a, &h, id).await?;
    }
    criteria.channel_id = marketing::channel_id(&h).into();
    criteria.b2b_unit = b2b::catalog_scope(&a, &h).await?;
    let scope = marketing::catalog_scope(&a, &h).await?;
    criteria.product_ids = match (criteria.product_ids.take(), scope) {
        (Some(ids), Some(scope)) => Some(ids.into_iter().filter(|id| scope.contains(id)).collect()),
        (Some(ids), None) => Some(ids),
        (None, scope) => scope,
    };
    let page = product_page(&a, &t, &chain, &criteria).await?;
    let ps = &page.products;
    let mut data = vec![];
    let c = if header(&h, "sw-context-token").is_some() {
        Some(load_cart(&a, &h).await?)
    } else {
        None
    };
    let (settings, _) = commerce::scoped_config(&a, &t, marketing::channel_id(&h)).await?;
    let currency = settings
        .currencies
        .selected(&currencies::requested(&h, c.as_ref()))?;
    let selected = c
        .as_ref()
        .map(|c| commerce::selection(&c.data))
        .unwrap_or_else(commerce::CheckoutSelection::defaults);
    let selected = commerce::resolve_selection(
        selected,
        c.as_ref()
            .map(|c| c.data.group.as_str())
            .unwrap_or("consumer"),
        &settings,
    );
    let taxes = commerce::tax_settings_for_header(&a, c.as_ref(), &chain, &settings).await?;
    let priced = commerce::tax_products(ps, &selected, &taxes)?;
    for p in &priced {
        let mut preview = if let Some(c) = &c {
            c.clone()
        } else {
            StoredCart {
                id: "preview".into(),
                tenant: t.clone(),
                token: String::new(),
                revision: 0,
                status: "preview".into(),
                data: Cart {
                    admin_actor: None,
                    coupons: vec![],
                    pos_actor: None,
                    pos_discount: None,
                    b2b: None,
                    currency: String::new(),
                    sales_channel: marketing::channel_id(&h).into(),
                    app_configurations: HashMap::new(),
                    items: vec![],
                    group: "consumer".into(),
                    email: None,
                    customer_id: None,
                    company: None,
                    session: String::new(),
                    buyer: None,
                    order: None,
                    locale: locale.clone(),
                    channel: "preview".into(),
                    checkout: None,
                },
            }
        };
        // A receipt keeps its accepted configuration, but a new catalogue view
        // must not price another purchase with the completed cart's old choices.
        if preview.status != "open" {
            preview.data.app_configurations.clear();
        }
        preview.data.currency = currency.code.clone();
        preview.data.items = vec![Item {
            id: p.id.clone(),
            quantity: p.min_purchase,
        }];
        // This preview contains one item. Searching the entire catalog for each
        // preview would turn catalog hydration into quadratic work.
        let q = quote(&preview, std::slice::from_ref(p), &settings)?;
        let mut v = json!(p);
        let quoted: PreviewQuote = view(&q);
        merge(
            &mut v,
            ElementPrice {
                price: currencies::product_price(p, &settings.currencies, currency)?,
                currency: &currency.code,
                currency_scale: currency.scale,
                calculated_price: quoted
                    .line_items
                    .first()
                    .map(|line| view::<QuotedLine>(line).price)
                    .unwrap_or_default(),
            },
        )?;
        data.push(v);
    }
    Ok(CatalogPage {
        currency_context: currencies::context(&settings.currencies, currency),
        available_currencies: json!(settings.currencies.enabled),
        elements: data,
        total: if criteria.after.is_none() && page.next_cursor.is_none() {
            Some(ps.len())
        } else {
            None
        },
        next_cursor: json!(page.next_cursor),
        has_more: page.next_cursor.is_some(),
        limit: json!(page.limit),
        locale: json!(locale),
        language_id_chain: json!(chain),
    })
}

#[cfg(test)]
impl CatalogPage {
    /// A page around the given elements for shape tests.
    pub(crate) fn sample(elements: Vec<Value>) -> Self {
        CatalogPage {
            currency_context: json!({"code":"EUR","scale":2}),
            available_currencies: json!(["EUR"]),
            elements,
            total: Some(3),
            next_cursor: Value::Null,
            has_more: false,
            limit: json!(8),
            locale: json!("en-GB"),
            language_id_chain: json!(["en-GB"]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CatalogPage, ElementPrice, PreviewQuote, QuotedLine};
    use serde_json::{Value, json};
    use veyndo_core::document::{merge, view};

    #[test]
    fn priced_element_keeps_the_indexed_shape() {
        let product = json!({"id":"p-1","price":9.5,"stock":3,"name":{"en":"Mug"}});
        let q = json!({"lineItems":[{"price":{"unitPrice":11.31,"totalPrice":11.31}}]});
        let mut old = product.clone();
        old["price"] = json!(10.25_f64);
        old["currency"] = json!("CHF");
        old["currencyScale"] = json!(2_u8);
        old["calculated_price"] = q["lineItems"][0]["price"].clone();
        let mut new = product;
        let quoted: PreviewQuote = view(&q);
        merge(
            &mut new,
            ElementPrice {
                price: veyndo_pricing::decimal::Decimal::parse("10.25").unwrap(),
                currency: "CHF",
                currency_scale: 2_u8,
                calculated_price: quoted
                    .line_items
                    .first()
                    .map(|line| view::<QuotedLine>(line).price)
                    .unwrap_or_default(),
            },
        )
        .unwrap();
        assert_eq!(new, old);
        let empty: PreviewQuote = view(&json!({"lineItems":[]}));
        assert!(empty.line_items.is_empty());
    }

    #[test]
    fn catalog_page_keeps_the_literal_shape() {
        let next: Option<String> = Some("c-2".into());
        let old = json!({"currencyContext":{"code":"EUR"},"availableCurrencies":["EUR"],"elements":[{"id":"p-1"}],"total":Option::<usize>::None,"nextCursor":next,"hasMore":next.is_some(),"limit":8,"locale":"de-DE","languageIdChain":["de-DE","en-GB"]});
        let new = json!(CatalogPage {
            currency_context: json!({"code":"EUR"}),
            available_currencies: json!(["EUR"]),
            elements: vec![json!({"id":"p-1"})],
            total: None,
            next_cursor: json!(next),
            has_more: next.is_some(),
            limit: json!(8),
            locale: json!("de-DE"),
            language_id_chain: json!(["de-DE", "en-GB"]),
        });
        assert_eq!(new, old);
        let _: Value = new;
    }
}
