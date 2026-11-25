//! Server-rendered storefront-kit islands for deep-linked product pages.
//!
//! The props come from the Store API product owner (`commerce::product_detail`, the
//! same call as the `catalog.detail` capability); no price is computed here. The
//! fallback markup mirrors the kit `product-summary` part with language-neutral
//! content (names, ISO currency amounts), so crawlers, agents and visitors without
//! JavaScript see the product. The client replaces or upgrades it: the storefront
//! app mounts over it, and hosts that load `elements.js` hydrate the element.
use crate::commerce;
use crate::foundation::App;
use axum::Json;
use axum::extract::{FromRef, Path, Query, State};
use serde::{Deserialize, Serialize};
use veyndo_core::request_context::RequestContext;
use veyndo_core::shop_scope::header;

/// The fields of a Store API product detail answer the island reads.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct Detail {
    product: DetailProduct,
    currency_context: CurrencyContext,
    calculated_prices: Vec<Tier>,
    delivery: Option<Delivery>,
    tax_status: String,
}
#[derive(Deserialize, Default)]
#[serde(default)]
struct DetailProduct {
    id: String,
    name: String,
    description: Option<String>,
    media: Vec<Media>,
    stock: Option<u64>,
    min_purchase: Option<u64>,
    purchase_steps: Option<u64>,
    max_purchase: Option<u64>,
    list_price: Option<f64>,
    price: Option<f64>,
    currency: Option<String>,
}
#[derive(Deserialize, Default)]
#[serde(default)]
struct Media {
    url: String,
}
#[derive(Deserialize, Default)]
#[serde(default)]
struct CurrencyContext {
    code: Option<String>,
}
#[derive(Deserialize, Default)]
#[serde(default)]
struct Tier {
    price: TierPrice,
}
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct TierPrice {
    unit_price: Option<f64>,
}
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct Delivery {
    max_days: Option<u64>,
}

/// Props of the kit `product-summary` part (its Zod contract is the source of truth).
#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Island {
    pub locale: String,
    pub price_kind: &'static str,
    pub product: IslandProduct,
}
#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct IslandProduct {
    pub id: String,
    pub name: String,
    pub price: f64,
    pub currency: String,
    pub stock: u64,
    pub min_purchase: u64,
    pub purchase_steps: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list_price: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_purchase: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delivery_days: Option<u64>,
}

/// The `<veyndo-product-summary>` island of one admitted product and its name, or `None` when unavailable.
pub(crate) async fn product_summary(
    a: &App,
    h: &RequestContext,
    id: &str,
) -> Option<(String, String)> {
    let Json(answer) = commerce::product_detail(
        State(FromRef::from_ref(a)),
        h.clone(),
        Path(id.to_string()),
        Query(Default::default()),
    )
    .await
    .ok()?;
    let detail: Detail = serde_json::from_value::<Detail>(answer).ok()?;
    let locale = header(h, "x-commerce-locale").unwrap_or("en-GB");
    let island = props(&detail, locale)?;
    let name = island.product.name.clone();
    Some((render(&island), name))
}

/// Truncates to at most `max` UTF-16 code units (the kit schema's string length).
fn bounded(text: &str, max: usize) -> String {
    let mut units = 0;
    text.chars()
        .take_while(|c| {
            units += c.len_utf16();
            units <= max
        })
        .collect()
}

