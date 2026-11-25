//! Inspect tenant-owned definition references and durable uses before configuration deletion. Settings, frontend bindings and product visibility come from their owners' stores; orders, carts and customers through the records port, all on the caller's connection.
use crate::ports::records;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_catalog::store::rule_products;
use veyndo_core::Result;
use veyndo_core::document::view;
use veyndo_shop::store::channel_settings;

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
                    deps.push(json!({"kind":other,"id":row.id,"name":view::<Named>(&row.data).name,"count":1}));
                }
            }
        }
        let settings = [
            (
                "settings",
                channel_settings::basis_documents(conn, t).await?,
            ),
            (
                "channel_settings",
                channel_settings::override_documents(conn, t).await?,
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
        "channels" => {
            let frontends = channel_settings::frontend_count(conn, t, id).await?;
            let [orders, carts, customers] = records()?.channel_usage(conn, t, id).await?;
            vec![
                ("frontends", frontends),
                orders,
                carts,
                customers,
                (
                    "product_visibility",
                    rule_products::visibility_count(conn, t, id).await?,
                ),
                (
                    "company_settings",
                    channel_settings::company_override_count(conn, t, id).await?,
                ),
                (
                    "channel_settings",
                    channel_settings::override_count(conn, t, id).await?,
                ),
            ]
        }
        _ => vec![],
    };
    for (scope, count) in counts {
        if count > 0 {
            deps.push(json!({"kind":scope,"id":id,"count":count}));
        }
    }
    Ok(json!({"blocked":!deps.is_empty(),"dependencies":deps}))
}

/// The name a referencing definition shows.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Named {
    name: Value,
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
