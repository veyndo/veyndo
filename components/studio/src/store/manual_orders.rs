//! SQL of merchant manual orders: the active customer a draft is for, its admin cart context and the selectable sales channels.
use serde_json::Value;
use veyndo_core::{Result, State};

/// The cart-relevant identity of an active customer.
pub struct Customer {
    pub id: String,
    pub email: String,
    pub group: String,
    pub company: Option<String>,
}

/// An active customer by ID.
pub async fn active_customer(a: &State, t: &str, id: &str) -> Result<Option<Customer>> {
    Ok(sqlx::query_as!(
        Customer,
        r#"SELECT id,email,group_name AS "group",company FROM customers WHERE tenant=$1 AND id=$2 AND active"#,
        t,
        id
    )
    .fetch_optional(&a.db)
    .await?)
}

/// Replaces a cart's data document.
pub async fn set_cart_data(a: &State, t: &str, cart: &str, data: &Value) -> Result<()> {
    sqlx::query!(
        "UPDATE carts SET data=$1 WHERE tenant=$2 AND id=$3",
        data,
        t,
        cart
    )
    .execute(&a.db)
    .await?;
    Ok(())
}

/// Up to 100 active public storefront/API channels as `{id,name}`.
pub async fn channels(a: &State, t: &str) -> Result<Vec<Value>> {
    Ok(sqlx::query_scalar!(r#"SELECT jsonb_build_object('id',id,'name',data->'name') AS "value!" FROM sales_channels WHERE tenant=$1 AND data->>'kind' IN ('storefront','headless') AND data->>'active'='true' AND coalesce(data->>'visibility','public')='public' ORDER BY id LIMIT 100"#, t).fetch_all(&a.db).await?)
}
