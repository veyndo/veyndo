//! Localized immutable checkout SKU snapshots read under the purchase transaction's locks.
use sqlx::Row;
use veyndo_catalog::domain::family::inherited_extra;
use veyndo_catalog::store::products::product;
use veyndo_core::Result;
use veyndo_pricing::catalog::Product;
use veyndo_shop::types::cart::StoredCart;

pub async fn snapshot(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    c: &StoredCart,
    rows: Vec<sqlx::postgres::PgRow>,
    chain: &[String],
    locales: &[String],
) -> Result<Vec<Product>> {
    let ids = rows
        .iter()
        .map(|r| r.get::<String, _>("id"))
        .collect::<Vec<_>>();
    let translations =
        veyndo_catalog::store::translations::lock_for_products(tx, &c.tenant, chain, &ids).await?;
    let locked = rows.iter().map(product).collect::<Vec<_>>();
    let roots = veyndo_catalog::domain::hydration::hydrate(
        locked
            .iter()
            .filter(|p| p.parent_id.is_none())
            .cloned()
            .collect(),
        &translations,
        chain,
        locales,
    );
    let selected = locked
        .into_iter()
        .filter(|p| c.data.items.iter().any(|i| i.id == p.id))
        .map(|mut p| {
            if let Some(parent) = roots.iter().find(|r| Some(&r.id) == p.parent_id.as_ref()) {
                p.name = parent.name.clone();
                p.description = parent.description.clone();
                p.extra = inherited_extra(&parent.extra, &p.extra);
            }
            p
        })
        .collect();
    Ok(veyndo_catalog::domain::hydration::hydrate(
        selected,
        &translations,
        chain,
        locales,
    ))
}
