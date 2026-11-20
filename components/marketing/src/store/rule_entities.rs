//! Writes rule metadata onto the product, customer or order it describes, bound to the revision the merchant read.
use serde_json::Value;
use veyndo_core::{Result, State, conflict};

pub async fn save(
    a: &State,
    t: &str,
    entity: &str,
    id: &str,
    data: &Value,
    rev: i64,
) -> Result<()> {
    let n=match entity{
        "products"=>sqlx::query("UPDATE products SET extra=jsonb_set(extra,'{automation}',$1),revision=revision+1 WHERE tenant=$2 AND id=$3 AND revision=$4").bind(data).bind(t).bind(id).bind(rev).execute(&a.db).await?.rows_affected(),
        "customers"=>sqlx::query("UPDATE customers SET automation=$1,revision=revision+1 WHERE tenant=$2 AND id=$3 AND revision=$4").bind(data).bind(t).bind(id).bind(rev).execute(&a.db).await?.rows_affected(),
        _=>sqlx::query("UPDATE orders SET data=jsonb_set(jsonb_set(data,'{automation}',$1),'{revision}',to_jsonb($4::bigint+1)) WHERE tenant=$2 AND id=$3 AND (data->>'revision')::bigint=$4").bind(data).bind(t).bind(id).bind(rev).execute(&a.db).await?.rows_affected(),
    };
    if n != 1 {
        return Err(conflict("Rule entity revision changed"));
    }
    Ok(())
}
