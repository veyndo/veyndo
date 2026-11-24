//! Transactional application of approved, revision-bound proposals: product changes, experience, app saves, translation drafts and bounded price autonomy.
use super::work_execution::{admit_provider, enqueue_translation};
use super::work_plan::approve;
use crate::domain::proposal::{Change, Proposal, validate_proposal};
use crate::ports::WorkHost;
use serde_json::{Value, json};
use sqlx::Row;
use veyndo_catalog::store::products::product;
use veyndo_cognition::inference::Choice;
use veyndo_core::history_context::context;
use veyndo_core::shop_scope::header;
use veyndo_core::{Error, RequestContext, Result, StatusCode, bad, conflict};
use veyndo_identity::{allowed, permit};
use veyndo_intelligence::domain::price_guardrails::check;
use veyndo_intelligence::knowledge::sync_product;
use veyndo_intelligence::service::autonomy::{Autonomous, reserve};
use veyndo_shop::Settings;

/// Applies an approved proposal; work plans go through their own approval.
pub async fn apply<H: WorkHost>(host: &H, t: &str, id: &str, h: &RequestContext) -> Result<Value> {
    let a = host.models().core;
    let work:bool=sqlx::query_scalar("SELECT coalesce(jsonb_array_length(proposal->'proposal'->'operations'),0)>0 FROM tasks WHERE tenant=$1 AND id=$2").bind(t).bind(id).fetch_optional(&a.db).await?.unwrap_or(false);
    if work {
        return approve(host, t, id, h).await;
    }
    apply_mode(host, t, id, h, false).await
}
/// A merchant approval needs the original actor of the run and every scope it was planned with.
fn admit_actor(
    h: &RequestContext,
    r: &sqlx::postgres::PgRow,
    v: &Value,
    autonomous: bool,
) -> Result<()> {
    if !autonomous
        && (r
            .get::<Option<String>, _>("run_actor")
            .as_deref()
            .or(v["actor"].as_str())
            != header(h, "x-rac-user")
            || r.get::<Option<Value>, _>("run_scopes")
                .is_some_and(|scopes| {
                    scopes.as_array().is_none_or(|scopes| {
                        scopes
                            .iter()
                            .any(|s| s.as_str().is_none_or(|s| !allowed(h, s)))
                    })
                }))
    {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Proposal requires its original actor and current permissions".into(),
        ));
    }
    Ok(())
}
/// The rights a proposal needs for what it changes; operation plans and autonomous translations never pass here.
fn admit_proposal(h: &RequestContext, p: &Proposal, autonomous: bool) -> Result<()> {
    if !p.operations.is_empty() {
        return Err(bad(
            "Operation plans require their exact private work approval",
        ));
    }
    if !p.changes.is_empty() {
        permit(h, "catalog.write")?;
    }
    if p.experience.is_some() || p.translation.is_some() {
        permit(h, "catalog.write")?;
    }
    if autonomous && p.translation.is_some() {
        return Err(bad("Translation drafts require explicit merchant approval"));
    }
    if autonomous {
        permit(h, "catalog.write")?;
        permit(h, "settings.write")?;
    }
    Ok(())
}
/// Writes the product changes and the experience against the revisions the preview saw.
async fn write_changes(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    t: &str,
    changes: Vec<Change>,
    (experience, expected): (Option<Value>, Option<i64>),
) -> Result<()> {
    for c in changes {
        let product_id = c.product_id.clone();
        let n=sqlx::query("UPDATE products SET price=COALESCE($1,price),stock=COALESCE($2,stock),revision=revision+1 WHERE tenant=$3 AND id=$4 AND revision=$5").bind(c.price).bind(c.stock).bind(t).bind(c.product_id).bind(c.expected_revision).execute(&mut **tx).await?.rows_affected();
        if n != 1 {
            return Err(conflict("Product changed since preview; create a new plan"));
        }
        let row = sqlx::query("SELECT * FROM products WHERE tenant=$1 AND id=$2")
            .bind(t)
            .bind(product_id)
            .fetch_one(&mut **tx)
            .await?;
        sync_product(tx, t, &serde_json::to_value(product(&row)).unwrap()).await?;
    }
    if let Some(e) = experience {
        let n = sqlx::query(
            "UPDATE experiences SET data=$1,revision=revision+1 WHERE tenant=$2 AND revision=$3",
        )
        .bind(e)
        .bind(t)
        .bind(expected)
        .execute(&mut **tx)
        .await?
        .rows_affected();
        if n != 1 {
            return Err(conflict("Experience changed since preview"));
        }
    }
    Ok(())
}
/// Applies a reviewed proposal in one transaction, explicitly or as bounded price autonomy.
pub async fn apply_mode<H: WorkHost>(
    host: &H,
    t: &str,
    id: &str,
    h: &RequestContext,
    autonomous: bool,
) -> Result<Value> {
    let a = host.models().core;
    let evidence: Option<Value> =
        sqlx::query_scalar("SELECT proposal FROM tasks WHERE tenant=$1 AND id=$2 AND NOT applied")
            .bind(t)
            .bind(id)
            .fetch_optional(&a.db)
            .await?;
    if let Some(evidence) = evidence
        && evidence["proposal"]["translation"].is_string()
    {
        permit(h, "catalog.write")?;
        let choice: Choice = serde_json::from_value(evidence["translationInference"].clone())
            .map_err(|_| bad("Invalid translation provider"))?;
        admit_provider(host.models().inference, &choice).await?;
    }
    let mut tx = a.db.begin().await?;
    context(
        &mut tx,
        h,
        if autonomous {
            "agent-autonomy"
        } else {
            "merchant"
        },
    )
    .await?;
    let r = sqlx::query("SELECT * FROM tasks WHERE tenant=$1 AND id=$2 FOR UPDATE")
        .bind(t)
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(Error(StatusCode::NOT_FOUND, "Task not found".into()))?;
    if r.get::<Option<String>, _>("run_state")
        .is_some_and(|s| s != "ready")
    {
        return Err(conflict("Agent run has no completed proposal to approve"));
    }
    let v: Value = r.get("proposal");
    admit_actor(h, &r, &v, autonomous)?;
    let p: Proposal =
        serde_json::from_value(v["proposal"].clone()).map_err(|e| bad(e.to_string()))?;
    admit_proposal(h, &p, autonomous)?;
    if r.get::<bool, _>("applied") {
        return Ok(
            json!({"taskId":id,"applied":true,"replayed":true,"translationJobId":p.translation.as_ref().map(|_|id)}),
        );
    }
    let settings_row =
        sqlx::query("SELECT data,revision FROM commerce_settings WHERE tenant=$1 FOR UPDATE")
            .bind(t)
            .fetch_one(&mut *tx)
            .await?;
    let settings: Settings = serde_json::from_value(settings_row.get("data"))
        .map_err(|_| bad("Invalid commerce settings"))?;
    if let Some(revision) = v["policyRevision"].as_i64()
        && revision != settings_row.get::<i64, _>("revision")
    {
        return Err(conflict(
            "Merchant guardrails/settings changed after preview",
        ));
    }
    let ids = p
        .changes
        .iter()
        .map(|c| c.product_id.clone())
        .collect::<Vec<_>>();
    let products =
        sqlx::query("SELECT * FROM products WHERE tenant=$1 AND id=ANY($2) ORDER BY id FOR UPDATE")
            .bind(t)
            .bind(ids)
            .fetch_all(&mut *tx)
            .await?
            .iter()
            .map(product)
            .collect::<Vec<_>>();
    validate_proposal(&p, &products)?;
    for c in &p.changes {
        check(
            &settings,
            products
                .iter()
                .find(|p| p.id == c.product_id)
                .ok_or(bad("Unknown product"))?,
            c,
            false,
        )?;
    }
    if autonomous {
        let request = Autonomous {
            task: id,
            changes: &p.changes,
            other_work: p.app_action.is_some() || p.experience.is_some(),
        };
        reserve(&mut tx, t, h, &settings, request, &products).await?;
    }
    if let Some(target) = &p.translation {
        let choice: Choice = serde_json::from_value(v["translationInference"].clone())
            .map_err(|_| bad("Invalid translation provider"))?;
        enqueue_translation(
            &mut tx,
            t,
            id,
            &settings,
            (target, &choice, false, p.translation_products.as_deref()),
        )
        .await?;
    }
    if let Some(change) = &p.app_action {
        host.apply_change(&mut tx, t, change, h).await?;
    }
    write_changes(
        &mut tx,
        t,
        p.changes,
        (p.experience, p.expected_experience_revision),
    )
    .await?;
    sqlx::query("UPDATE tasks SET applied=true,applied_at=now() WHERE tenant=$2 AND id=$1")
        .bind(id)
        .bind(t)
        .execute(&mut *tx)
        .await?;
    veyndo_core::outbox::publish_on(
        &mut *tx,
        t,
        "merchant.change.applied",
        &json!({"taskId":id,"autonomous":autonomous,"actor":header(h,"x-rac-user")}),
    )
    .await?;
    tx.commit().await?;
    Ok(json!({"taskId":id,"applied":true,"translationJobId":p.translation.as_ref().map(|_|id)}))
}
