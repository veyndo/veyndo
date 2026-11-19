//! The keyset-paged merchant product list with its filters; one extra row tells whether more follow.
use crate::store::products::product;
use crate::types::product_list::{Listed, ProductCriteria};
use sqlx::{Postgres, QueryBuilder, Row};
use veyndo_core::{Result, State};

/// Only localized-name sorting needs this bounded native translation lookup; other sorts keep their direct path.
fn grid_name_join(q: &mut QueryBuilder<Postgres>, chain: &[String]) {
    q.push(" LEFT JOIN LATERAL (SELECT name FROM product_translations tr WHERE tr.tenant=p.tenant AND tr.product_id=p.id AND tr.language_id=ANY(").push_bind(chain.to_vec()).push(") AND tr.name IS NOT NULL ORDER BY array_position(").push_bind(chain.to_vec()).push(",tr.language_id) LIMIT 1) grid_tr ON true");
}

/// Up to `limit + 1` products after the cursor. `extend` may add further predicates, such as app filters.
pub async fn page(
    a: &State,
    tenant: &str,
    c: &ProductCriteria,
    limit: i64,
    chain: &[String],
    extend: impl FnOnce(&mut QueryBuilder<Postgres>),
) -> Result<Vec<Listed>> {
    let (sort, direction) = c.order()?;
    let by_name = c.sort == "name";
    let mut q = QueryBuilder::<Postgres>::new(
        "SELECT p.*, (SELECT count(*) FROM products c WHERE c.tenant=p.tenant AND c.parent_id=p.id) AS variant_count, ARRAY(SELECT pc.category_id FROM product_categories pc WHERE pc.tenant=p.tenant AND pc.product_id=p.id ORDER BY pc.category_id) AS category_ids FROM products p",
    );
    if by_name {
        grid_name_join(&mut q, chain);
    }
    q.push(" WHERE p.tenant=").push_bind(tenant.to_owned());
    if let Some(after) = c.after.clone().filter(|v| !v.is_empty()) {
        q.push(format!(
            " AND ({sort},p.id) {} (SELECT {sort},p.id FROM products p",
            if direction == "ASC" { ">" } else { "<" }
        ));
        if by_name {
            grid_name_join(&mut q, chain);
        }
        q.push(" WHERE p.tenant=")
            .push_bind(tenant.to_owned())
            .push(" AND p.id=")
            .push_bind(after)
            .push(")");
    }
    if let Some(active) = c.active {
        q.push(" AND p.active=").push_bind(active);
    }
    if let Some(pattern) = c.pattern() {
        q.push(" AND (lower(p.name) LIKE lower(").push_bind(pattern.clone()).push(") OR lower(p.product_number) LIKE lower(").push_bind(pattern.clone()).push(") OR EXISTS(SELECT 1 FROM product_translations tr WHERE tr.tenant=p.tenant AND tr.product_id=p.id AND lower(coalesce(tr.name,'')) LIKE lower(").push_bind(pattern).push(")))");
    }
    if let Some(category) = c.category_id.clone() {
        q.push(" AND EXISTS(SELECT 1 FROM product_categories pc WHERE pc.tenant=p.tenant AND pc.product_id=p.id AND pc.category_id=").push_bind(category).push(")");
    }
    if let Some(parent) = c.parent_id.clone() {
        q.push(" AND p.parent_id=").push_bind(parent);
    } else {
        q.push(" AND p.parent_id IS NULL");
    }
    if c.low_stock == Some(true) {
        q.push(" AND p.stock<=5");
    }
    extend(&mut q);
    q.push(format!(
        " ORDER BY {sort} {direction},p.id {direction} LIMIT "
    ))
    .push_bind(limit + 1);
    Ok(q.build()
        .fetch_all(&a.db)
        .await?
        .iter()
        .map(|r| Listed {
            product: product(r),
            active: r.get("active"),
            product_number: r.get("product_number"),
            variant_count: r.get("variant_count"),
            category_ids: r.get("category_ids"),
        })
        .collect())
}
