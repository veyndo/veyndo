//! Storefront kit parts as an MCP App view: the `ui://` document, the tool links and each tool result's part payload.
//!
//! The view is the `frontend/dist/mcp/storefront-parts.js` bundle built from the kit
//! part contracts (`frontend/src/kit/storefront/mcp-app.tsx`). Part props travel in
//! the result's `_meta` (hosts do not give `_meta` to the model) and are derived from
//! the same Store API answers the model sees. The view never pays: buying opens the
//! shop's own product page or checkout through the host (`ui/open-link`); cart
//! changes and purchase stay with the agent's native tools.
use crate::apps::mcp_views::MIME;
use crate::storefront_islands;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::view;
use veyndo_core::request_context::RequestContext;
use veyndo_core::shop_scope::{header, tenant};

/// The view resource every linked tool names.
pub(super) const URI: &str = "ui://veyndo/kit/storefront";
/// Where a tool result carries its part payload.
pub(super) const PART: &str = "ai.veyndo/part";
/// Commerce tools whose results the view renders.
const TOOLS: [&str; 5] = [
    "catalog.detail",
    "catalog.search",
    "cart.create",
    "cart.quote",
    "cart.replace",
];

/// The `_meta` object of a JSON object, created when absent.
fn meta(value: &mut Value) -> Option<&mut serde_json::Map<String, Value>> {
    value
        .as_object_mut()?
        .entry("_meta")
        .or_insert_with(|| json!({}))
        .as_object_mut()
}

/// Links the description of tool `name` to the view (standard key and the ChatGPT alias).
pub(super) fn link(name: &str, tool: &mut Value) {
    if TOOLS.contains(&name)
        && let Some(meta) = meta(tool)
    {
        meta.insert("ui".into(), json!({"resourceUri": URI}));
        meta.insert("openai/outputTemplate".into(), json!(URI));
    }
}

/// The fields of a tool result `attach` reads.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ToolResult {
    is_error: bool,
    structured_content: Value,
}
#[derive(Deserialize, Default)]
#[serde(default)]
struct Answer {
    locale: Option<String>,
}

/// The resource listing entry of the view.
pub(super) fn listing() -> Value {
    json!({"uri":URI,"name":"Storefront parts","mimeType":MIME,
        "description":"Product cards, the buy box and the cart summary from the shop's storefront kit."})
}

/// The view document: the built bundle in a self-contained page, or `None` before a frontend build.
pub(super) async fn read(origin: Option<&str>) -> Option<Value> {
    let script = tokio::fs::read_to_string("frontend/dist/mcp/storefront-parts.js")
        .await
        .ok()?;
    Some(
        json!({"uri":URI,"mimeType":MIME,"text":document(&script, origin),
        "_meta":{"ui":{"csp":{"connectDomains":[],"resourceDomains":origin.into_iter().collect::<Vec<_>>()},
                       "prefersBorder":true}}}),
    )
}

/// The page around the bundle; the script cannot end its element early.
pub(super) fn document(script: &str, origin: Option<&str>) -> String {
    let script = script
        .replace("</script", "<\\/script")
        .replace("</SCRIPT", "<\\/SCRIPT")
        .replace("<!--", "<\\!--");
    let images = origin.unwrap_or("'none'");
    format!(
        "<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src {images}; base-uri 'none'; form-action 'none'\"><title>Veyndo</title><body style=\"margin:0;padding:12px\"><div id=\"root\"></div><script>{script}</script></body></html>"
    )
}

/// Adds the part payload to a successful result of a linked tool.
pub(super) fn attach(h: &RequestContext, name: &str, result: &mut Value, origin: Option<&str>) {
    if !TOOLS.contains(&name) {
        return;
    }
    let typed: ToolResult = view(result);
    let Ok(shop) = tenant(h) else { return };
    if typed.is_error {
        return;
    }
    let answer: Answer = view(&typed.structured_content);
    let locale = header(h, "x-commerce-locale")
        .map(str::to_string)
        .or(answer.locale)
        .unwrap_or_else(|| "en-GB".into());
    if let Some(part) = payload(name, &typed.structured_content, &locale, &shop, origin)
        && let Some(meta) = meta(result)
    {
        meta.insert(PART.into(), part);
    }
}

