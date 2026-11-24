//! Apply at most 50 reviewed drafts per request; stale products become conflicts rather than being overwritten.
use crate::domain::selection;
use crate::domain::translation_fields::merge_metadata;
use crate::ports::Catalog;
use crate::store::{product_edit, translation_jobs};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::PgConnection;
use veyndo_core::document::{lenient, view};
use veyndo_core::{RequestContext, Result, bad, conflict, outbox};
use veyndo_identity::{merchant, permit};
use veyndo_pricing::verified_kernel;

/// An explicit selection must consist of products of this shop.
pub async fn admit_selection(
    tx: &mut PgConnection,
    tenant: &str,
    ids: Option<&[String]>,
) -> Result<()> {
    if let Some(ids) = ids {
        selection::validate(ids)?;
        if translation_jobs::count_products(tx, tenant, ids).await? != ids.len() as i64 {
            return Err(bad("Selected products are not available in this shop"));
        }
    }
    Ok(())
}

/// A draft is applied only to the product revision it was made against.
fn current(product_revision: i64, draft_revision: i64) -> bool {
    u64::try_from(product_revision)
        .ok()
        .zip(u64::try_from(draft_revision).ok())
        .is_some_and(|(current, expected)| verified_kernel::revision_admissible(current, expected))
}

/// The JSON fields `apply` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ApplyInput {
    #[serde(deserialize_with = "lenient")]
    product_id: Option<String>,
}

pub async fn apply(c: &Catalog, h: &RequestContext, id: &str, v: &Value) -> Result<Value> {
    let input: ApplyInput = view(v);
    permit(h, "catalog")?;
    let tenant = merchant(h)?;
    let mut tx = c.core.db.begin().await?;
    let languages = c.shop.languages_shared(&mut tx, &tenant).await?;
    let job = translation_jobs::lock_ready(&mut tx, &tenant, id)
        .await?
        .ok_or(conflict("Translation job is not ready"))?;
    if !languages.enabled.contains(&job.target_locale) || languages.main != job.source_locale {
        return Err(conflict("Shop languages changed during translation"));
    }
    let key = languages.as_ref().key(&job.target_locale);
    let drafts =
        translation_jobs::lock_drafts(&mut tx, &tenant, id, input.product_id.as_deref()).await?;
    let mut applied = 0;
    let mut conflicts = 0;
    for draft in drafts {
        let product = translation_jobs::lock_product(&mut tx, &tenant, &draft.product_id).await?;
        let status = if let Some((_, mut extra)) =
            product.filter(|(revision, _)| current(*revision, draft.revision))
        {
            merge_metadata(&mut extra, &draft.result, &key)?;
            product_edit::upsert_translation(
                &mut tx,
                &tenant,
                &draft.product_id,
                &job.target_locale,
                draft.result["name"].as_str().map(str::to_owned),
                draft.result["description"].as_str().map(str::to_owned),
            )
            .await?;
            translation_jobs::update_metadata(&mut tx, &tenant, &draft.product_id, extra).await?;
            outbox::publish(
                &mut tx,
                &tenant,
                "product.updated",
                translation_jobs::applied_event(
                    &draft.product_id,
                    &job.target_locale,
                    draft.revision + 1,
                    id,
                ),
            )
            .await?;
            applied += 1;
            "applied"
        } else {
            conflicts += 1;
            "conflict"
        };
        translation_jobs::set_draft_status(&mut tx, &tenant, id, &draft.product_id, status).await?;
    }
    let remaining = translation_jobs::ready_drafts(&mut tx, &tenant, id).await?;
    tx.commit().await?;
    Ok(json!({"applied":applied,"conflicts":conflicts,"remaining":remaining}))
}

#[cfg(test)]
mod tests {
    use super::current;

    #[test]
    fn a_draft_applies_only_to_the_revision_it_was_made_against() {
        assert!(current(4, 4));
        assert!(!current(5, 4));
        assert!(!current(3, 4));
        assert!(!current(-1, -1));
    }
}
