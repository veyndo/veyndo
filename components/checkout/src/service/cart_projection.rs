//! The priced cart as storefront, Studio and agents receive it: native quote, promotions, app hooks, taxes, legal flags, delivery dates and exact minor totals.
use crate::domain::cart_quote::quote;
use crate::domain::quote_view::{OrderRef, digital, quote_view};
use crate::domain::session_payment::{has_saved_funding, provider, test_environment};
use crate::domain::tax_lines::tax_products;
use crate::http::buyer_identity::browser_checkout_transport;
use crate::ports::Quotes;
use crate::service::delivery::{dates_conn, enrich};
use crate::store::tax_conditions::tax_settings_for_cart;
use serde::Serialize;
use serde_json::{Value, json};
use veyndo_core::document::{merge, view};
use veyndo_core::document_slot::field;
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
        merge(
            &mut result,
            CompletedCart {
                status: &c.status,
                order,
            },
        )?;
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
    merge(
        &mut result,
        CartLegalState {
            selection_needs_confirmation: changed,
            legal: CartLegal {
                requires_digital: !config.is_business(&c.data.group)
                    && ps.iter().any(|p| digital(&p.extra)),
                strict_checkout: config.legal.strict_checkout,
                policy_version: include_policy_version.then(|| policy_version(&config.legal)),
            },
        },
    )?;
    checkout_service(c, &config, paypal_environment.as_deref(), &mut result)?;
    finish(a, c, &config, locale, chain, result).await
}

/// The test checkout service a signed-in consumer may use with a simulated or PayPal sandbox method.
fn checkout_service(
    c: &StoredCart,
    config: &Settings,
    paypal_environment: Option<&str>,
    result: &mut Value,
) -> Result<()> {
    if c.status != "open"
        || c.data.customer_id.is_none()
        || c.data.admin_actor.is_some()
        || c.data.pos_actor.is_some()
        || config.is_business(&c.data.group)
    {
        return Ok(());
    }
    let Some(transport) = browser_checkout_transport(&c.tenant) else {
        return Ok(());
    };
    let process = process_config::get();
    let quoted = quote_view(result);
    let simulated = quoted.method().mode == "simulated";
    let paypal_sandbox = process.checkout_paypal_sandbox
        && provider(&quoted.payment_method) == Some("shopware_payments")
        && paypal_environment.is_some_and(|e| test_environment(e, process.strict))
        && !has_saved_funding(c);
    if simulated || paypal_sandbox {
        merge(
            result,
            OfferedService {
                checkout_service: TestCheckoutService {
                    legal_policy_version: policy_version(&config.legal),
                    transport,
                    paypal_sandbox: paypal_sandbox.then_some(true),
                },
            },
        )?;
    }
    Ok(())
}

/// A completed cart answers with the quote frozen in its order.
#[derive(Serialize)]
struct CompletedCart<'a> {
    status: &'a str,
    order: &'a Value,
}

/// Whether the resolved selection changed, and what the legal checkout requires.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CartLegalState {
    selection_needs_confirmation: bool,
    legal: CartLegal,
}

/// The legal requirements of the cart; the policy version only when asked for.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CartLegal {
    requires_digital: bool,
    strict_checkout: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    policy_version: Option<String>,
}

/// The test checkout service a signed-in consumer may use.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OfferedService {
    checkout_service: TestCheckoutService,
}

/// The policy version and transport of the test checkout service; `paypalSandbox` only for the PayPal sandbox.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TestCheckoutService {
    legal_policy_version: String,
    transport: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    paypal_sandbox: Option<bool>,
}

/// The exact minor total of the cart price.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MinorTotal {
    total_minor: String,
}

/// The language a cart is shown in and the accepted app configuration inputs it restores.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CartLanguage<'a> {
    locale: String,
    language_id_chain: Vec<String>,
    saved_configurations: Vec<SavedConfiguration<'a>>,
}

