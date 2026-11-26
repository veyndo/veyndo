//! Generic app contributions: configure a cart, bind package/data revisions and persist audited pricing inputs.
use crate::domain::configuration_budget::configuration_budget;
use crate::service::configuration_selection::resolve_selection;
use crate::store::cart_configurations::{
    lock_cart, prepare_configurations, save_cart, validate_configurations,
};
use crate::store::configuration_events::configuration_events;
use serde::Deserialize;
use serde_json::Value;
use veyndo_b2b::store::access::guard_conn;
use veyndo_checkout::domain::cart_quote::configuration_price;
use veyndo_checkout::store::cart_load::load;
use veyndo_core::document::{lenient, view};
use veyndo_core::{RequestContext, Result, State, bad, conflict};
use veyndo_marketing::service::channels::admit_product;
use veyndo_shop::domain::cart_admission::request;
use veyndo_shop::domain::configuration_selection::Selection;
use veyndo_shop::service::read_cache::settings;
use veyndo_shop::types::cart::{Item, StoredCart};

/// The JSON fields `configure` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ConfigureInput {
    #[serde(deserialize_with = "lenient")]
    product_id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    revision: Option<i64>,
    fields: Value,
}

/// Configures one product of the request's cart with an app's inputs, binds package and data revisions, and writes the cart.
pub async fn configure(a: &State, h: &RequestContext, id: &str, v: &Value) -> Result<StoredCart> {
    let input: ConfigureInput = view(v);
    let initial = load(a, h).await?;
    let product = input
        .product_id
        .as_deref()
        .ok_or(bad("productId required"))?;
    admit_product(a, h, product).await?;
    if initial.status != "open" || input.revision != Some(initial.revision) {
        return Err(conflict("Cart changed or terminal"));
    }
    let selection = Selection {
        app: id.into(),
        product_id: product.into(),
        fields: input.fields.clone(),
    };
    let (basis, _) = settings(a, &initial.tenant, "default").await?;
    let config =
        resolve_selection(a, &initial.tenant, &selection, basis.main_locale.clone()).await?;
    let mut candidate = initial.clone();
    candidate.data.app_configurations.remove(product);
    candidate
        .data
        .app_configurations
        .insert(format!("{product}:{id}"), config);
    configuration_budget(&candidate.data.app_configurations)?;
    let (settings, _) = settings(a, &candidate.tenant, &candidate.data.sales_channel).await?;
    let currency = settings.currencies.selected(&candidate.data.currency)?;
    for configuration in candidate.data.app_configurations.values() {
        configuration_price(configuration, &settings.currencies, currency)?;
    }
    let modules = prepare_configurations(a, &candidate).await?;
    let mut tx = a.db.begin().await?;
    let mut cart = lock_cart(&mut tx, &initial.tenant, &initial.token).await?;
    request(h, &cart)?;
    guard_conn(&mut tx, h, &cart).await?;
    if cart.status != "open" || cart.revision != initial.revision {
        return Err(conflict("Cart changed or terminal"));
    }
    let before = cart.data.app_configurations.clone();
    cart.data.app_configurations = candidate.data.app_configurations;
    // Validate even a configuration selected before its product is added to the cart.
    let mut checked = cart.clone();
    if !checked.data.items.iter().any(|item| item.id == product) {
        checked.data.items.push(Item {
            id: product.into(),
            quantity: 1,
        });
    }
    validate_configurations(&mut tx, &checked, &modules).await?;
    cart.revision += 1;
    save_cart(&mut tx, &cart).await?;
    configuration_events(&mut tx, &cart, &before).await?;
    tx.commit().await?;
    Ok(cart)
}
