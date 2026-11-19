//! Review submission with server-derived purchase verification, and the import of historical reviews with their provenance.
use crate::ports::Catalog;
use crate::store::{products, reviews};
use crate::types::review::{ImportedReview, NewReview};
use serde_json::{Value, json};
use veyndo_core::{RequestContext, Result, bad, conflict, outbox, uid};
use veyndo_identity::{merchant, permit};

/// Stores a shopper's review unpublished; whether it is a verified purchase comes from the orders, never from the request.
pub async fn submit(c: &Catalog, h: &RequestContext, product: &str, v: &Value) -> Result<Value> {
    let shopper = c.shoppers.current(h).await?;
    let review = NewReview::parse(v)?;
    if !reviews::product_exists(&c.core, &shopper.tenant, product).await? {
        return Err(bad("Unknown product"));
    }
    let verified = c.shoppers.bought(&shopper, product).await?;
    // Guest contact text cannot claim a buyer identity or reserve that buyer's review.
    let reviewer = shopper
        .customer
        .as_ref()
        .map(|id| format!("customer:{id}"))
        .unwrap_or_else(|| format!("cart:{}", shopper.cart));
    let id = uid();
    if !reviews::insert(
        &c.core,
        &shopper.tenant,
        &id,
        product,
        &reviewer,
        &review,
        verified,
    )
    .await?
    {
        return Err(conflict("One review per product and customer context"));
    }
    Ok(json!({"id":id,"state":"pending-moderation","verifiedPurchase":verified}))
}

/// Imports one historical review once; a replay with the same source is a no-op, a changed source a conflict.
pub async fn import(
    c: &Catalog,
    h: &RequestContext,
    id: &str,
    product: &str,
    data: &Value,
    source: &Value,
) -> Result<Value> {
    let tenant = merchant(h)?;
    permit(h, "catalog.write")?;
    permit(h, "customers.pii")?;
    if c.shop.is_preview(&tenant).await? {
        return Err(bad(
            "Historical reviews require their live product and native moderation",
        ));
    }
    let review = ImportedReview::parse(data)?;
    let mut tx = c.core.db.begin().await?;
    c.shop
        .attribute(&mut tx, h, "historical_review_import")
        .await?;
    reviews::lock_import(&mut tx, &tenant, id).await?;
    if !products::exists(&mut tx, &tenant, product).await? {
        return Err(bad(
            "Publish the referenced product before its historical reviews",
        ));
    }
    if let Some(previous) = reviews::imported_source(&mut tx, &tenant, id).await? {
        if previous != *source {
            return Err(conflict(
                "Imported review source changed; review its conflict",
            ));
        }
        tx.commit().await?;
        return Ok(json!({"id":id,"replayed":true}));
    }
    reviews::insert_imported(&mut tx, &tenant, id, product, &review, source).await?;
    outbox::publish(
        &mut tx,
        &tenant,
        "migration.record.imported",
        json!({"kind":"reviews","targetId":id}),
    )
    .await?;
    tx.commit().await?;
    Ok(json!({"id":id,"replayed":false,"state":"pending-moderation","verifiedPurchase":false}))
}
