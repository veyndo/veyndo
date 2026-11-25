//! Token-redacted merchant order detail with its activity, and append-only operational notes.
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{lenient, view};
use veyndo_core::shop_scope::header;
use veyndo_core::{
    Error, RequestContext, Result, State, StatusCode, bad, conflict, store::history_context,
};

/// The order without its cart token, with creation time and the customer's email.
pub async fn detail(a: &State, t: &str, id: &str) -> Result<Value> {
    let r=sqlx::query!(r#"SELECT o.data #- '{cart,token}' AS "data!",o.created_at::text AS created,coalesce(o.data->'orderCustomer'->>'email',c.data->>'email') AS email FROM orders o JOIN carts c ON c.id=o.cart_id WHERE o.tenant=$1 AND o.id=$2"#, t, id).fetch_optional(&a.db).await?.ok_or(Error(StatusCode::NOT_FOUND,"Order not found".into()))?;
    let mut v: Value = r.data;
    crate::domain::order_fields::order_fields(&mut v);
    v["createdAt"] = json!(r.created);
    v["customerEmail"] = json!(r.email);
    let rows=sqlx::query!("SELECT id,actor,kind,data,created_at::text AS created FROM order_activity WHERE tenant=$1 AND order_id=$2 ORDER BY id DESC LIMIT 100", t, id).fetch_all(&a.db).await?;
    v["activity"]=json!(rows.iter().map(|r|json!({"id":r.id,"actor":r.actor,"kind":r.kind,"data":r.data,"createdAt":r.created})).collect::<Vec<_>>());
    Ok(v)
}

/// The JSON fields `note` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct NoteInput {
    #[serde(deserialize_with = "lenient")]
    text: Option<String>,
    #[serde(deserialize_with = "lenient")]
    revision: Option<i64>,
}

/// Appends a merchant note when the order still has the revision the merchant read.
pub async fn note(a: &State, h: &RequestContext, t: &str, id: &str, v: &Value) -> Result<()> {
    let input: NoteInput = view(v);
    let text = input
        .text
        .as_deref()
        .filter(|s| !s.trim().is_empty() && s.len() <= 4000)
        .ok_or(bad("Note must contain 1..4000 characters"))?;
    let mut tx = a.db.begin().await?;
    history_context::context(&mut tx, h, "merchant").await?;
    let n=sqlx::query!("UPDATE orders SET data=jsonb_set(data,'{revision}',to_jsonb((data->>'revision')::bigint+1)) WHERE tenant=$1 AND id=$2 AND (data->>'revision')::bigint=$3", t, id, input.revision.ok_or(bad("revision required"))?).execute(&mut *tx).await?.rows_affected();
    if n != 1 {
        return Err(conflict("Order revision changed"));
    }
    sqlx::query!(
        "INSERT INTO order_activity(tenant,order_id,actor,kind,data) VALUES($1,$2,$3,'note',$4)",
        t,
        id,
        header(h, "x-rac-user").unwrap(),
        json!({"text":text})
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!("UPDATE carts SET data=jsonb_set(carts.data,'{order}',o.data) FROM orders o WHERE carts.id=o.cart_id AND o.tenant=$1 AND o.id=$2", t, id).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}

/// The latest 100 orders of the shop without cart tokens, with the derived order fields.
pub async fn recent(a: &State, t: &str) -> Result<Vec<Value>> {
    let rows = sqlx::query!(r#"SELECT data #- '{cart,token}' AS "data!" FROM orders WHERE tenant=$1 ORDER BY created_at DESC LIMIT 100"#, t)
    .fetch_all(&a.db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| {
            let mut v: Value = r.data;
            crate::domain::order_fields::order_fields(&mut v);
            v
        })
        .collect())
}

/// The stored order document of one order.
pub async fn data(a: &State, t: &str, id: &str) -> Result<Value> {
    Ok(
        sqlx::query_scalar!("SELECT data FROM orders WHERE tenant=$1 AND id=$2", t, id)
            .fetch_one(&a.db)
            .await?,
    )
}

/// Appends a flow note to the order activity inside the caller's transaction.
pub async fn flow_note(
    tx: &mut sqlx::PgConnection,
    t: &str,
    (order, actor): (&str, &str),
    data: &Value,
) -> Result<()> {
    sqlx::query!(
        "INSERT INTO order_activity(tenant,order_id,actor,kind,data) VALUES($1,$2,$3,'flow',$4)",
        t,
        order,
        actor,
        data
    )
    .execute(tx)
    .await?;
    Ok(())
}

/// Whether the shopper's cart or customer account already ordered the product.
pub async fn bought(
    a: &State,
    t: &str,
    (cart, customer): (&str, Option<&str>),
    product: &str,
) -> Result<bool> {
    Ok(sqlx::query_scalar!(r#"SELECT EXISTS(SELECT 1 FROM orders o WHERE o.tenant=$1 AND (o.cart_id=$2 OR ($3::text IS NOT NULL AND o.data->'orderCustomer'->>'customerId'=$3)) AND EXISTS(SELECT 1 FROM jsonb_array_elements(o.data->'cart'->'lineItems') i WHERE i->>'id'=$4)) AS "exists!""#, t, cart, customer, product)
        .fetch_one(&a.db)
        .await?)
}
