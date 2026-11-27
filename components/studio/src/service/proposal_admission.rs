//! Native admission of model proposals and list views; every initial or repaired proposal traverses the same read-only checks and receives trusted aggregate and content clocks.
use super::work_plan::bind;
use crate::domain::grid_criteria::validate as validate_criteria;
use crate::domain::proposal::{Proposal, View, validate_proposal};
use crate::domain::view_components::validate as validate_components;
use crate::domain::views::Criteria;
use crate::ports::WorkHost;
use crate::store::products;
use serde::Deserialize;
use serde_json::Value;
use serde_json::json;
use veyndo_app_model::Object;
use veyndo_app_platform::store::extension_filter::admit as admit_filter;
use veyndo_catalog::service::translation_apply::admit_selection;
use veyndo_core::document::{lenient, merge, view};
use veyndo_core::{RequestContext, Result, State, StatusCode, bad};
use veyndo_identity::{allowed, permit};
use veyndo_intelligence::domain::price_guardrails::check;
use veyndo_intelligence::service::navigation::{admit as admit_navigation, validate_view};
use veyndo_pricing::catalog::Product;
use veyndo_shop::service::read_cache::config;

/// Admits a list view with the current entity and PII rights, native criteria and admitted app filters.
pub async fn admit_view(a: &State, h: &RequestContext, view: &mut View) -> Result<()> {
    validate_components(&view.entity, &view.components)?;
    permit(
        h,
        match view.entity.as_str() {
            "orders" => "orders.read",
            "customers" => "customers.read",
            "products" => "catalog.read",
            _ => return Err(bad("Unknown agent grid")),
        },
    )?;
    if view.entity != "products" {
        permit(h, "customers.pii")?;
    }
    if !allowed(h, "knowledge.read") {
        view.components
            .retain(|component| component != "shippingPolicy");
    }
    view.criteria = validate_criteria(&view.entity, &view.criteria)?;
    if let Some(filter) = veyndo_core::document::view::<Criteria>(&view.criteria)
        .app_filter
        .as_deref()
    {
        let object = match view.entity.as_str() {
            "orders" => Object::Order,
            "customers" => Object::Customer,
            _ => Object::Product,
        };
        admit_filter(a, h, object, filter).await?;
    }
    Ok(())
}

/// Checks every price change of a plan against the current guardrails and returns the settings revision.
/// A corridor conflict does not refuse the plan: the twin records it as a blocking finding the merchant sees, and apply
/// still refuses it. Configuration errors (unknown currency, invalid tax basis) still refuse the candidate.
async fn validate_plan(a: &State, t: &str, p: &Proposal, ps: &[Product]) -> Result<i64> {
    let (settings, revision) = config(a, t).await?;
    for c in &p.changes {
        let product = ps
            .iter()
            .find(|p| p.id == c.product_id)
            .ok_or(bad("Unknown product"))?;
        match check(&settings, product, c, false) {
            Err(e) if e.0 == StatusCode::CONFLICT => {}
            other => other?,
        }
    }
    Ok(revision)
}
/// The JSON fields `admit` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct AdmitInput {
    products: Value,
    experience: Value,
    #[serde(deserialize_with = "lenient")]
    experience_revision: Option<i64>,
}

/// Native read-only admission of a model proposal: navigation, view, work, products, app change, experience and translation.
pub async fn admit<H: WorkHost>(
    host: &H,
    t: &str,
    authority: &RequestContext,
    prepared: &Value,
    locale: &str,
    candidate: &Value,
) -> Result<(Proposal, Vec<Product>, i64)> {
    let input: AdmitInput = view(prepared);
    let a = host.models().core;
    let mut ps: Vec<Product> = serde_json::from_value(input.products.clone())
        .map_err(|_| bad("Invalid saved agent context"))?;
    let (commerce_settings, _) = config(a, t).await?;
    let mut p: Proposal = serde_json::from_value(candidate.clone())
        .map_err(|e| bad(format!("Invalid model proposal: {e}")))?;
    if (p.view.is_some() || p.navigation.is_some())
        && (!p.operations.is_empty()
            || !p.changes.is_empty()
            || p.app_action.is_some()
            || p.experience.is_some()
            || p.translation.is_some())
    {
        return Err(bad("Review list views separately from mutations"));
    }
    validate_view(p.navigation.as_ref(), p.view.as_ref())?;
    if let Some(target) = &p.navigation {
        admit_navigation(a, authority, target).await?;
    }
    if let Some(view) = &mut p.view {
        admit_view(a, authority, view).await?;
    }
    if !p.operations.is_empty() {
        if !p.changes.is_empty()
            || p.app_action.is_some()
            || p.experience.is_some()
            || p.translation.is_some()
        {
            return Err(bad(
                "Review native work separately from legacy commerce changes",
            ));
        }
        bind(host, t, authority, &mut p.operations).await?;
    }
    // Read tools may discover another requested SKU; only hydrate its current owning-tenant row.
    let requested = p
        .changes
        .iter()
        .map(|c| c.product_id.clone())
        .collect::<Vec<_>>();
    if requested.len() > 100 {
        return Err(bad("Proposal product budget exceeded"));
    }
    for item in products::requested(a, t, &requested).await? {
        if !ps.iter().any(|p| p.id == item.id) {
            ps.push(item);
        }
    }
    if let Some(change) = &mut p.app_action {
        host.bind_change(t, change, authority).await?;
    }
    // Concurrency tokens are trusted state, never facts invented by the model.
    let ids = p
        .changes
        .iter()
        .map(|c| c.product_id.clone())
        .collect::<Vec<_>>();
    let clocks = if ids.is_empty() {
        vec![]
    } else {
        let mut tx = a.db.begin().await?;
        products::clocks(&mut tx, t, &ids).await?
    };
    for c in &mut p.changes {
        let before = ps
            .iter()
            .find(|p| p.id == c.product_id)
            .ok_or(bad("Unknown product in plan"))?;
        c.expected_revision = before.revision;
        // The content clock belongs to the row the model saw only while its aggregate revision is unchanged.
        c.expected_content_revision = clocks
            .iter()
            .find(|k| k.id == c.product_id && k.revision == before.revision)
            .map(|k| k.content_revision);
        if c.price == Some(before.price) {
            c.price = None;
        }
        if c.stock == Some(before.stock) {
            c.stock = None;
        }
    }
    p.changes.retain(|c| c.price.is_some() || c.stock.is_some());
    if p.experience.as_ref() == Some(&input.experience.clone()) {
        p.experience = None;
        p.expected_experience_revision = None;
    } else if p.experience.is_some() {
        p.expected_experience_revision = Some(input.experience_revision.unwrap());
    }
    if let Some(experience) = p.experience.as_mut() {
        merge(experience, json!({ "headlineLocale": locale }))?;
    }
    if p.translation.as_ref().is_some_and(|l| {
        !commerce_settings.locales.contains(l) || *l == commerce_settings.main_locale
    }) {
        return Err(bad("Select an enabled translation target"));
    }
    if let Some(ids) = &p.translation_products {
        let mut tx = a.db.begin().await?;
        admit_selection(&mut tx, t, Some(ids)).await?;
    }
    validate_proposal(&p, &ps)?;
    let policy_revision = validate_plan(a, t, &p, &ps).await?;
    Ok((p, ps, policy_revision))
}
