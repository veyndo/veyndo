//! Authoritative quantity pricing and localized checkout quote assembly.
use crate::*;

pub(crate) use veyndo_checkout::domain::cart_quote::{
    configuration_price, normalized_quantity, quote,
};
pub(crate) async fn cart_json(a: &App, c: &StoredCart) -> Result<Value> {
    project_cart(a, c, false).await
}

pub(crate) async fn project_cart(
    a: &App,
    c: &StoredCart,
    include_policy_version: bool,
) -> Result<Value> {
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
    let (locale, chain) = language_context(a, &h).await?;
    if c.status == "completed"
        && let Some(order) = &c.data.order
    {
        let mut result = order["cart"].clone();
        result["status"] = json!(c.status);
        result["order"] = order.clone();
        return Ok(result);
    }
    let mut ps = commerce::cart_products(a, &c.tenant, &chain, &c.data.items).await?;
    b2b::contract_prices(&mut *a.db.begin().await?, c, &mut ps).await?;
    let (config, revision) = commerce::scoped_config(a, &c.tenant, &c.data.sales_channel).await?;
    let code = config.currencies.selected(&c.data.currency)?.code.clone();
    let (config, paypal_environment) = payments::currency_methods(
        a,
        &c.tenant,
        &c.data.sales_channel,
        &code,
        config.as_ref().clone(),
    )
    .await?;
    let original = commerce::selection(&c.data);
    let selected = commerce::resolve_selection(original.clone(), &c.data.group, &config);
    let changed = json!(original) != json!(selected);
    let mut preview = c.clone();
    preview.data.checkout = Some(selected.clone());
    let taxes =
        commerce::tax_settings_for_cart(&mut *a.db.begin().await?, &preview, &ps, &config).await?;
    let ps = commerce::tax_products(&ps, &selected, &taxes)?;
    let hooks = apps::prepare_hooks(a, &c.tenant).await?;
    let q = marketing::promote(
        &mut *a.db.begin().await?,
        &preview,
        commerce::enrich(
            quote(&preview, &ps, &config)?,
            &preview,
            &ps,
            &config,
            revision,
        )?,
    )
    .await?;
    let mut result = commerce::enrich(q, &preview, &ps, &config, revision)?;
    result = apps::apply_hooks(
        &mut *a.db.begin().await?,
        &preview,
        &ps,
        result,
        &config,
        revision,
        &hooks,
    )
    .await?;
    result = pos::adjust_quote(c, result, &ps, &config, revision)?;
    result["selectionNeedsConfirmation"] = json!(changed);
    result["legal"] = json!({"requiresDigital":!config.is_business(&c.data.group) && ps.iter().any(|p|p.extra["digital"]==true),"strictCheckout":config.legal.strict_checkout});
    if include_policy_version {
        result["legal"]["policyVersion"] = json!(legal::policy_version(&config.legal));
    }
    if c.status == "open"
        && c.data.customer_id.is_some()
        && c.data.admin_actor.is_none()
        && c.data.pos_actor.is_none()
        && !config.is_business(&c.data.group)
        && let Some(transport) = checkout::buyer_identity::browser_checkout_transport(&c.tenant)
    {
        let simulated = result["paymentMethod"]["mode"] == "simulated";
        let paypal_sandbox = runtime_config::get().checkout_paypal_sandbox
            && checkout::session_payment::provider(&result["paymentMethod"])
                == Some("shopware_payments")
            && paypal_environment
                .as_deref()
                .is_some_and(checkout::session_payment::test_environment)
            && !checkout::session_payment::has_saved_funding(c);
        if simulated || paypal_sandbox {
            result["checkoutService"] = json!({"legalPolicyVersion":legal::policy_version(&config.legal),"transport":transport});
            if paypal_sandbox {
                result["checkoutService"]["paypalSandbox"] = json!(true);
            }
        }
    }
    commerce::dates(a, &mut result).await?;
    let currency = config.currencies.selected(&c.data.currency)?;
    result["price"]["totalMinor"] = json!(
        currencies::amount(
            result["price"]["totalPrice"]
                .as_f64()
                .ok_or(bad("Invalid total"))?,
            &currency.code,
            currency.scale
        )?
        .minor()
        .to_string()
    );
    b2b::preview(a, c, &mut result).await?;
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