/// One accepted app configuration input of a cart line.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SavedConfiguration<'a> {
    app: &'a str,
    product_id: &'a str,
    fields: &'a Value,
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
    let total_minor = currencies::amount(
        quote_view(&result)
            .price
            .total_price
            .as_f64()
            .ok_or(bad("Invalid total"))?,
        &currency.code,
        currency.scale,
    )?
    .minor()
    .to_string();
    merge(field(&mut result, "price")?, MinorTotal { total_minor })?;
    veyndo_b2b::store::purchase::preview(a, c, &mut result).await?;
    // Restore accepted inputs even before the product is added. Fees and rule
    // snapshots remain native pricing data, not editable form state.
    let mut saved = c.data.app_configurations.values().collect::<Vec<_>>();
    saved.sort_by(|a, b| (&a.app, &a.product_id).cmp(&(&b.app, &b.product_id)));
    merge(
        &mut result,
        CartLanguage {
            locale,
            language_id_chain: chain,
            saved_configurations: saved
                .iter()
                .map(|v| SavedConfiguration {
                    app: &v.app,
                    product_id: &v.product_id,
                    fields: &v.fields,
                })
                .collect(),
        },
    )?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::{
        CartLanguage, CartLegal, CartLegalState, CompletedCart, MinorTotal, OfferedService,
        SavedConfiguration, TestCheckoutService,
    };
    use serde_json::json;
    use veyndo_core::document::merge;
    use veyndo_core::document_slot::field;

    /// The typed cart projection patches write exactly what the indexed assignments wrote.
    #[test]
    fn typed_cart_patches_match_the_indexed_writes() {
        let order = json!({"id":"o1","cart":{"id":"c1"}});
        let mut indexed = json!({"id":"c1"});
        indexed["status"] = json!("completed");
        indexed["order"] = order.clone();
        let mut typed = json!({"id":"c1"});
        merge(
            &mut typed,
            CompletedCart {
                status: "completed",
                order: &order,
            },
        )
        .unwrap();
        assert_eq!(typed, indexed);
        for (policy, sandbox) in [(true, true), (false, false)] {
            let base = json!({"id":"c1","price":{"totalPrice":12.5}});
            let mut indexed = base.clone();
            indexed["selectionNeedsConfirmation"] = json!(true);
            indexed["legal"] = json!({"requiresDigital":false,"strictCheckout":true});
            if policy {
                indexed["legal"]["policyVersion"] = json!("v1");
            }
            indexed["checkoutService"] = json!({"legalPolicyVersion":"v1","transport":"redirect"});
            if sandbox {
                indexed["checkoutService"]["paypalSandbox"] = json!(true);
            }
            indexed["price"]["totalMinor"] = json!("1250");
            indexed["locale"] = json!("de-DE");
            indexed["languageIdChain"] = json!(["de-DE", "en-GB"]);
            indexed["savedConfigurations"] = json!([{"app":"a","productId":"p","fields":{"x":1}}]);
            let mut typed = base.clone();
            merge(
                &mut typed,
                CartLegalState {
                    selection_needs_confirmation: true,
                    legal: CartLegal {
                        requires_digital: false,
                        strict_checkout: true,
                        policy_version: policy.then(|| "v1".to_string()),
                    },
                },
            )
            .unwrap();
            merge(
                &mut typed,
                OfferedService {
                    checkout_service: TestCheckoutService {
                        legal_policy_version: "v1".into(),
                        transport: "redirect",
                        paypal_sandbox: sandbox.then_some(true),
                    },
                },
            )
            .unwrap();
            merge(
                field(&mut typed, "price").unwrap(),
                MinorTotal {
                    total_minor: "1250".into(),
                },
            )
            .unwrap();
            let fields = json!({"x":1});
            merge(
                &mut typed,
                CartLanguage {
                    locale: "de-DE".into(),
                    language_id_chain: vec!["de-DE".into(), "en-GB".into()],
                    saved_configurations: vec![SavedConfiguration {
                        app: "a",
                        product_id: "p",
                        fields: &fields,
                    }],
                },
            )
            .unwrap();
            assert_eq!(typed, indexed);
        }
    }
}
