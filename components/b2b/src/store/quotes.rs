//! Negotiated quotes of a company cart: listing for merchants, buyers and the cart, revisioned transitions, offer acceptance and release, and the one-time purchase of an accepted offer. Columns checked at compile time.
use serde_json::Value;
use sqlx::PgConnection;
use veyndo_core::Result;

/// A quote as the lists show it.
pub struct QuoteRow {
    pub id: String,
    pub unit_id: String,
    pub buyer_id: String,
    pub state: String,
    pub revision: i64,
    pub data: Value,
}

/// The shop's latest quotes (at most 100).
pub async fn latest(conn: &mut PgConnection, tenant: &str) -> Result<Vec<QuoteRow>> {
    Ok(sqlx::query_as!(QuoteRow, "SELECT id,unit_id,buyer_id,state,revision,data FROM b2b_quotes WHERE tenant=$1 ORDER BY created_at DESC LIMIT 100", tenant)
        .fetch_all(conn)
        .await?)
}

/// The buyer's latest quotes in one unit (at most 100).
pub async fn of_buyer(
    conn: &mut PgConnection,
    tenant: &str,
    unit: &str,
    buyer: &str,
) -> Result<Vec<QuoteRow>> {
    Ok(sqlx::query_as!(QuoteRow, "SELECT id,unit_id,buyer_id,state,revision,data FROM b2b_quotes WHERE tenant=$1 AND unit_id=$2 AND buyer_id=$3 ORDER BY created_at DESC LIMIT 100", tenant, unit, buyer)
        .fetch_all(conn)
        .await?)
}

/// Whose cart a quote belongs to.
pub struct CartOwner<'a> {
    pub cart: &'a str,
    pub buyer: &'a str,
    pub unit: &'a str,
}

/// The cart's latest quotes for its buyer and unit, or only quote `id` (at most 20).
pub async fn of_cart(
    conn: &mut PgConnection,
    tenant: &str,
    owner: CartOwner<'_>,
    id: Option<&str>,
) -> Result<Vec<QuoteRow>> {
    Ok(sqlx::query_as!(QuoteRow, "SELECT id,unit_id,buyer_id,state,revision,data FROM b2b_quotes WHERE tenant=$1 AND cart_id=$2 AND buyer_id=$3 AND unit_id=$4 AND ($5::text IS NULL OR id=$5) ORDER BY created_at DESC,id DESC LIMIT 20",
        tenant, owner.cart, owner.buyer, owner.unit, id)
    .fetch_all(conn)
    .await?)
}

/// The unit and buyer of a quote.
pub struct QuoteParty {
    pub unit_id: String,
    pub buyer_id: String,
}

/// Who quote `id` belongs to.
pub async fn party(conn: &mut PgConnection, tenant: &str, id: &str) -> Result<Option<QuoteParty>> {
    Ok(sqlx::query_as!(
        QuoteParty,
        "SELECT unit_id,buyer_id FROM b2b_quotes WHERE tenant=$1 AND id=$2",
        tenant,
        id
    )
    .fetch_optional(conn)
    .await?)
}

/// A quote locked for a merchant or buyer transition.
pub struct TransitionQuote {
    pub unit_id: String,
    pub buyer_id: String,
    pub state: String,
    pub revision: i64,
    pub data: Value,
    pub cart_id: String,
}

/// Quote `id`, locked for a transition.
pub async fn for_transition(
    conn: &mut PgConnection,
    tenant: &str,
    id: &str,
) -> Result<Option<TransitionQuote>> {
    Ok(sqlx::query_as!(TransitionQuote, "SELECT unit_id,buyer_id,state,revision,data,cart_id FROM b2b_quotes WHERE tenant=$1 AND id=$2 FOR UPDATE", tenant, id)
        .fetch_optional(conn)
        .await?)
}

/// A quote whose offer a cart selects or prices from; `valid` says whether its offer has not expired (null without an offer).
pub struct LockedQuote {
    pub unit_id: String,
    pub buyer_id: String,
    pub cart_id: String,
    pub state: String,
    pub revision: i64,
    pub data: Value,
    pub valid: Option<bool>,
}

