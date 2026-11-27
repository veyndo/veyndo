//! Durable UTC cron ticks emit namespaced outbox events; replicas lock due rows and staging never runs them.
use crate::domain::schedule::parse;
use crate::types::events::AppDefined;
use serde_json::json;
use veyndo_app_model::manifest::Manifest;
use veyndo_app_model::triggers::AppSchedule;
use veyndo_core::{Result, State, bad};

/// Stores the schedules of a package; an unchanged schedule keeps its due time.
pub async fn sync(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    t: &str,
    m: &Manifest,
) -> Result<()> {
    let ids = m.schedules.iter().map(|s| s.id.clone()).collect::<Vec<_>>();
    sqlx::query!(
        "DELETE FROM app_schedules WHERE tenant=$1 AND app=$2 AND NOT(id=ANY($3))",
        t,
        m.id,
        &ids
    )
    .execute(&mut **tx)
    .await?;
    for s in &m.schedules {
        let next = parse(&s.cron)?
            .upcoming(chrono::Utc)
            .next()
            .ok_or(bad("Cron has no future occurrence"))?
            .to_rfc3339();
        // Identical installs retain their due time. Changing a schedule starts its new clock.
        sqlx::query!(
            "INSERT INTO app_schedules(tenant,app,id,definition,next_run) VALUES($1,$2,$3,$4,$5::text::timestamptz) ON CONFLICT(tenant,app,id) DO UPDATE SET next_run=CASE WHEN app_schedules.definition=$4 THEN app_schedules.next_run ELSE EXCLUDED.next_run END,definition=$4",
            t,
            m.id,
            s.id,
            json!(s),
            next
        )
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}
/// Emits one event for every due schedule of an active package in a live shop.
pub async fn schedule_once(a: &State) -> Result<()> {
    let mut tx = a.db.begin().await?;
    // Lock the package as well: deactivation/release cannot race an old schedule into the live outbox.
    let rows = sqlx::query!(r#"SELECT s.tenant,s.app,s.id,s.definition,s.next_run::text AS "due!" FROM app_schedules s JOIN app_packages p ON p.tenant=s.tenant AND p.id=s.app WHERE p.active AND EXISTS(SELECT 1 FROM tenants t WHERE t.id=coalesce((SELECT live_tenant FROM shop_environments WHERE tenant=s.tenant),s.tenant) AND t.status='active') AND NOT EXISTS(SELECT 1 FROM shop_environments e WHERE e.tenant=s.tenant) AND s.next_run<=now() AND s.definition->>'enabled'='true' ORDER BY s.next_run LIMIT 32 FOR UPDATE OF s,p SKIP LOCKED"#)
        .fetch_all(&mut *tx)
        .await?;
    for r in rows {
        let t = r.tenant;
        let app = r.app;
        let id = r.id;
        let s: AppSchedule =
            serde_json::from_value(r.definition).map_err(|_| bad("Invalid stored schedule"))?;
        let next = parse(&s.cron)?
            .upcoming(chrono::Utc)
            .next()
            .ok_or(bad("Schedule exhausted"))?
            .to_rfc3339();
        let event = veyndo_core::events::emit_id_as(
            &mut *tx,
            &t,
            &format!("app.{app}.{}", s.action),
            &AppDefined(s.input),
        )
        .await?;
        sqlx::query!(
            "INSERT INTO app_schedule_runs(tenant,app,schedule,due,event_id) VALUES($1,$2,$3,$4::text::timestamptz,$5)",
            t,
            app,
            id,
            r.due,
            event
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query!(
            "UPDATE app_schedules SET next_run=$1::text::timestamptz WHERE tenant=$2 AND app=$3 AND id=$4",
            next,
            t,
            app,
            id
        )
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}
