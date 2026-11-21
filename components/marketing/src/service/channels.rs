//! Sales channels share a shop but bind their own catalog visibility, languages and cart identity; storefront reads pass through them.
use crate::domain::channel_admission::{admissible, selected, serves};
use crate::types::channel::Channel;
use sqlx::Row;
use std::collections::HashSet;
use veyndo_core::shop_scope::{channel as channel_id, header, tenant};
use veyndo_core::{Error, RequestContext, Result, State, StatusCode, bad};
use veyndo_pricing::catalog::Product;
use veyndo_shop::service::language::language_context;
use veyndo_shop::service::read_cache::config;

/// The configuration of a channel as this request may see it; the main channel inherits the shop languages.
pub async fn channel_configuration(a: &State, h: &RequestContext, id: &str) -> Result<Channel> {
    let t = tenant(h)?;
    let data: serde_json::Value =
        sqlx::query_scalar("SELECT data FROM sales_channels WHERE tenant=$1 AND id=$2")
            .bind(&t)
            .bind(id)
            .fetch_optional(&a.db)
            .await?
            .ok_or(Error(
                StatusCode::NOT_FOUND,
                "Sales channel unavailable".into(),
            ))?;
    let mut c: Channel = serde_json::from_value(data).map_err(|_| bad("Invalid channel"))?;
    if id == "default" {
        c.locales = config(a, &t).await?.0.locales.clone();
    }
    admissible(&c, id, h.pos.is_some(), header(h, "x-rac-channel-preview"))?;
    Ok(c)
}

/// The channel when it serves the locale; otherwise the request is refused.
pub async fn channel(
    a: &State,
    h: &RequestContext,
    id: &str,
    locale: &str,
) -> Result<Option<Channel>> {
    let c = channel_configuration(a, h, id).await?;
    if !serves(&c, id, locale) {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Sales channel or language unavailable".into(),
        ));
    }
    Ok(Some(c))
}

async fn current(a: &State, h: &RequestContext) -> Result<Option<Channel>> {
    channel(a, h, channel_id(h), &language_context(a, h).await?.0).await
}

/// Admits one product in the current channel.
pub async fn admit_product(a: &State, h: &RequestContext, id: &str) -> Result<()> {
    admit_products(a, h, &[id.to_owned()]).await
}

/// Admits up to 24 products with the same parent, visibility, channel-list and company-catalog rules as a single read.
pub async fn admit_products(a: &State, h: &RequestContext, ids: &[String]) -> Result<()> {
    if ids.is_empty() || ids.len() > 24 {
        return Err(bad("Product admission batch must contain 1..24 IDs"));
    }
    veyndo_b2b::store::catalog::admit(a, h, ids).await?;
    let rows=sqlx::query("SELECT p.id,p.parent_id FROM products p WHERE p.tenant=$1 AND p.id=ANY($3) AND p.active AND (p.parent_id IS NULL OR EXISTS(SELECT 1 FROM products parent WHERE parent.tenant=p.tenant AND parent.id=p.parent_id AND parent.active)) AND NOT EXISTS(SELECT 1 FROM product_channel_visibility v WHERE v.tenant=p.tenant AND v.channel_id=$2 AND NOT v.visible AND (v.product_id=p.id OR v.product_id=p.parent_id))").bind(tenant(h)?).bind(channel_id(h)).bind(ids).fetch_all(&a.db).await?;
    let selection = current(a, h)
        .await?
        .map(|c| c.product_ids)
        .unwrap_or_default();
    let denied = ids.iter().any(|id| {
        !rows.iter().any(|r| {
            r.get::<String, _>("id") == *id
                && selected(
                    &selection,
                    id,
                    r.get::<Option<String>, _>("parent_id").as_deref(),
                )
        })
    });
    if denied {
        return Err(Error(
            StatusCode::NOT_FOUND,
            "Product unavailable in sales channel".into(),
        ));
    }
    Ok(())
}

/// Keeps the products the current channel and company catalog show.
pub async fn filter_channel(
    a: &State,
    h: &RequestContext,
    ps: Vec<Product>,
) -> Result<Vec<Product>> {
    let config = current(a, h).await?;
    let ids = ps.iter().map(|p| p.id.clone()).collect::<Vec<_>>();
    let admitted: Vec<String> = sqlx::query_scalar("SELECT p.id FROM products p WHERE p.tenant=$1 AND p.id=ANY($2) AND p.active AND (p.parent_id IS NULL OR EXISTS(SELECT 1 FROM products parent WHERE parent.tenant=p.tenant AND parent.id=p.parent_id AND parent.active)) AND NOT EXISTS(SELECT 1 FROM product_channel_visibility v WHERE v.tenant=p.tenant AND v.channel_id=$3 AND NOT v.visible AND (v.product_id=p.id OR v.product_id=p.parent_id))")
        .bind(tenant(h)?).bind(ids).bind(channel_id(h)).fetch_all(&a.db).await?;
    let unit = veyndo_b2b::store::catalog::scope(a, h).await?;
    let admitted = if let Some(unit) = unit {
        sqlx::query_scalar("SELECT p.id FROM products p WHERE p.tenant=$1 AND p.id=ANY($2) AND EXISTS(SELECT 1 FROM b2b_catalog b WHERE b.tenant=p.tenant AND b.unit_id=$3 AND (b.product_id=p.id OR b.product_id=p.parent_id OR (p.parent_id IS NULL AND EXISTS(SELECT 1 FROM products child WHERE child.tenant=p.tenant AND child.parent_id=p.id AND child.id=b.product_id))))")
            .bind(tenant(h)?).bind(admitted).bind(unit).fetch_all(&a.db).await?
    } else {
        admitted
    };
    let admitted = admitted.into_iter().collect::<HashSet<_>>();
    Ok(ps
        .into_iter()
        .filter(|p| admitted.contains(&p.id))
        .filter(|p| {
            config
                .as_ref()
                .is_none_or(|c| selected(&c.product_ids, &p.id, p.parent_id.as_deref()))
        })
        .collect())
}

/// The product selection of the current channel; never taken from caller-supplied catalog criteria.
pub async fn catalog_scope(a: &State, h: &RequestContext) -> Result<Option<Vec<String>>> {
    Ok(current(a, h)
        .await?
        .and_then(|c| (!c.product_ids.is_empty()).then_some(c.product_ids)))
}
