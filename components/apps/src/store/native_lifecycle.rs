//! Capture accepted definitions in existing operational JSON under native package locks; no app-specific tables.
use crate::domain::native_lifecycle::{Snapshot, compile, quote_machine, validate_locale};
use serde_json::{Value, json};
use veyndo_app_model::manifest::Manifest;
use veyndo_core::{Result, bad};
use veyndo_orders::domain::order_machine::OrderMachine;
use veyndo_orders::store::machine::machine_conn;
use veyndo_runtime::scoped_pool::ScopedPool;

async fn packages(conn: &mut sqlx::PgConnection, t: &str) -> Result<Vec<Manifest>> {
    let rows: Vec<Value> = sqlx::query_scalar("SELECT manifest FROM app_packages WHERE tenant=$1 AND active AND jsonb_array_length(coalesce(manifest->'lifecycles','[]'))>0 ORDER BY id LIMIT 9 FOR SHARE")
        .bind(t).fetch_all(conn).await?;
    if rows.len() > 8 {
        return Err(bad("At most eight native lifecycle apps per shop"));
    }
    rows.into_iter()
        .map(|v| serde_json::from_value(v).map_err(|_| bad("Invalid installed lifecycle")))
        .collect()
}
/// Called under the existing aggregate configuration lock, including selected publication.
/// Requires every active lifecycle package to carry labels in a new main language.
pub async fn validate_active_locale(
    conn: &mut sqlx::PgConnection,
    t: &str,
    main: &str,
) -> Result<()> {
    for m in packages(conn, t).await? {
        validate_locale(&m, main)?;
    }
    Ok(())
}
/// Admits a lifecycle package only if all active ones still compile together.
pub async fn admission(conn: &mut sqlx::PgConnection, t: &str, m: &Manifest) -> Result<()> {
    if m.lifecycles.is_empty() {
        return Ok(());
    }
    let main: Option<String> = sqlx::query_scalar(
        "SELECT coalesce(data->>'mainLocale','en-GB') FROM commerce_settings WHERE tenant=$1",
    )
    .bind(t)
    .fetch_optional(&mut *conn)
    .await?;
    validate_locale(m, main.as_deref().unwrap_or("en-GB"))?;
    let mut apps = packages(conn, t).await?;
    apps.retain(|old| old.id != m.id);
    apps.push(m.clone());
    if apps.len() > 8 {
        return Err(bad("At most eight native lifecycle apps per shop"));
    }
    let (base, _) = machine_conn(conn, t).await?;
    compile(base, &apps, "order")?;
    compile(quote_machine(), &apps, "quote")?;
    Ok(())
}
/// The compiled lifecycle of an object, captured for the object that is being accepted.
pub async fn capture(conn: &mut sqlx::PgConnection, t: &str, object: &str) -> Result<Value> {
    // This single ordered package snapshot is held until the native object commits.
    let apps = packages(conn, t).await?;
    if !apps
        .iter()
        .any(|m| m.lifecycles.iter().any(|c| c.object == object))
    {
        return Ok(Value::Null);
    }
    let (base, revision) = if object == "order" {
        machine_conn(conn, t).await?
    } else {
        (quote_machine(), 0)
    };
    let machine = compile(base, &apps, object)?;
    let main_locale: Option<String> =
        sqlx::query_scalar("SELECT data->>'mainLocale' FROM commerce_settings WHERE tenant=$1")
            .bind(t)
            .fetch_optional(&mut *conn)
            .await?
            .flatten();
    let main_locale = main_locale.unwrap_or_else(|| "en-GB".into());
    Ok(json!(Snapshot {
        version: 1,
        object: object.into(),
        revision,
        main_locale,
        machine
    }))
}
/// Requires the active lifecycle packages to compile on a new order base machine.
pub async fn validate_order_base(
    conn: &mut sqlx::PgConnection,
    t: &str,
    base: OrderMachine,
) -> Result<()> {
    let apps = packages(conn, t).await?;
    compile(base, &apps, "order")?;
    Ok(())
}

/// The order machine currently in force for a shop.
pub async fn current_order(db: &ScopedPool, t: &str) -> Result<OrderMachine> {
    let mut conn = db.begin().await?;
    let apps = packages(&mut conn, t).await?;
    let (base, _) = machine_conn(&mut conn, t).await?;
    compile(base, &apps, "order")
}
/// Stores the captured lifecycle on an order that is being placed.
pub async fn bind_order(conn: &mut sqlx::PgConnection, t: &str, order: &mut Value) -> Result<()> {
    let lifecycle = capture(conn, t, "order").await?;
    if !lifecycle.is_null() {
        order["nativeLifecycle"] = lifecycle;
    }
    Ok(())
}
