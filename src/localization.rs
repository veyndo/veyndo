//! Shop locale resolution, translated catalog hydration and non-mutating merchant quote.
use super::*;
use veyndo::context::{Language, language_chain};

pub(super) async fn language_context(a: &App, h: &RequestContext) -> Result<(String, Vec<String>)> {
    let (settings, _) = commerce::config(a, &tenant(h)?).await?;
    let locale = header(h, "x-commerce-locale").unwrap_or(&settings.main_locale);
    let rows = performance::languages(a).await?;
    let selected = if let Some(id) = header(h, "sw-language-id") {
        rows.iter().find(|r| r.id == id)
    } else {
        rows.iter().find(|r| r.locale == locale)
    }
    .ok_or(bad("Language unavailable"))?;
    let available = rows.iter().map(|r| r.id.clone()).collect::<Vec<_>>();
    let languages = rows
        .iter()
        .map(|r| Language {
            id: r.id.clone(),
            parent_id: r.parent_id.clone(),
        })
        .collect::<Vec<_>>();
    let mut chain = language_chain(&selected.id, &available, &languages).map_err(bad)?;
    if settings.main_locale != "en-GB" {
        let main = rows
            .iter()
            .find(|r| r.locale == settings.main_locale)
            .ok_or(bad("Main language unavailable"))?
            .id
            .clone();
        let current = selected.id.clone();
        chain.retain(|id| id != veyndo::context::SYSTEM_LANGUAGE || id == &current);
        if !chain.contains(&main) {
            chain.push(main);
        }
    }
    Ok((selected.locale.clone(), chain))
}
pub(super) async fn localize_products(
    a: &App,
    t: &str,
    chain: &[String],
    ps: Vec<Product>,
) -> Result<Vec<Product>> {
    veyndo_catalog::service::catalog_reads::localize(
        &axum::extract::FromRef::from_ref(a),
        t,
        chain,
        ps,
    )
    .await
}
pub(super) async fn context_info(State(a): State<App>, h: RequestContext) -> Result<Json<Value>> {
    let (locale, chain) = language_context(&a, &h).await?;
    let (settings, _) =
        commerce::scoped_config(&a, &tenant(&h)?, marketing::channel_id(&h)).await?;
    let cart = if header(&h, "sw-context-token").is_some() {
        Some(load_cart(&a, &h).await?)
    } else {
        None
    };
    let currency = settings
        .currencies
        .selected(&currencies::requested(&h, cart.as_ref()))?;
    Ok(Json(
        json!({"locale":locale,"mainLocale":settings.main_locale,"languageIdChain":chain,"availableLocales":settings.locales,"currency":currency.code,"currencyContext":currencies::context(&settings.currencies,currency),"currencies":settings.currencies.enabled,"taxStates":["gross","net"]}),
    ))
}
pub(super) async fn preview_quote(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let t = merchant(&a, &h)?;
    let (locale, chain) = language_context(&a, &h).await?;
    let id = v["productId"].as_str().ok_or(bad("Product ID required"))?;
    let ps = commerce::cart_products(
        &a,
        &t,
        &chain,
        &[Item {
            id: id.into(),
            quantity: 1,
        }],
    )
    .await?;
    let product = ps
        .iter()
        .find(|p| p.id == id)
        .ok_or(bad("Unknown product"))?;
    let requested = v["quantity"]
        .as_u64()
        .filter(|q| *q > 0 && *q <= 10000)
        .ok_or(bad("Quantity must be 1..10000"))? as u32;
    let group = v["customerGroup"].as_str().unwrap_or("consumer");
    let (settings, _) = commerce::config(&a, &t).await?;
    if !settings.customer_groups.iter().any(|g| g.id == group) {
        return Err(bad("Unknown customer group"));
    }
    let quantity = normalized_quantity(product, requested)?;
    let c = StoredCart {
        id: "preview".into(),
        tenant: t,
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
            sales_channel: "default".into(),
            app_configurations: HashMap::new(),
            items: vec![Item {
                id: id.into(),
                quantity,
            }],
            group: group.into(),
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
    };
    Ok(Json(
        json!({"locale":locale,"requestedQuantity":requested,"effectiveQuantity":quantity,"quote":quote(&c,&ps,&settings)?,"sideEffects":false}),
    ))
}