/// Kit `product-summary` props from a Store API product detail answer.
pub(crate) fn props(detail: &Detail, locale: &str) -> Option<Island> {
    let p = &detail.product;
    if p.id.is_empty() || p.name.is_empty() {
        return None;
    }
    let currency = detail
        .currency_context
        .code
        .clone()
        .or(p.currency.clone())?;
    let price = detail
        .calculated_prices
        .first()
        .and_then(|tier| tier.price.unit_price)
        .or(p.price)?;
    // Only same-origin paths or HTTPS media enter the island.
    let image = p
        .media
        .first()
        .map(|m| m.url.as_str())
        .filter(|u| (u.starts_with('/') && !u.starts_with("//")) || u.starts_with("https://"))
        .map(|u| bounded(u, 2000));
    Some(Island {
        locale: bounded(locale, 35),
        price_kind: if detail.tax_status == "net" {
            "net"
        } else {
            "gross"
        },
        product: IslandProduct {
            id: bounded(&p.id, 200),
            name: bounded(&p.name, 300),
            price,
            currency,
            stock: p.stock.unwrap_or(0),
            min_purchase: p.min_purchase.unwrap_or(1).max(1),
            purchase_steps: p.purchase_steps.unwrap_or(1).max(1),
            description: p
                .description
                .as_deref()
                .filter(|d| !d.is_empty())
                .map(|d| bounded(d, 5000)),
            image,
            list_price: p.list_price.filter(|l| *l > price),
            max_purchase: p.max_purchase.filter(|m| *m >= 1),
            // Zero days means pickup or no window; the part then shows no delivery time.
            delivery_days: detail
                .delivery
                .as_ref()
                .and_then(|d| d.max_days)
                .filter(|d| (1..=365).contains(d)),
        },
    })
}

/// The fields of a `catalog.search` answer the product cards read.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct Listing {
    elements: Vec<ListedProduct>,
    currency_context: CurrencyContext,
}
#[derive(Deserialize, Default)]
#[serde(default)]
struct ListedProduct {
    id: String,
    name: String,
    description: Option<String>,
    media: Vec<Media>,
    stock: Option<u64>,
    price: Option<f64>,
    currency: Option<String>,
    calculated_price: TierPrice,
}

/// Props of the kit `product-card` part.
#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Card {
    pub locale: String,
    pub price_kind: &'static str,
    pub product: CardProduct,
}
#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CardProduct {
    pub id: String,
    pub name: String,
    pub url: String,
    pub price: f64,
    pub currency: String,
    pub stock: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
}

/// The storefront path of a product page in a shop.
pub(crate) fn product_path(tenant: &str, id: &str) -> String {
    let encode = |s: &str| {
        s.bytes()
            .map(|b| match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    (b as char).to_string()
                }
                _ => format!("%{b:02X}"),
            })
            .collect::<String>()
    };
    format!("/products/{}?shop={}", encode(id), encode(tenant))
}

/// Product cards from a `catalog.search` answer: at most 24, with shop links under `origin`.
pub(crate) fn cards(
    listing: &Listing,
    locale: &str,
    tenant: &str,
    origin: Option<&str>,
) -> Vec<Card> {
    let absolute = |path: &str| origin.map(|o| format!("{o}{path}"));
    listing
        .elements
        .iter()
        .filter(|p| !p.id.is_empty() && !p.name.is_empty())
        .take(24)
        .filter_map(|p| {
            let price = p.calculated_price.unit_price.or(p.price)?;
            let currency = listing
                .currency_context
                .code
                .clone()
                .or(p.currency.clone())?;
            let path = product_path(tenant, &p.id);
            Some(Card {
                locale: bounded(locale, 35),
                price_kind: "gross",
                product: CardProduct {
                    id: bounded(&p.id, 200),
                    name: bounded(&p.name, 300),
                    url: bounded(&absolute(&path).unwrap_or(path), 2000),
                    price,
                    currency,
                    stock: p.stock.unwrap_or(0),
                    description: p
                        .description
                        .as_deref()
                        .filter(|d| !d.is_empty())
                        .map(|d| bounded(d, 2000)),
                    image: p.media.first().and_then(|m| media_url(&m.url, origin)),
                },
            })
        })
        .collect()
}

