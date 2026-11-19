//! Downloads require a paid, owned order snapshot; public attachments are served by the catalog component.
use super::*;
fn response(r: &sqlx::postgres::PgRow) -> Response {
    veyndo_catalog::http::downloads::response(veyndo_catalog::types::asset::StoredFile {
        product_id: String::new(),
        mime: r.get("mime"),
        filename: r.get("filename"),
        content: r.get("content"),
    })
}
fn paid(o: &Value) -> bool {
    verified_kernel::download_admissible(
        ["cancelled", "expired", "payment_review"].contains(&o["state"].as_str().unwrap_or("")),
        ["paid", "captured", "partially_refunded"]
            .contains(&o["payment"]["state"].as_str().unwrap_or("")),
        o["payment"]["provider"] == "simulated" && o["payment"]["state"] == "authorized",
    )
}
pub(super) async fn owned(State(a): State<App>, h: RequestContext) -> Result<Json<Value>> {
    let (t, email) = accounts::identity(&a, &h).await?;
    let (settings, _) = commerce::config(&a, &t).await?;
    let locale = header(&h, "x-commerce-locale").unwrap_or(&settings.main_locale);
    let mut query = sqlx::QueryBuilder::<sqlx::Postgres>::new(
        "SELECT a.id,a.title,a.filename,o.id AS order_id,o.data FROM order_downloads d JOIN product_assets a ON a.tenant=d.tenant AND a.id=d.asset_id JOIN orders o ON o.tenant=d.tenant AND o.id=d.order_id WHERE d.tenant=$1 AND ",
    );
    query
        .push(accounts::ORDER_ACCOUNT_SCOPE)
        .push(" ORDER BY o.created_at DESC LIMIT 200");
    let rows = query.build().bind(t).bind(email).fetch_all(&a.db).await?;
    Ok(Json(
        json!({"elements":rows.iter().filter(|r|paid(&r.get::<Value,_>("data"))).map(|r|json!({"id":r.get::<String,_>("id"),"title":r.get::<Value,_>("title"),"name":commerce::translated_string(&r.get::<Value,_>("title"),locale,&settings.main_locale),"filename":r.get::<String,_>("filename"),"orderId":r.get::<String,_>("order_id")})).collect::<Vec<_>>()}),
    ))
}
pub(super) async fn file(
    State(a): State<App>,
    h: RequestContext,
    Path((order, asset)): Path<(String, String)>,
) -> Result<Response> {
    let t = tenant(&h)?;
    let email = if header(&h, "x-customer-token").is_some() {
        Some(accounts::identity(&a, &h).await?.1)
    } else {
        None
    };
    let token = header(&h, "sw-context-token").unwrap_or("");
    let mut query = sqlx::QueryBuilder::<sqlx::Postgres>::new(
        "SELECT a.mime,a.filename,a.content,o.data FROM order_downloads d JOIN product_assets a ON a.tenant=d.tenant AND a.id=d.asset_id JOIN orders o ON o.tenant=d.tenant AND o.id=d.order_id JOIN carts c ON c.id=o.cart_id AND c.tenant=o.tenant WHERE d.tenant=$1 AND d.order_id=$3 AND d.asset_id=$4 AND (($2::text IS NOT NULL AND ",
    );
    query
        .push(accounts::ORDER_ACCOUNT_SCOPE)
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
