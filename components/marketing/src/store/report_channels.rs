//! The sales channels a merchant report covers, with their names, kinds and settings overrides, read inside the report's snapshot transaction.
use serde_json::Value;
use veyndo_core::Result;

/// A sales channel with its name, kind and settings overrides.
pub struct ChannelRow {
    pub id: String,
    pub name: Option<Value>,
    pub kind: Option<String>,
    pub overrides: Option<Value>,
}

/// Up to 501 channels of the shop in ID order, so a caller can refuse more than 500.
pub async fn channels(c: &mut sqlx::PgConnection, t: &str) -> Result<Vec<ChannelRow>> {
    Ok(sqlx::query_as!(ChannelRow, r#"SELECT s.id,s.data->'name' AS name,s.data->>'kind' AS kind,o.data AS "overrides?" FROM sales_channels s LEFT JOIN commerce_overrides o ON o.tenant=s.tenant AND o.channel_id=s.id WHERE s.tenant=$1 ORDER BY s.id LIMIT 501"#, t).fetch_all(&mut *c).await?)
}
