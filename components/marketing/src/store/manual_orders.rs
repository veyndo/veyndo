//! The sales channels a merchant manual order may be placed in.
use serde_json::Value;
use veyndo_core::{Result, State};

/// Up to 100 active public storefront/API channels as `{id,name}`.
pub async fn channels(a: &State, t: &str) -> Result<Vec<Value>> {
    Ok(sqlx::query_scalar!(r#"SELECT jsonb_build_object('id',id,'name',data->'name') AS "value!" FROM sales_channels WHERE tenant=$1 AND data->>'kind' IN ('storefront','headless') AND data->>'active'='true' AND coalesce(data->>'visibility','public')='public' ORDER BY id LIMIT 100"#, t).fetch_all(&a.db).await?)
}
