//! Transactional application of approved, revision-bound proposals: product changes, experience, app saves, translation drafts and bounded price autonomy.
use super::work_execution::{admit_provider, enqueue_translation};
use super::work_plan::approve;
use crate::domain::proposal::{Change, Proposal, validate_proposal};
use crate::domain::views::{Evidence, ProposalParts};
use crate::ports::WorkHost;
use crate::store::{products, tasks};
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_cognition::inference::Choice;
use veyndo_core::document::{lenient, view};
use veyndo_core::shop_scope::header;
use veyndo_core::store::history_context::context;
use veyndo_core::{Error, RequestContext, Result, StatusCode, bad, conflict};
use veyndo_identity::{allowed, permit};
use veyndo_intelligence::domain::price_guardrails::check;
use veyndo_intelligence::knowledge::sync_product;
use veyndo_intelligence::service::autonomy::{Autonomous, reserve};
use veyndo_shop::Settings;

/// Applies an approved proposal; work plans go through their own approval.
pub async fn apply<H: WorkHost>(host: &H, t: &str, id: &str, h: &RequestContext) -> Result<Value> {
    let a = host.models().core;
    let work = tasks::has_operations(a, t, id).await?;
    if work {
        return approve(host, t, id, h).await;
    }
    apply_mode(host, t, id, h, false).await
}
/// The JSON fields `admit_actor` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct AdmitActorInput {
    #[serde(deserialize_with = "lenient")]
    actor: Option<String>,
}

/// A merchant approval needs the original actor of the run and every scope it was planned with.
fn admit_actor(
    h: &RequestContext,
    r: &tasks::ProposalTask,
    v: &Value,
    autonomous: bool,
) -> Result<()> {
    let input: AdmitActorInput = view(v);
    if !autonomous
        && (r.run_actor.as_deref().or(input.actor.as_deref()) != header(h, "x-rac-user")
            || r.run_scopes.as_ref().is_some_and(|scopes| {
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
        let n = products::apply_change(tx, t, &c).await?;
        if n != 1 {
            return Err(conflict("Product changed since preview; create a new plan"));
        }
        let row = products::get(tx, t, &product_id).await?;
        sync_product(tx, t, &serde_json::to_value(row).unwrap()).await?;
    }
    if let Some(e) = experience {
        let n = products::replace_experience(tx, t, e, expected).await?;
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
    let evidence = tasks::unapplied_proposal(a, t, id).await?;
    if let Some(evidence) = evidence.as_ref().map(view::<Evidence>)
        && view::<ProposalParts>(&evidence.proposal)
            .translation
            .is_string()
    {
        permit(h, "catalog.write")?;
        let choice: Choice = serde_json::from_value(evidence.translation_inference)
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
    let r = tasks::lock(&mut tx, t, id)
        .await?
        .ok_or(Error(StatusCode::NOT_FOUND, "Task not found".into()))?;
    if r.run_state.as_deref().is_some_and(|s| s != "ready") {
        return Err(conflict("Agent run has no completed proposal to approve"));
    }
    admit_actor(h, &r, &r.proposal, autonomous)?;
    let saved: Evidence = view(&r.proposal);
    let p: Proposal =
        serde_json::from_value(saved.proposal.clone()).map_err(|e| bad(e.to_string()))?;
    admit_proposal(h, &p, autonomous)?;
    if r.applied {
        return Ok(
            json!({"taskId":id,"applied":true,"replayed":true,"translationJobId":p.translation.as_ref().map(|_|id)}),
        );
    }
    let (settings_data, settings_revision) = products::lock_settings(&mut tx, t).await?;
    let settings: Settings =
        serde_json::from_value(settings_data).map_err(|_| bad("Invalid commerce settings"))?;
    if let Some(revision) = saved.policy_revision
        && revision != settings_revision
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
    let products = products::lock(&mut tx, t, ids).await?;
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
        let choice: Choice = serde_json::from_value(saved.translation_inference.clone())
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
    tasks::mark_applied(&mut tx, t, id).await?;
    veyndo_core::store::outbox::publish_on(
        &mut *tx,
        t,
        "merchant.change.applied",
        &json!({"taskId":id,"autonomous":autonomous,"actor":header(h,"x-rac-user")}),
    )
    .await?;
    tx.commit().await?;
    Ok(json!({"taskId":id,"applied":true,"translationJobId":p.translation.as_ref().map(|_|id)}))
}