/// The fields of a Store API cart (`cart.quote`, `cart.create`, `cart.replace`) the cart summary reads.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct Quote {
    locale: Option<String>,
    line_items: Vec<QuoteLine>,
    discounts: Vec<QuoteDiscount>,
    shipping_costs: QuoteAmount,
    deliveries: Vec<QuoteDelivery>,
    price: QuotePrice,
    currency_context: CurrencyContext,
}
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct QuoteLine {
    id: String,
    label: String,
    media: Vec<Media>,
    quantity: u64,
    min_purchase: Option<u64>,
    purchase_steps: Option<u64>,
    max_purchase: Option<u64>,
    price: QuoteLinePrice,
}
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct QuoteLinePrice {
    unit_price: Option<f64>,
    total_price: Option<f64>,
}
#[derive(Deserialize, Default)]
#[serde(default)]
struct QuoteDiscount {
    id: String,
    name: std::collections::BTreeMap<String, String>,
    amount: Option<f64>,
}
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct QuoteAmount {
    total_price: Option<f64>,
}
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct QuoteDelivery {
    delivery_date: Option<DeliveryWindow>,
}
#[derive(Deserialize, Serialize, Default, Debug, PartialEq, Clone)]
#[serde(default)]
pub(crate) struct DeliveryWindow {
    pub earliest: String,
    pub latest: String,
}
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct QuotePrice {
    currency: Option<String>,
    position_price: Option<f64>,
    total_price: Option<f64>,
    tax: Option<f64>,
    tax_status: String,
}

/// Props of the kit `cart-summary` part.
#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CartView {
    pub locale: String,
    pub currency: String,
    pub price_kind: &'static str,
    pub lines: Vec<CartLine>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtotal: Option<f64>,
    pub discounts: Vec<CartDiscount>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shipping: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tax: Option<f64>,
    pub total: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delivery: Option<DeliveryWindow>,
}
#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CartLine {
    pub id: String,
    pub label: String,
    pub quantity: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit_price: Option<f64>,
    pub total: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    pub min_purchase: u64,
    pub purchase_steps: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_purchase: Option<u64>,
}
#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CartDiscount {
    pub id: String,
    pub label: String,
    pub amount: f64,
}

/// A finite amount without a negative zero (an empty cart must not show "-0.00").
fn amount(value: f64) -> Option<f64> {
    value.is_finite().then_some(value + 0.0)
}

/// Kit `cart-summary` props from a Store API cart; `None` without a total or currency.
///
/// A discount label uses the exact locale, then its language; without either the
/// part shows its own localized "discount" word instead of another language.
pub(crate) fn cart_view(quote: &Quote, locale: &str, origin: Option<&str>) -> Option<CartView> {
    let currency = quote
        .price
        .currency
        .clone()
        .or(quote.currency_context.code.clone())
        .filter(|c| c.len() == 3 && c.bytes().all(|b| b.is_ascii_uppercase()))?;
    let total = quote.price.total_price.and_then(amount)?;
    let locale = quote.locale.clone().unwrap_or_else(|| locale.to_string());
    let language = locale.split(['-', '_']).next().unwrap_or("").to_string();
    let count = |n: Option<u64>| n.filter(|n| (1..=100_000).contains(n));
    let lines = quote
        .line_items
        .iter()
        .filter(|l| !l.id.is_empty() && !l.label.is_empty() && (1..=100_000).contains(&l.quantity))
        .take(200)
        .filter_map(|l| {
            Some(CartLine {
                id: bounded(&l.id, 200),
                label: bounded(&l.label, 300),
                quantity: l.quantity,
                unit_price: l.price.unit_price.and_then(amount),
                total: l.price.total_price.and_then(amount)?,
                image: l.media.first().and_then(|m| media_url(&m.url, origin)),
                min_purchase: count(l.min_purchase).unwrap_or(1),
                purchase_steps: count(l.purchase_steps).unwrap_or(1),
                max_purchase: count(l.max_purchase),
            })
        })
        .collect();
    let discounts = quote
        .discounts
        .iter()
        .filter(|d| !d.id.is_empty())
        .take(50)
        .filter_map(|d| {
            Some(CartDiscount {
                id: bounded(&d.id, 200),
                label: bounded(
                    d.name
                        .get(&locale)
                        .or_else(|| d.name.get(&language))
                        .map(String::as_str)
                        .unwrap_or(""),
                    300,
                ),
                amount: d.amount.and_then(amount)?,
            })
        })
        .collect();
    Some(CartView {
        locale: bounded(&locale, 35),
        currency,
        price_kind: if quote.price.tax_status == "net" {
            "net"
        } else {
            "gross"
        },
        lines,
        subtotal: quote.price.position_price.and_then(amount),
        discounts,
        shipping: quote.shipping_costs.total_price.and_then(amount),
        tax: quote.price.tax.and_then(amount),
        total,
        delivery: quote
            .deliveries
            .first()
            .and_then(|d| d.delivery_date.clone())
            .filter(|w| !w.earliest.is_empty() && !w.latest.is_empty())
            .map(|w| DeliveryWindow {
                earliest: bounded(&w.earliest, 40),
                latest: bounded(&w.latest, 40),
            }),
    })
}

