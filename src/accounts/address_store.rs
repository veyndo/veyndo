//! Entry points of the address book: country admission and history attribution here, persistence in the customers component.
use crate::*;
use veyndo_customers::service::address_book;
pub(crate) async fn address_list(a: &App, t: &str, email: &str) -> Result<Value> {
    Ok(json!(address_book::list(&a.core, t, email).await?))
}
pub(crate) async fn address_save(
    a: &App,
    t: &str,
    email: &str,
    id: Option<&str>,
    v: &Value,
    h: &RequestContext,
) -> Result<Value> {
    let mut address: commerce::Address =
        serde_json::from_value(v["address"].clone()).map_err(|_| bad("Invalid address"))?;
    address.validate()?;
    let (s, _) = commerce::config(a, t).await?;
    commerce::validate_address_geography(&address, &s)?;
    if !s.countries.contains(&address.country) {
        return Err(bad("Address country unavailable"));
    }
    let mut tx = a.db.begin().await?;
    if header(h, "x-rac-user").is_some() {
        history::context(&mut tx, h, "merchant").await?;
    } else {
        history::customer_context(&mut tx, email).await?;
    }
    let result = address_save_conn(&mut tx, t, email, id, v, &address).await?;
    tx.commit().await?;
    Ok(result)
}
pub(crate) async fn address_save_conn(
    conn: &mut sqlx::PgConnection,
    t: &str,
    email: &str,
    id: Option<&str>,
    v: &Value,
    address: &commerce::Address,
) -> Result<Value> {
    address_store_conn(conn, t, email, id, v, address).await
}
async fn address_store_conn(
    conn: &mut sqlx::PgConnection,
    t: &str,
    email: &str,
    id: Option<&str>,
    v: &Value,
    address: &commerce::Address,
) -> Result<Value> {
    let address_id = address_book::save(
        conn,
        t,
        email,
        address_book::Save {
            id,
            revision: v["revision"].as_i64(),
            default_billing: v["defaultBilling"].as_bool().unwrap_or(false),
            default_shipping: v["defaultShipping"].as_bool().unwrap_or(false),
            announce: true,
        },
        address,
    )
    .await?;
    Ok(json!({"id":address_id,"saved":true}))
}
pub(crate) async fn address_delete(
    a: &App,
    t: &str,
    email: &str,
    id: &str,
    revision: i64,
    h: &RequestContext,
) -> Result<Value> {
    let mut tx = a.db.begin().await?;
    if header(h, "x-rac-user").is_some() {
        history::context(&mut tx, h, "merchant").await?;
    } else {
        history::customer_context(&mut tx, email).await?;
    }
    address_book::delete(&mut tx, t, email, id, revision).await?;
    tx.commit().await?;
    Ok(json!({"deleted":true}))
}
pub(crate) async fn address_get(
    conn: &mut sqlx::PgConnection,
    t: &str,
    email: &str,
    id: &str,
) -> Result<commerce::Address> {
    address_book::get(conn, t, email, id).await
}
