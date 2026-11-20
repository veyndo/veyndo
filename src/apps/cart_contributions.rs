//! Generic app contributions: configure a cart, bind package/data revisions and persist audited pricing inputs.
use super::*;
pub(crate) use veyndo_shop::types::cart::AppConfiguration as Configuration;
pub(crate) async fn configure(a: &App, h: &RequestContext, id: &str, v: &Value) -> Result<Value> {
    let initial = load_cart(a, h).await?;
    let product = v["productId"].as_str().ok_or(bad("productId required"))?;
    marketing::admit_product(a, h, product).await?;
    if initial.status != "open" || v["revision"].as_i64() != Some(initial.revision) {
        return Err(conflict("Cart changed or terminal"));
    }
    let selection = configuration_selection::Selection {
        app: id.into(),
        product_id: product.into(),
        fields: v["fields"].clone(),
    };
    let config = configuration_selection::resolve_selection(a, &initial.tenant, &selection).await?;
    let mut candidate = initial.clone();
    candidate.data.app_configurations.remove(product);
    candidate
        .data
        .app_configurations
        .insert(format!("{product}:{id}"), config);
    configuration_budget(&candidate.data.app_configurations)?;
    let (settings, _) =
        commerce::scoped_config(a, &candidate.tenant, &candidate.data.sales_channel).await?;
    let currency = settings.currencies.selected(&candidate.data.currency)?;
    for configuration in candidate.data.app_configurations.values() {
        crate::cart_price::configuration_price(configuration, &settings.currencies, currency)?;
    }
    let modules = prepare_configurations(a, &candidate).await?;
    let mut tx = a.db.begin().await?;
    let row = sqlx::query("SELECT * FROM carts WHERE tenant=$1 AND token=$2 FOR UPDATE")
        .bind(&initial.tenant)
        .bind(&initial.token)
        .fetch_one(&mut *tx)
        .await?;
    let mut cart = stored(&row)?;
    pos::guard_cart(h, &cart)?;
    b2b::guard_conn(&mut tx, h, &cart).await?;
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
    sqlx::query("UPDATE carts SET data=$1,revision=$2 WHERE id=$3")
        .bind(json!(cart.data))
        .bind(cart.revision)
        .bind(&cart.id)
        .execute(&mut *tx)
        .await?;
    configuration_events(&mut tx, &cart, &before).await?;
    tx.commit().await?;
    cart_json(a, &cart).await
}
pub(crate) use veyndo_app_platform::store::cart_configurations::{
    prepare_configurations, validate_configurations,
};
