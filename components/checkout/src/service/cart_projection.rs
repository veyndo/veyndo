//! The priced cart as storefront, Studio and agents receive it: native quote, promotions, app hooks, taxes, legal flags, delivery dates and exact minor totals.
use crate::domain::cart_quote::quote;
use crate::domain::quote_view::{OrderRef, digital, quote_view};
use crate::domain::session_payment::{has_saved_funding, provider, test_environment};
use crate::domain::tax_lines::tax_products;
use crate::http::buyer_identity::browser_checkout_transport;
use crate::ports::Quotes;
use crate::service::delivery::{dates_conn, enrich};
use crate::store::tax_conditions::tax_settings_for_cart;
use serde_json::{Value, json};
use veyndo_core::document::view;
use veyndo_core::{RequestContext, Result, State, bad, process_config};
use veyndo_pricing::service::conversion as currencies;
use veyndo_shop::Settings;
use veyndo_shop::domain::checkout_selection::resolve_selection;
use veyndo_shop::service::language::language_context;
use veyndo_shop::service::read_cache::settings;
use veyndo_shop::types::cart::{StoredCart, selection};
use veyndo_shop::types::legal::version as policy_version;

/// The request context of a stored cart: its tenant and the locale it was priced in.
fn context(c: &StoredCart) -> Result<RequestContext> {
    let mut h = RequestContext::new();
    h.insert(
        "x-tenant",
        c.tenant.parse().map_err(|_| bad("Invalid tenant"))?,
    );
    if !c.data.locale.is_empty() {
        h.insert(
            "x-commerce-locale",
            c.data
                .locale
                .parse()
                .map_err(|_| bad("Invalid stored locale"))?,
        );
    }
    Ok(h)
}

/// The priced cart; a completed cart returns the quote frozen in its order.
pub async fn project(
    a: &State,
    ports: &dyn Quotes,
    c: &StoredCart,
    include_policy_version: bool,
) -> Result<Value> {
    let (locale, chain) = language_context(a, &context(c)?).await?;
    if c.status == "completed"
        && let Some(order) = &c.data.order
    {
        let mut result = view::<OrderRef>(order).cart;
        result["status"] = json!(c.status);
        result["order"] = order.clone();
        return Ok(result);
    }
    let mut ps = ports.products(&c.tenant, &chain, &c.data.items).await?;
    veyndo_b2b::service::quote_contract::prices(&mut *a.db.begin().await?, c, &mut ps).await?;
    let (config, revision) = settings(a, &c.tenant, &c.data.sales_channel).await?;
    let code = config.currencies.selected(&c.data.currency)?.code.clone();
    let (config, paypal_environment) = ports
        .methods(
            &c.tenant,
            &c.data.sales_channel,
            &code,
            config.as_ref().clone(),
        )
        .await?;
    let original = selection(&c.data);
    let selected = resolve_selection(original.clone(), &c.data.group, &config);
    let changed = json!(original) != json!(selected);
    let mut preview = c.clone();
    preview.data.checkout = Some(selected.clone());
    let taxes = tax_settings_for_cart(&mut *a.db.begin().await?, &preview, &ps, &config).await?;
    let ps = tax_products(&ps, &selected, &taxes)?;
    let q = veyndo_marketing::store::promotions::promote(
        &mut *a.db.begin().await?,
        &preview,
        enrich(
            quote(&preview, &ps, &config)?,
            &preview,
            &ps,
            &config,
            revision,
        )?,
    )
    .await?;
    let mut result = enrich(q, &preview, &ps, &config, revision)?;
    result = ports
        .adjust(c, &preview, &ps, result, &config, revision)
        .await?;
    result["selectionNeedsConfirmation"] = json!(changed);
    result["legal"] = json!({"requiresDigital":!config.is_business(&c.data.group) && ps.iter().any(|p|digital(&p.extra)),"strictCheckout":config.legal.strict_checkout});
    if include_policy_version {
        result["legal"]["policyVersion"] = json!(policy_version(&config.legal));
    }
    checkout_service(c, &config, paypal_environment.as_deref(), &mut result);
    finish(a, c, &config, locale, chain, result).await
}

/// The test checkout service a signed-in consumer may use with a simulated or PayPal sandbox method.
fn checkout_service(
    c: &StoredCart,
    config: &Settings,
    paypal_environment: Option<&str>,
    result: &mut Value,
) {
    if c.status != "open"
        || c.data.customer_id.is_none()
        || c.data.admin_actor.is_some()
        || c.data.pos_actor.is_some()
        || config.is_business(&c.data.group)
    {
        return;
    }
    let Some(transport) = browser_checkout_transport(&c.tenant) else {
        return;
    };
    let process = process_config::get();
    let quoted = quote_view(result);
    let simulated = quoted.method().mode == "simulated";
    let paypal_sandbox = process.checkout_paypal_sandbox
        && provider(&quoted.payment_method) == Some("shopware_payments")
        && paypal_environment.is_some_and(|e| test_environment(e, process.strict))
        && !has_saved_funding(c);
    if simulated || paypal_sandbox {
        result["checkoutService"] =
            json!({"legalPolicyVersion":policy_version(&config.legal),"transport":transport});
        if paypal_sandbox {
            result["checkoutService"]["paypalSandbox"] = json!(true);
        }
    }
}

/// Delivery dates, the exact minor total, the company preview, the language and the saved app configurations.
async fn finish(
    a: &State,
    c: &StoredCart,
    config: &Settings,
    locale: String,
    chain: Vec<String>,
    mut result: Value,
) -> Result<Value> {
    dates_conn(&mut *a.db.begin().await?, &mut result).await?;
    let currency = config.currencies.selected(&c.data.currency)?;
    result["price"]["totalMinor"] = json!(
        currencies::amount(
            quote_view(&result)
                .price
                .total_price
                .as_f64()
                .ok_or(bad("Invalid total"))?,
            &currency.code,
            currency.scale
        )?
        .minor()
        .to_string()
    );
    veyndo_b2b::store::purchase::preview(a, c, &mut result).await?;
    result["locale"] = json!(locale);
    result["languageIdChain"] = json!(chain);
    // Restore accepted inputs even before the product is added. Fees and rule
    // snapshots remain native pricing data, not editable form state.
    let mut saved = c.data.app_configurations.values().collect::<Vec<_>>();
    saved.sort_by(|a, b| (&a.app, &a.product_id).cmp(&(&b.app, &b.product_id)));
    result["savedConfigurations"] = json!(
        saved
            .iter()
            .map(|v| json!({"app":v.app,"productId":v.product_id,"fields":v.fields}))
            .collect::<Vec<_>>()
    );
    Ok(result)
}
