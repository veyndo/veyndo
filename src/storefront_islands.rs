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
