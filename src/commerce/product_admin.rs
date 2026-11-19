//! Central product list and identity/association writes; all persistence is tenant scoped.
use super::*;
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ProductCriteria {
    #[serde(default)]
    app_filter: String,
    search: Option<String>,
    after: Option<String>,
    limit: Option<i64>,
    active: Option<bool>,
    category_id: Option<String>,
    parent_id: Option<String>,
    low_stock: Option<bool>,
    #[serde(default)]
    preparation: bool,
    #[serde(default)]
    sort: String,
    #[serde(default)]
    direction: String,
}
impl ProductCriteria {
    pub(crate) fn validate(&self) -> Result<i64> {
        let n = self.limit.unwrap_or(25);
        if !(1..=100).contains(&n)
            || self.search.as_ref().is_some_and(|s| s.len() > 200)
            || [&self.after, &self.category_id, &self.parent_id]
                .iter()
                .any(|s| s.as_ref().is_some_and(|v| v.len() > 100))
        {
            return Err(bad("Invalid product filters"));
        }
        apps::extension_filter::decode(&self.app_filter)?;
        self.order()?;
        Ok(n)
    }
    fn order(&self) -> Result<(&'static str, &'static str)> {
        let sort = match self.sort.as_str() {
            "" | "id" => "p.id",
            "name" => "lower(coalesce(grid_tr.name,p.name))",
            "number" => "coalesce(p.product_number,'')",
            "price" => "p.price",
            "stock" => "p.stock",
            _ => return Err(bad("Unsupported product sort")),
        };
        Ok((
            sort,
            super::super::operations::list_direction(&self.direction, "ASC")?,
        ))
    }
}
pub(crate) fn product_admin_router() -> Router<App> {
    Router::new().secure_route(
        "/api/merchant/products",
        &[("GET", "catalog.read"), ("POST", "catalog.write")],
        get(list_products).post(super::create_product),
    )
}
pub(crate) async fn list_products(
    State(a): State<App>,
    h: RequestContext,
    axum::extract::Query(c): axum::extract::Query<ProductCriteria>,
) -> Result<Json<Value>> {
    auth::permit(&h, "catalog.read")?;
    let t = merchant(&a, &h)?;
    let limit = c.validate()?;
    let app_filters = apps::extension_filter::admit(
        &a,
        &h,
        apps::core_extensions::Object::Product,
        &c.app_filter,
    )
    .await?;
    let (sort, direction) = c.order()?;
    let (_, chain) = language_context(&a, &h).await?;
    let pattern = c.search.map(|s| {
        format!(
            "%{}%",
            s.replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_")
        )
    });
    let mut q = sqlx::QueryBuilder::<sqlx::Postgres>::new(
        "SELECT p.*, (SELECT count(*) FROM products c WHERE c.tenant=p.tenant AND c.parent_id=p.id) AS variant_count, ARRAY(SELECT pc.category_id FROM product_categories pc WHERE pc.tenant=p.tenant AND pc.product_id=p.id ORDER BY pc.category_id) AS category_ids FROM products p",
    );
    if c.sort == "name" {
        grid_name_join(&mut q, &chain);
    }
    q.push(" WHERE p.tenant=").push_bind(t.clone());
    if let Some(after) = c.after.filter(|v| !v.is_empty()) {
        q.push(format!(
            " AND ({sort},p.id) {} (SELECT {sort},p.id FROM products p",
            if direction == "ASC" { ">" } else { "<" }
        ));
        if c.sort == "name" {
            grid_name_join(&mut q, &chain);
        }
        q.push(" WHERE p.tenant=")
            .push_bind(t.clone())
            .push(" AND p.id=")
            .push_bind(after)
            .push(")");
    }
    if let Some(active) = c.active {
        q.push(" AND p.active=").push_bind(active);
    }
    if let Some(pattern) = pattern {
        q.push(" AND (lower(p.name) LIKE lower(").push_bind(pattern.clone()).push(") OR lower(p.product_number) LIKE lower(").push_bind(pattern.clone()).push(") OR EXISTS(SELECT 1 FROM product_translations tr WHERE tr.tenant=p.tenant AND tr.product_id=p.id AND lower(coalesce(tr.name,'')) LIKE lower(").push_bind(pattern).push(")))");
    }
    if let Some(category) = c.category_id {
        q.push(" AND EXISTS(SELECT 1 FROM product_categories pc WHERE pc.tenant=p.tenant AND pc.product_id=p.id AND pc.category_id=").push_bind(category).push(")");
    }
    if let Some(parent) = c.parent_id {
        q.push(" AND p.parent_id=").push_bind(parent);
    } else {
        q.push(" AND p.parent_id IS NULL");
    }
    if c.low_stock == Some(true) {
        q.push(" AND p.stock<=5");
    }
    apps::extension_filter::predicate(
        &mut q,
        apps::core_extensions::Object::Product,
        &t,
        &app_filters,
    );
    q.push(format!(
        " ORDER BY {sort} {direction},p.id {direction} LIMIT "
    ))
    .push_bind(limit + 1);
    let rows = q.build().fetch_all(&a.db).await?;
    let translated = localize_products(
        &a,
        &t,
        &chain,
        rows.iter().take(limit as usize).map(product).collect(),
    )
    .await?;
    let has_more = rows.len() > limit as usize;
    let mut elements = vec![];
    for (r, p) in rows.iter().take(limit as usize).zip(translated.iter()) {
        let mut v = json!(p);
        v["active"] = json!(r.get::<bool, _>("active"));
        v["productNumber"] = json!(r.get::<Option<String>, _>("product_number"));
        v["variantCount"] = json!(r.get::<i64, _>("variant_count"));
        v["categoryIds"] = json!(r.get::<Vec<String>, _>("category_ids"));
        elements.push(v);
    }
    apps::core_extensions::attach(
        &a,
        &h,
        apps::core_extensions::Object::Product,
        &mut elements,
    )
    .await?;
    if c.preparation {
        super::product_preparation::attach(&a, &t, &mut elements, &rows).await?;
    }
    let cursor = has_more.then(|| elements.last().unwrap()["id"].clone());
    Ok(Json(
        json!({"elements":elements,"hasMore":has_more,"nextCursor":cursor,"limit":limit}),
    ))
}

/// Only localized-name sorting needs this bounded native translation lookup; other sorts keep their direct path.
fn grid_name_join(q: &mut sqlx::QueryBuilder<sqlx::Postgres>, chain: &[String]) {
    q.push(" LEFT JOIN LATERAL (SELECT name FROM product_translations tr WHERE tr.tenant=p.tenant AND tr.product_id=p.id AND tr.language_id=ANY(").push_bind(chain.to_vec()).push(") AND tr.name IS NOT NULL ORDER BY array_position(").push_bind(chain.to_vec()).push(",tr.language_id) LIMIT 1) grid_tr ON true");
}
