//! Scripted hostile shoppers probe native cart and checkout fences in a private sandbox; no model call, no live effect.
use super::*;
/// A refusal by the native owner is the expected result; an accepted attack is reported with a closed finding code.
fn verdict(
    id: &str,
    result: Result<Value>,
    accepted: impl FnOnce(&Value) -> (&'static str, &'static str),
) -> Value {
    match result {
        Err(e) if e.0.is_client_error() => {
            json!({"id":id,"outcome":"held","status":e.0.as_u16(),"detail":e.1.chars().take(300).collect::<String>()})
        }
        Err(e) => {
            json!({"id":id,"outcome":"review","finding":"failed_unexpectedly","status":e.0.as_u16()})
        }
        Ok(v) => {
            let (outcome, finding) = accepted(&v);
            json!({"id":id,"outcome":outcome,"finding":finding})
        }
    }
}
async fn fresh(
    a: &App,
    t: &str,
    request: &Value,
    run: &str,
    n: u8,
) -> Result<(RequestContext, i64)> {
    let guest = shopper::headers(t, request, None)?;
    let cart = shopper::call(
        a,
        &guest,
        "cart.create",
        &json!({"session":format!("probe-{run}-{n}")}),
    )
    .await?;
    let h = shopper::headers(t, request, cart["token"].as_str())?;
    let revision = load_cart(a, &h).await?.revision;
    Ok((h, revision))
}
fn total(v: &Value) -> Option<&str> {
    v["price"]["totalMinor"].as_str()
}
/// The product with this ID, or the first one that has stock.
fn stocked(products: &Value, id: Option<&str>) -> Option<Value> {
    products
        .as_array()
        .into_iter()
        .flatten()
        .find(|p| match id {
            Some(id) => p["id"].as_str() == Some(id),
            None => p["stock"].as_i64().is_some_and(|s| s > 0),
        })
        .cloned()
}
/// Forged total, stale review and replayed order against one honestly reviewed basket.
async fn checkout_probes(
    a: &App,
    c: &RequestContext,
    guest: &RequestContext,
    country: &str,
    run: &str,
    id: &str,
    stock: i64,
) -> Result<Vec<Value>> {
    let mut probes = Vec::new();
    // The checkout attacks share one honestly reviewed basket; only the claimed review is forged.
    let reviewed = async {
        let (revision, minor) = shopper::review(a, c, country).await?;
        shopper::accept(a, c).await?;
        Ok::<_, Error>((revision, minor))
    }
    .await;
    match reviewed {
        Err(e) => {
            for id in ["forged_total", "stale_review", "replayed_order"] {
                probes.push(json!({"id":id,"outcome":"review","finding":"checkout_unavailable","status":e.0.as_u16(),"detail":e.1.chars().take(300).collect::<String>()}));
            }
        }
        Ok((revision, minor)) => {
            let key = format!("probe-{run}");
            probes.push(verdict(
                "forged_total",
                shopper::complete(a, c, &revision, 1, &key).await,
                |_| ("breached", "accepted"),
            ));
            probes.push(verdict(
                "stale_review",
                shopper::complete(
                    a,
                    c,
                    &json!(revision.as_i64().unwrap_or(1) - 1),
                    minor,
                    &key,
                )
                .await,
                |_| ("breached", "accepted"),
            ));
            let first = shopper::complete(a, c, &revision, minor, &key).await;
            let second = shopper::complete(a, c, &revision, minor, &key).await;
            let after = stocked(&shopper::products(a, guest, "").await?, Some(id))
                .and_then(|p| p["stock"].as_i64())
                .unwrap_or(stock);
            probes.push(match (first, second) {
                (Ok(first), Ok(second)) if first["id"] != second["id"] => {
                    json!({"id":"replayed_order","outcome":"breached","finding":"duplicate_order"})
                }
                (Ok(_), Ok(_)) if stock - after > 1 => {
                    json!({"id":"replayed_order","outcome":"breached","finding":"stock_double"})
                }
                (Ok(first), Ok(_)) => {
                    json!({"id":"replayed_order","outcome":"held","finding":"same_order","orderId":first["id"]})
                }
                (Ok(first), Err(_)) => {
                    json!({"id":"replayed_order","outcome":"held","finding":"replay_refused","orderId":first["id"]})
                }
                (Err(e), _) => {
                    json!({"id":"replayed_order","outcome":"review","finding":"checkout_unavailable","status":e.0.as_u16(),"detail":e.1.chars().take(300).collect::<String>()})
                }
            });
        }
    }
    Ok(probes)
}
pub(super) async fn run(a: &App, h: &RequestContext, t: &str) -> Result<Value> {
    let settings = catalogue::settings(a, h, t).await?;
    let currency = settings.currencies.selected("")?;
    let locale = settings
        .locales
        .iter()
        .find(|l| *l == &settings.main_locale)
        .or_else(|| settings.locales.first())
        .ok_or(bad("Enable a channel language before running attack tests"))?;
    let country = settings
        .countries
        .first()
        .ok_or(bad("Enable a delivery country before running attack tests"))?;
    let request = json!({"channel":marketing::channel_id(h),"locale":locale,"scenario":{"currency":currency.code}});
    let guest = shopper::headers(t, &request, None)?;
    let target = stocked(&shopper::products(a, &guest, "").await?, None).ok_or(conflict(
        "Add a product with stock to this sandbox before running attack tests",
    ))?;
    let id = target["id"].as_str().ok_or(bad("Product unavailable"))?;
    let stock = target["stock"].as_i64().unwrap_or(0);
    let run = uid();
    let one = |extra: Value| {
        let mut item = json!({"id":id,"quantity":1});
        if let (Some(item), Some(extra)) = (item.as_object_mut(), extra.as_object()) {
            item.extend(extra.clone());
        }
        json!([item])
    };
    let mut probes = Vec::new();
    let (c, rev) = fresh(a, t, &request, &run, 1).await?;
    probes.push(verdict(
        "negative_quantity",
        shopper::call(
            a,
            &c,
            "cart.replace",
            &json!({"revision":rev,"items":[{"id":id,"quantity":-1}]}),
        )
        .await,
        |_| ("breached", "accepted"),
    ));
    probes.push(verdict(
        "unknown_product",
        shopper::call(
            a,
            &c,
            "cart.replace",
            &json!({"revision":rev,"items":[{"id":format!("probe-missing-{run}"),"quantity":1}]}),
        )
        .await,
        |_| ("breached", "accepted"),
    ));
    // A cart may hold more than the stock; what matters is whether that basket can be bought.
    let excess = shopper::call(
        a,
        &c,
        "cart.replace",
        &json!({"revision":rev,"items":[{"id":id,"quantity":stock + 1}]}),
    )
    .await;
    let over = excess.as_ref().is_ok_and(|v| {
        v["lineItems"][0]["quantity"]
            .as_i64()
            .is_none_or(|q| q > stock)
    });
    probes.push(if over {
        let bought = async {
            let (revision, minor) = shopper::review(a, &c, country).await?;
            shopper::accept(a, &c).await?;
            shopper::complete(a, &c, &revision, minor, &format!("probe-{run}-excess")).await
        }
        .await;
        verdict("excess_quantity", bought, |_| ("review", "over_stock"))
            .as_object()
            .map_or(Value::Null, |o| {
                let mut o = o.clone();
                if o["outcome"] == "held" {
                    o.insert("finding".into(), json!("refused_at_checkout"));
                }
                Value::Object(o)
            })
    } else {
        verdict("excess_quantity", excess, |_| ("held", "limited"))
    });
    // A clean one-item basket is the reference total for the injected-price basket.
    let (c, rev) = fresh(a, t, &request, &run, 2).await?;
    let clean = shopper::call(
        a,
        &c,
        "cart.replace",
        &json!({"revision":rev,"items":one(json!({}))}),
    )
    .await?;
    probes.push(verdict(
        "stale_cart",
        shopper::call(
            a,
            &c,
            "cart.replace",
            &json!({"revision":rev,"items":[{"id":id,"quantity":2}]}),
        )
        .await,
        |_| ("breached", "accepted"),
    ));
    let (injected, rev) = fresh(a, t, &request, &run, 3).await?;
    probes.push(verdict(
        "price_injection",
        shopper::call(a, &injected, "cart.replace", &json!({"revision":rev,"items":one(json!({"price":0.01,"unitPrice":0.01,"priceMinor":1}))})).await,
        |v| if total(v).is_some() && total(v) == total(&clean) { ("held", "ignored") } else { ("breached", "total_changed") },
    ));
    probes.push(verdict(
        "guessed_cart",
        shopper::call(
            a,
            &shopper::headers(t, &request, Some(&format!("probe-guess-{run}")))?,
            "cart.quote",
            &json!({}),
        )
        .await,
        |_| ("breached", "accepted"),
    ));
    probes.extend(checkout_probes(a, &c, &guest, country, &run, id, stock).await?);
    let count = |outcome: &str| probes.iter().filter(|p| p["outcome"] == outcome).count();
    Ok(
        json!({"id":run,"synthetic":true,"execution":"native-commerce-api","channel":marketing::channel_id(h),
        "product":{"id":id,"name":target["name"]},"configurationDigest":evidence::capture(a, t).await,
        "summary":{"held":count("held"),"breached":count("breached"),"review":count("review")},"probes":probes}),
    )
}