/// The part payload of a tool answer, or `None` when the answer has nothing to show.
pub(super) fn payload(
    name: &str,
    answer: &Value,
    locale: &str,
    shop: &str,
    origin: Option<&str>,
) -> Option<Value> {
    let link =
        |id: &str| origin.map(|o| format!("{o}{}", storefront_islands::product_path(shop, id)));
    match name {
        "catalog.detail" => {
            let detail: storefront_islands::Detail = serde_json::from_value(answer.clone()).ok()?;
            let mut island = storefront_islands::props(&detail, locale)?;
            island.product.image = island
                .product
                .image
                .as_deref()
                .and_then(|url| storefront_islands::media_url(url, origin));
            let id = island.product.id.clone();
            Some(
                json!({"part":"product-summary","locale":locale,"props":island,
                "links": link(&id).map(|url| json!({id: url})).unwrap_or_else(|| json!({}))}),
            )
        }
        "catalog.search" => {
            let listing: storefront_islands::Listing =
                serde_json::from_value(answer.clone()).ok()?;
            let cards = storefront_islands::cards(&listing, locale, shop, origin);
            (!cards.is_empty()).then(|| {
                let links = cards
                    .iter()
                    .filter_map(|card| {
                        link(&card.product.id).map(|url| (card.product.id.clone(), json!(url)))
                    })
                    .collect::<serde_json::Map<_, _>>();
                json!({"part":"product-card","locale":locale,"items":cards,"links":links})
            })
        }
        "cart.create" | "cart.quote" | "cart.replace" => {
            let quote: storefront_islands::Quote = serde_json::from_value(answer.clone()).ok()?;
            let view = storefront_islands::cart_view(&quote, locale, origin)?;
            // Lines open their product page; changing the cart stays with the agent's tools.
            let links = view
                .lines
                .iter()
                .filter_map(|line| link(&line.id).map(|url| (line.id.clone(), json!(url))))
                .collect::<serde_json::Map<_, _>>();
            Some(json!({"part":"cart-summary","locale":view.locale,"props":view,"links":links}))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn detail() -> Value {
        json!({"product":{"id":"coat","name":"Coat","media":[{"url":"/media/coat.webp"}],"stock":3,"price":249.0,"currency":"EUR"},
            "currencyContext":{"code":"EUR"},"calculatedPrices":[{"price":{"unitPrice":249.0}}],"taxStatus":"gross"})
    }

    #[test]
    fn commerce_reads_and_carts_link_the_view() {
        let mut tool = json!({"name":"catalog.detail"});
        link("catalog.detail", &mut tool);
        assert_eq!(tool["_meta"]["ui"]["resourceUri"], URI);
        assert_eq!(tool["_meta"]["openai/outputTemplate"], URI);
        let mut other = json!({"name":"checkout.complete"});
        link("checkout.complete", &mut other);
        assert!(other.get("_meta").is_none());
    }

    #[test]
    fn detail_and_search_become_part_payloads_with_shop_links() {
        let origin = Some("https://shop.example");
        let summary = payload("catalog.detail", &detail(), "de-DE", "dev shop", origin).unwrap();
        assert_eq!(summary["part"], "product-summary");
        assert_eq!(
            summary["props"]["product"]["image"],
            "https://shop.example/media/coat.webp"
        );
        assert_eq!(
            summary["links"]["coat"],
            "https://shop.example/products/coat?shop=dev%20shop"
        );
        // Without a public HTTPS origin there are no links and no relative images.
        let local = payload("catalog.detail", &detail(), "de-DE", "s", None).unwrap();
        assert!(local["props"]["product"].get("image").is_none());
        assert_eq!(local["links"], json!({}));
        let listing = json!({"currencyContext":{"code":"EUR"},"elements":[
            {"id":"bag","name":"Bag","price":10.0,"calculated_price":{"unitPrice":12.0},"stock":1,"media":[{"url":"https://cdn.example/b.png"}]},
            {"id":"","name":"nameless"}]});
        let cards = payload("catalog.search", &listing, "en-GB", "s", origin).unwrap();
        assert_eq!(cards["items"].as_array().unwrap().len(), 1);
        assert_eq!(cards["items"][0]["product"]["price"], 12.0);
        assert_eq!(
            cards["items"][0]["product"]["url"],
            "https://shop.example/products/bag?shop=s"
        );
        assert!(
            payload(
                "catalog.search",
                &json!({"elements":[]}),
                "en-GB",
                "s",
                origin
            )
            .is_none()
        );
        assert!(payload("cart.quote", &detail(), "en-GB", "s", origin).is_none());
        let cart = json!({"locale":"de-DE","lineItems":[{"id":"coat","label":"Coat","quantity":1,
            "price":{"unitPrice":249.0,"totalPrice":249.0}}],
            "price":{"currency":"EUR","totalPrice":249.0,"taxStatus":"gross"}});
        for tool in ["cart.create", "cart.quote", "cart.replace"] {
            let summary = payload(tool, &cart, "en-GB", "s", origin).unwrap();
            assert_eq!(summary["part"], "cart-summary");
            assert_eq!(summary["locale"], "de-DE");
            assert_eq!(summary["props"]["lines"][0]["total"], 249.0);
            assert_eq!(
                summary["links"]["coat"],
                "https://shop.example/products/coat?shop=s"
            );
        }
    }

    #[test]
    fn the_document_keeps_the_script_inside_its_element() {
        let page = document(
            "let a = '</script><script>x()'; <!--",
            Some("https://shop.example"),
        );
        assert_eq!(page.matches("</script>").count(), 1);
        assert!(page.contains("img-src https://shop.example;"));
        assert!(document("1", None).contains("img-src 'none'"));
    }
}