/// Quote `id` with its offer validity, locked for selection.
pub async fn offer_locked(
    conn: &mut PgConnection,
    tenant: &str,
    id: &str,
) -> Result<Option<LockedQuote>> {
    Ok(sqlx::query_as!(LockedQuote, "SELECT unit_id,buyer_id,cart_id,state,revision,data,(data->>'validUntil')::timestamptz>now() AS valid FROM b2b_quotes WHERE tenant=$1 AND id=$2 FOR UPDATE", tenant, id)
        .fetch_optional(conn)
        .await?)
}

/// Quote `id`, share-locked so checkout prices from an offer that cannot change meanwhile.
pub async fn shared(
    conn: &mut PgConnection,
    tenant: &str,
    id: &str,
) -> Result<Option<LockedQuote>> {
    Ok(sqlx::query_as!(LockedQuote, "SELECT unit_id,buyer_id,cart_id,state,revision,data,(data->>'validUntil')::timestamptz>now() AS valid FROM b2b_quotes WHERE tenant=$1 AND id=$2 FOR SHARE", tenant, id)
        .fetch_optional(conn)
        .await?)
}

/// Moves quote `id` to `state` with its new document and advances the revision.
pub async fn transition(
    conn: &mut PgConnection,
    tenant: &str,
    id: &str,
    state: &str,
    data: &Value,
) -> Result<()> {
    sqlx::query!(
        "UPDATE b2b_quotes SET state=$3,revision=revision+1,data=$4 WHERE tenant=$1 AND id=$2",
        tenant,
        id,
        state,
        data
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// Records a new quote request for the cart.
pub async fn request(
    conn: &mut PgConnection,
    tenant: &str,
    id: &str,
    owner: CartOwner<'_>,
    data: &Value,
) -> Result<()> {
    sqlx::query!(
        "INSERT INTO b2b_quotes(tenant,id,unit_id,buyer_id,cart_id,data) VALUES($1,$2,$3,$4,$5,$6)",
        tenant,
        id,
        owner.unit,
        owner.buyer,
        owner.cart,
        data
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// Whether an offer valid until `until` expires in the future, within one year.
pub async fn offer_window(conn: &mut PgConnection, until: &str) -> Result<bool> {
    Ok(sqlx::query_scalar!(r#"SELECT $1::text::timestamptz>now() AND $1::text::timestamptz<now()+interval '1 year' AS "valid!""#, until)
        .fetch_one(conn)
        .await?)
}

/// Marks quote `id` accepted.
pub async fn accept(conn: &mut PgConnection, tenant: &str, id: &str) -> Result<()> {
    sqlx::query!(
        "UPDATE b2b_quotes SET state='accepted',revision=revision+1 WHERE tenant=$1 AND id=$2",
        tenant,
        id
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// Returns an accepted quote of this cart, buyer and unit to the offered state.
pub async fn release(
    conn: &mut PgConnection,
    tenant: &str,
    id: &str,
    owner: CartOwner<'_>,
) -> Result<()> {
    sqlx::query!("UPDATE b2b_quotes SET state='offered',revision=revision+1 WHERE tenant=$1 AND id=$2 AND state='accepted' AND cart_id=$3 AND buyer_id=$4 AND unit_id=$5",
        tenant, id, owner.cart, owner.buyer, owner.unit)
    .execute(conn)
    .await?;
    Ok(())
}

/// Marks an accepted quote ordered by `order`; returns whether it was still accepted.
pub async fn consume(conn: &mut PgConnection, tenant: &str, id: &str, order: &str) -> Result<bool> {
    let changed = sqlx::query!("UPDATE b2b_quotes SET state='ordered',revision=revision+1,data=data||jsonb_build_object('orderId',$3::text) WHERE tenant=$1 AND id=$2 AND state='accepted'",
        tenant, id, order)
    .execute(conn)
    .await?;
    Ok(changed.rows_affected() == 1)
}
