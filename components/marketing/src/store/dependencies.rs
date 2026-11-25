//! Inspect tenant-owned definition references and durable uses before configuration deletion.
use serde_json::{Value, json};
use veyndo_core::Result;

pub fn references(v: &Value, kind: &str, id: &str) -> bool {
    match v {
        Value::Object(o) => {
            let rule = kind == "rules"
                && o.get("type").and_then(Value::as_str) == Some("ruleReference")
                && o.get("ruleId").and_then(Value::as_str) == Some(id);
            let channel = kind == "channels"
                && (o.get("type").and_then(Value::as_str) == Some("salesChannel")
                    || o.get("name").and_then(Value::as_str) == Some("salesChannel"))
                && [
                    o.get("values"),
                    o.get("config").and_then(|c| c.get("salesChannelIds")),
                ]
                .into_iter()
                .flatten()
                .any(|v| {
                    v.as_array()
                        .is_some_and(|a| a.iter().any(|v| v.as_str() == Some(id)))
                });
            rule || channel || o.values().any(|v| references(v, kind, id))
        }
        Value::Array(a) => a.iter().any(|v| references(v, kind, id)),
        _ => false,
    }
}
/// A definition or settings document and its id.
struct Definition {
    id: String,
    data: Value,
}

pub async fn inspect(
    conn: &mut sqlx::PgConnection,
    t: &str,
    kind: &str,
    id: &str,
) -> Result<Value> {
    let mut deps = Vec::<Value>::new();
    if kind == "channels" && id == "default" {
        deps.push(json!({"kind":"default_channel","id":id,"count":1}));
    }
    if kind == "rules" || kind == "channels" {
        let definitions = [
            (
                "rules",
                sqlx::query_as!(Definition, r#"SELECT id,jsonb_build_object('name',name,'condition',condition) AS "data!" FROM commerce_rules WHERE tenant=$1"#, t)
                    .fetch_all(&mut *conn)
                    .await?,
            ),
            (
                "promotions",
                sqlx::query_as!(Definition, "SELECT id,data FROM commerce_promotions WHERE tenant=$1", t)
                    .fetch_all(&mut *conn)
                    .await?,
            ),
            (
                "flows",
                sqlx::query_as!(Definition, "SELECT id,data FROM commerce_flows WHERE tenant=$1", t)
                    .fetch_all(&mut *conn)
                    .await?,
            ),
        ];
        for (other, rows) in definitions {
            for row in rows {
                if !(other == kind && row.id == id) && references(&row.data, kind, id) {
                    deps.push(json!({"kind":other,"id":row.id,"name":row.data["name"],"count":1}));
                }
            }
        }
        let settings = [
            (
                "settings",
                sqlx::query_as!(
                    Definition,
                    r#"SELECT 'default' AS "id!",data FROM commerce_settings WHERE tenant=$1"#,
                    t
                )
                .fetch_all(&mut *conn)
                .await?,
            ),
            (
                "channel_settings",
                sqlx::query_as!(
                    Definition,
                    "SELECT channel_id AS id,data FROM commerce_overrides WHERE tenant=$1",
                    t
                )
                .fetch_all(&mut *conn)
                .await?,
            ),
        ];
        for (scope, rows) in settings {
            for row in rows {
                if references(&row.data, kind, id) {
                    deps.push(json!({"kind":scope,"id":row.id,"count":1}));
                }
            }
        }
    }
    let counts: Vec<(&str, i64)> = match kind {
        "promotions" => vec![(
            "orders",
            sqlx::query_scalar!(r#"SELECT count(*) AS "count!" FROM promotion_uses WHERE tenant=$1 AND promotion=$2"#, t, id).fetch_one(&mut *conn).await?,
        )],
        "flows" => vec![(
            "pending_jobs",
            sqlx::query_scalar!(r#"SELECT count(*) AS "count!" FROM flow_jobs WHERE tenant=$1 AND flow=$2 AND state IN ('queued','running','uncertain')"#, t, id).fetch_one(&mut *conn).await?,
        )],
        "channels" => vec![
            (
                "frontends",
                sqlx::query_scalar!(r#"SELECT count(*) AS "count!" FROM hosted_frontends WHERE tenant=$1 AND channel=$2"#, t, id).fetch_one(&mut *conn).await?,
            ),
            (
                "orders",
                sqlx::query_scalar!(r#"SELECT count(*) AS "count!" FROM orders WHERE tenant=$1 AND data->>'salesChannelId'=$2"#, t, id).fetch_one(&mut *conn).await?,
            ),
            (
                "carts",
                sqlx::query_scalar!(r#"SELECT count(*) AS "count!" FROM carts WHERE tenant=$1 AND sales_channel_id=$2"#, t, id).fetch_one(&mut *conn).await?,
            ),
            (
                "customers",
                sqlx::query_scalar!(r#"SELECT count(*) AS "count!" FROM customers WHERE tenant=$1 AND sales_channel_id=$2"#, t, id).fetch_one(&mut *conn).await?,
            ),
            (
                "product_visibility",
                sqlx::query_scalar!(r#"SELECT count(*) AS "count!" FROM product_channel_visibility WHERE tenant=$1 AND channel_id=$2"#, t, id).fetch_one(&mut *conn).await?,
            ),
            (
                "company_settings",
                sqlx::query_scalar!(r#"SELECT count(*) AS "count!" FROM company_overrides WHERE tenant=$1 AND channel_id=$2"#, t, id).fetch_one(&mut *conn).await?,
            ),
            (
                "channel_settings",
                sqlx::query_scalar!(r#"SELECT count(*) AS "count!" FROM commerce_overrides WHERE tenant=$1 AND channel_id=$2"#, t, id).fetch_one(&mut *conn).await?,
            ),
        ],
        _ => vec![],
    };
    for (scope, count) in counts {
        if count > 0 {
            deps.push(json!({"kind":scope,"id":id,"count":count}));
        }
    }
    Ok(json!({"blocked":!deps.is_empty(),"dependencies":deps}))
}
#[cfg(test)]
mod tests {
    use super::references;
    use serde_json::json;
    #[test]
    fn references_are_semantic_and_nested() {
        let v = json!({"pipeline":{"nodes":[{"condition":{"type":"ruleReference","ruleId":"r"}}]}});
        assert!(references(&v, "rules", "r"));
        assert!(!references(&v, "rules", "other"));
        assert!(!references(
            &json!({"instruction":"r","ruleId":"r"}),
            "rules",
            "r"
        ));
        assert!(references(
            &json!({"type":"salesChannel","values":["c"]}),
            "channels",
            "c"
        ));
        assert!(references(
            &json!({"type":"shopwareCondition","name":"salesChannel","config":{"salesChannelIds":["c"]}}),
            "channels",
            "c"
        ));
        assert!(!references(&v, "channels", "r"));
    }
}
