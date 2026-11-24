//! Downloads require a paid, owned order snapshot; public attachments are served by the catalog component.
use crate::store::account_orders::ACCOUNT_SCOPE;
use axum::Json;
use axum::extract::{Path, State as Shared};
use axum::response::Response;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use veyndo_catalog::domain::content::translated_string;
use veyndo_core::document::{lenient, view};
use veyndo_core::shop_scope::{header, session_digest, tenant};
use veyndo_core::{Error, RequestContext, Result, State, StatusCode};
use veyndo_customers::service::sessions::identify;
use veyndo_kernel::verified_kernel;
use veyndo_shop::service::read_cache::settings;

/// The signed-in customer of the request and their shop.
async fn customer(a: &State, h: &RequestContext) -> Result<(String, String)> {
    let t = tenant(h)?;
    let email = identify(a, &t, &session_digest(h)?).await?;
    Ok((t, email))
}
/// The file as an attachment response.
fn response(r: &sqlx::postgres::PgRow) -> Response {
    veyndo_catalog::http::downloads::response(veyndo_catalog::types::asset::StoredFile {
        product_id: String::new(),
        mime: r.get("mime"),
        filename: r.get("filename"),
        content: r.get("content"),
    })
}
/// Whether the order's payment unlocks its downloads.
fn paid(o: &Value) -> bool {
    let o: DownloadOrder = view(o);
    let state = o.payment.state.as_str().unwrap_or("");
    verified_kernel::download_admissible(
        ["cancelled", "expired", "payment_review"].contains(&o.state.as_str().unwrap_or("")),
        ["paid", "captured", "partially_refunded"].contains(&state),
        o.payment.provider == "simulated" && o.payment.state == "authorized",
    )
}

/// The order facts that unlock its downloads.
#[derive(Deserialize, Default)]
#[serde(default)]
struct DownloadOrder {
    state: Value,
    #[serde(deserialize_with = "lenient")]
    payment: DownloadPayment,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct DownloadPayment {
    state: Value,
    provider: Value,
}
/// `GET /store-api/account/downloads`: the downloads of the customer's paid orders.
pub async fn owned(Shared(a): Shared<State>, h: RequestContext) -> Result<Json<Value>> {
    let (t, email) = customer(&a, &h).await?;
    let (settings, _) = settings(&a, &t, "default").await?;
    let locale = header(&h, "x-commerce-locale").unwrap_or(&settings.main_locale);
    let mut query = sqlx::QueryBuilder::<sqlx::Postgres>::new(
        "SELECT a.id,a.title,a.filename,o.id AS order_id,o.data FROM order_downloads d JOIN product_assets a ON a.tenant=d.tenant AND a.id=d.asset_id JOIN orders o ON o.tenant=d.tenant AND o.id=d.order_id WHERE d.tenant=$1 AND ",
    );
    query
        .push(ACCOUNT_SCOPE)
        .push(" ORDER BY o.created_at DESC LIMIT 200");
    let rows = query.build().bind(t).bind(email).fetch_all(&a.db).await?;
    Ok(Json(
        json!({"elements":rows.iter().filter(|r|paid(&r.get::<Value,_>("data"))).map(|r|json!({"id":r.get::<String,_>("id"),"title":r.get::<Value,_>("title"),"name":translated_string(&r.get::<Value,_>("title"),locale,&settings.main_locale),"filename":r.get::<String,_>("filename"),"orderId":r.get::<String,_>("order_id")})).collect::<Vec<_>>()}),
    ))
}
/// `GET /store-api/orders/{order}/downloads/{asset}`: one file of a paid order, for its customer or its guest cart.
pub async fn file(
    Shared(a): Shared<State>,
    h: RequestContext,
    Path((order, asset)): Path<(String, String)>,
) -> Result<Response> {
    let t = tenant(&h)?;
    let email = if header(&h, "x-customer-token").is_some() {
        Some(customer(&a, &h).await?.1)
    } else {
        None
    };
    let token = header(&h, "sw-context-token").unwrap_or("");
    let mut query = sqlx::QueryBuilder::<sqlx::Postgres>::new(
        "SELECT a.mime,a.filename,a.content,o.data FROM order_downloads d JOIN product_assets a ON a.tenant=d.tenant AND a.id=d.asset_id JOIN orders o ON o.tenant=d.tenant AND o.id=d.order_id JOIN carts c ON c.id=o.cart_id AND c.tenant=o.tenant WHERE d.tenant=$1 AND d.order_id=$3 AND d.asset_id=$4 AND (($2::text IS NOT NULL AND ",
    );
    query
        .push(ACCOUNT_SCOPE)
        .push(") OR (length($5)>0 AND c.token=$5 AND c.data->>'admin_actor' IS NULL AND c.data->>'pos_actor' IS NULL))");
    let r = query
        .build()
        .bind(t)
        .bind(email)
        .bind(order)
        .bind(asset)
        .bind(token)
        .fetch_optional(&a.db)
        .await?
        .ok_or(Error(StatusCode::NOT_FOUND, "Download not found".into()))?;
    if !paid(&r.get::<Value, _>("data")) {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Download requires a confirmed payment".into(),
        ));
    }
    Ok(response(&r))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn payment_gate() {
        assert!(paid(
            &json!({"state":"placed","payment":{"provider":"simulated","state":"authorized"}})
        ));
        assert!(!paid(
            &json!({"state":"cancelled","payment":{"state":"captured"}})
        ));
        assert!(!paid(
            &json!({"state":"placed","payment":{"provider":"paypal","state":"approved"}})
        ));
    }
}
