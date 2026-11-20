//! Product-centred canonical facts and tenant-filtered graph evidence, independent of the overview's truncated sample.
use super::*;
pub(crate) async fn product_knowledge(
    State(a): State<App>,
    h: RequestContext,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    let t = merchant(&a, &h)?;
    auth::permit(&h, "knowledge.read")?;
    let row = sqlx::query("SELECT * FROM products WHERE tenant=$1 AND id=$2")
        .bind(&t)
        .bind(&id)
        .fetch_optional(&a.db)
        .await?
        .ok_or(Error(StatusCode::NOT_FOUND, "Product unavailable".into()))?;
    let (locale, chain) = language_context(&a, &h).await?;
    let product = localize_products(&a, &t, &chain, vec![crate::product(&row)])
        .await?
        .remove(0);
    let (settings, _) = commerce::config(&a, &t).await?;
    let f = veyndo_intelligence::store::documents::product_facts(
        &a,
        &t,
        &id,
        row.get("parent_id"),
        &locale,
        &settings.main_locale,
    )
    .await?;
    let external = apps::private_graph(&a, &t).await?;
    let refs = external["products"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|p| p["productId"] == id)
        .cloned()
        .collect::<Vec<_>>();
    Ok(Json(
        json!({"product":product,"stats":f.stats,"needs":f.needs,"complements":f.complements,"observed":f.observed,"sources":f.sources,"external":refs,"limit":50,"catalogueDerived":true}),
    ))
}