/// Media as an absolute HTTPS URL: same-origin paths join the shop origin, HTTPS stays.
pub(crate) fn media_url(url: &str, origin: Option<&str>) -> Option<String> {
    if url.starts_with("https://") {
        return Some(bounded(url, 2000));
    }
    (url.starts_with('/') && !url.starts_with("//"))
        .then(|| origin.map(|o| bounded(&format!("{o}{url}"), 2000)))
        .flatten()
}

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// The island element: the props as an attribute and escaped fallback markup.
pub(crate) fn render(island: &Island) -> String {
    let p = &island.product;
    let name = escape(&p.name);
    let amount = format!("{:.2} {}", p.price, escape(&p.currency));
    let media = p.image.as_deref().map_or(String::new(), |src| {
        format!(
            r#"<div class="vy-sf-summary-media"><img src="{}" alt="{name}" width="800" height="640"></div>"#,
            escape(src)
        )
    });
    let description = p
        .description
        .as_deref()
        .map_or(String::new(), |d| format!("<p>{}</p>", escape(d)));
    let props = serde_json::to_string(island).unwrap_or_default();
    format!(
        r#"<veyndo-product-summary data-island="server" locale="{locale}" props="{props}"><section class="vy-sf-summary">{media}<div class="vy-sf-summary-body"><h1>{name}</h1>{description}<p class="vy-sf-price"><strong>{amount}</strong></p></div></section></veyndo-product-summary>"#,
        locale = escape(&island.locale),
        props = escape(&props),
    )
}

