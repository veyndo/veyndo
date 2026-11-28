//! The shop twin of an admitted proposal: replays its product changes through the apply path's own CAS writes in a transaction that is always rolled back, then stores diff, projection and rule findings; parts it cannot replay are named, never estimated.
use crate::domain::proposal::Proposal;
use crate::domain::simulation::{
    self, Basis, Demand, ProductBasis, RuleFinding, Simulation, VERSION, WINDOW_DAYS,
};
use crate::ports::WorkHost;
use crate::store::{products, twin};
use serde_json::Value;
use veyndo_core::store::history_context::context;
use veyndo_core::{Error, RequestContext, Result, StatusCode};
use veyndo_identity::allowed;
use veyndo_shop::service::read_cache::config;

/// The twin's result for a proposal; a replay failure becomes an explicit `failed` simulation, never a fake projection.
pub async fn simulate<H: WorkHost>(
    host: &H,
    t: &str,
    h: &RequestContext,
    p: &Proposal,
    policy_revision: i64,
) -> Value {
    let at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let result = if p.changes.is_empty() {
        Ok(simulation::unreplayed(p, Some(policy_revision), at.clone()))
    } else {
        replay(host, t, h, p, at.clone()).await
    };
    let simulation = result.unwrap_or_else(|e| {
        let basis = Basis {
            policy_revision: Some(policy_revision),
            products: vec![],
        };
        simulation::failed(p, basis, at, reason(&e))
    });
    serde_json::to_value(simulation).unwrap_or(Value::Null)
}

/// The stable reason code of a failed replay.
fn reason(e: &Error) -> &'static str {
    if e.0 == StatusCode::CONFLICT || e.1.contains("lock") || e.1.contains("timeout") {
        "busy"
    } else {
        "replayFailed"
    }
}

/// Writes the changes on the current rows exactly as apply would, reads the result and the recorded demand, and rolls back.
async fn replay<H: WorkHost>(
    host: &H,
    t: &str,
    h: &RequestContext,
    p: &Proposal,
    at: String,
) -> Result<Simulation> {
    let a = host.models().core;
    let (settings, settings_revision) = config(a, t).await?;
    let ids = p
        .changes
        .iter()
        .map(|c| c.product_id.clone())
        .collect::<Vec<_>>();
    let mut tx = a.db.begin().await?;
    twin::bound(&mut tx).await?;
    // History triggers fire inside the twin too; the rollback discards them with every other write.
    context(&mut tx, h, "simulation").await?;
    let clocks = products::clocks(&mut tx, t, &ids).await?;
    let mut before = vec![];
    for id in &ids {
        before.push(products::get(&mut tx, t, id).await?);
    }
    let mut rules = vec![];
    for c in &p.changes {
        if products::apply_change(&mut tx, t, c).await? != 1 {
            rules.push(RuleFinding {
                rule: "revisionCurrent".into(),
                product_id: c.product_id.clone(),
                outcome: "violated".into(),
                severity: "blocking".into(),
                codes: vec!["staleRevision".into()],
            });
        }
    }
    let mut after = vec![];
    for id in &ids {
        after.push(products::get(&mut tx, t, id).await?);
    }
    // Order figures feed the projection only for an actor who may read them, as in planning.
    let demand = if allowed(h, "orders.read") || allowed(h, "overview.read") {
        let rows = twin::demand(&mut tx, t, &ids, WINDOW_DAYS).await?;
        Some(
            rows.into_iter()
                .map(|d| Demand {
                    product_id: d.product_id,
                    units: d.units,
                    orders: d.orders,
                })
                .collect::<Vec<_>>(),
        )
    } else {
        None
    };
    tx.rollback().await?;
    let (diff, impact, mut found) =
        simulation::changes(&settings, &p.changes, &before, &after, demand.as_deref())?;
    rules.append(&mut found);
    let parts = simulation::parts(p);
    Ok(Simulation {
        version: VERSION,
        status: simulation::status(&parts).into(),
        twin: simulation::TWIN.into(),
        simulated_at: at,
        basis: Basis {
            policy_revision: Some(settings_revision),
            products: clocks
                .into_iter()
                .map(|c| ProductBasis {
                    product_id: c.id,
                    revision: c.revision,
                    content_revision: c.content_revision,
                })
                .collect(),
        },
        parts,
        diff,
        impact: Some(impact),
        blocking: simulation::blocking(&rules),
        rules,
        not_checked: simulation::NOT_CHECKED
            .iter()
            .map(|s| s.to_string())
            .collect(),
        reason: None,
    })
}