/// Places the island into the document root and names the page after the product.
pub(crate) fn inject(html: &str, island: &str, title: &str) -> String {
    html.replacen(
        r#"<div id="root"></div>"#,
        &format!(r#"<div id="root">{island}</div>"#),
        1,
    )
    .replacen(
        "<title>Veyndo</title>",
        &format!("<title>{}</title>", escape(title)),
        1,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn detail(change: impl FnOnce(&mut Value)) -> Detail {
        let mut v = json!({
            "product": {"id":"coat","name":"Coat <b>&</b>","description":"Warm \"wool\"",
                "media":[{"url":"/media/coat.webp"}],"stock":30,"min_purchase":1,
                "purchase_steps":1,"max_purchase":null,"list_price":null,"price":249.0,"currency":"EUR"},
            "currencyContext":{"code":"EUR"},
            "calculatedPrices":[{"price":{"unitPrice":249.0}}],
            "delivery":{"maxDays":3},
            "taxStatus":"gross"
        });
        change(&mut v);
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn props_follow_the_store_api_answer() {
        let island = props(&detail(|_| {}), "de-DE").unwrap();
        assert_eq!(island.locale, "de-DE");
        assert_eq!(island.price_kind, "gross");
        assert_eq!(island.product.price, 249.0);
        assert_eq!(island.product.delivery_days, Some(3));
        assert_eq!(island.product.image.as_deref(), Some("/media/coat.webp"));
        assert_eq!(island.product.max_purchase, None);
        let pickup = detail(|v| v["delivery"]["maxDays"] = json!(0));
        assert_eq!(props(&pickup, "de-DE").unwrap().product.delivery_days, None);
        for url in ["javascript:alert(1)", "//evil.test/x.png"] {
            let unsafe_media = detail(|v| v["product"]["media"][0]["url"] = json!(url));
            assert_eq!(props(&unsafe_media, "de-DE").unwrap().product.image, None);
        }
        assert!(props(&detail(|v| v["product"]["name"] = json!("")), "de-DE").is_none());
        assert_eq!(bounded(&"😀".repeat(200), 300).chars().count(), 150);
    }

    #[test]
    fn cart_views_follow_the_store_api_cart() {
        let quote: Quote = serde_json::from_value(json!({
            "locale":"de-DE",
            "lineItems":[
                {"id":"coat","label":"Coat","quantity":2,"minPurchase":1,"purchaseSteps":1,"maxPurchase":null,
                 "media":[{"url":"/media/coat.webp"}],"price":{"unitPrice":100.0,"totalPrice":200.0}},
                {"id":"ghost","label":"Ghost","quantity":0,"price":{"totalPrice":0.0}}],
            "discounts":[{"id":"d1","name":{"de":"Rabatt"},"amount":20.0},{"id":"d2","name":{"fr":"Remise"},"amount":5.0}],
            "shippingCosts":{"totalPrice":4.9},
            "deliveries":[{"deliveryDate":{"earliest":"2026-11-27","latest":"2026-11-29"}}],
            "price":{"currency":"EUR","positionPrice":200.0,"totalPrice":179.9,"tax":28.72,"taxStatus":"gross"}
        }))
        .unwrap();
        let view = cart_view(&quote, "en-GB", Some("https://shop.example")).unwrap();
        assert_eq!(view.locale, "de-DE");
        assert_eq!(view.lines.len(), 1);
        assert_eq!(
            view.lines[0].image.as_deref(),
            Some("https://shop.example/media/coat.webp")
        );
        assert_eq!(view.discounts[0].label, "Rabatt");
        // No other language stands in for a missing translation.
        assert_eq!(view.discounts[1].label, "");
        assert_eq!(view.total, 179.9);
        assert_eq!(view.delivery.as_ref().unwrap().latest, "2026-11-29");
        let json = serde_json::to_value(&view).unwrap();
        assert!(json["lines"][0].get("maxPurchase").is_none());
        assert!(json["lines"][0].get("url").is_none());
        assert!(cart_view(&Quote::default(), "en-GB", None).is_none());
        assert!(amount(-0.0).unwrap().is_sign_positive());
    }

    #[test]
    fn markup_is_escaped_and_carries_parseable_props() {
        let island = props(&detail(|_| {}), "de-DE").unwrap();
        let html = render(&island);
        assert!(html.starts_with("<veyndo-product-summary data-island=\"server\""));
        assert!(html.contains("<h1>Coat &lt;b&gt;&amp;&lt;/b&gt;</h1>"));
        assert!(html.contains("<p>Warm &quot;wool&quot;</p>"));
        assert!(html.contains("<strong>249.00 EUR</strong>"));
        assert!(!html.contains("<b>"));
        let attribute = html
            .split("props=\"")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap();
        let decoded = attribute
            .replace("&quot;", "\"")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&#39;", "'")
            .replace("&amp;", "&");
        let parsed: Value = serde_json::from_str(&decoded).unwrap();
        assert_eq!(parsed["product"]["name"], "Coat <b>&</b>");
        assert_eq!(parsed["priceKind"], "gross");
        assert!(parsed["product"].get("maxPurchase").is_none());
        let page = inject(
            "<title>Veyndo</title><div id=\"root\"></div>",
            &html,
            "Coat <b>",
        );
        assert!(page.contains("<title>Coat &lt;b&gt;</title>"));
        assert!(page.contains("<div id=\"root\"><veyndo-product-summary"));
    }
}
