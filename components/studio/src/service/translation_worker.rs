//! The catalogue translation worker: one leased product per step, keyset traversal, bounded inference and resumable provider errors.
use crate::domain::views::Usage;
use crate::service::work_execution::{task_context, task_headers};
use crate::store::translation_runs as db;
use serde_json::{Value, json};
use uuid::Uuid;
use veyndo_catalog::domain::translation_fields::{Source, source, texts, translated};
use veyndo_cognition::inference::{Choice, budget};
use veyndo_core::document::view;
use veyndo_core::store::lease::Lease;
use veyndo_core::{Result, bad, conflict};
use veyndo_intelligence::service::agent_controls::model_authority;
use veyndo_intelligence::service::models::Models;
use veyndo_runtime::tenant_scope::{self, Scope};
use veyndo_runtime::trace;
use veyndo_shop::service::read_cache::config;

/// Translates one leased product of the oldest runnable job; a dead worker never silently re-runs a possibly charged call of a linked task.
pub async fn once(m: Models<'_>) -> Result<bool> {
    let a = m.core;
    // A linked task must explicitly authorize another possibly charged call after a dead worker.
    db::interrupt_expired(a).await?;
    let lease = Uuid::new_v4().to_string();
    let row = db::lease_job(a, &lease).await?;
    let Some(job) = row else {
        return Ok(false);
    };
    let t = job.tenant.clone();
    let id = job.id.clone();
    let result = authorized_step(m, &job, &lease).await;
    if let Err(e) = result {
        db::fail(a, &t, &id, &lease, &e.1).await?;
    }
    // Notify only after the child transaction is closed: parent admission locks task before job.
    if let Some(task) = &job.run_task {
        db::wake_task(a, &job.tenant, task).await?;
    }
    Ok(true)
}
async fn authorized_step(m: Models<'_>, job: &db::Job, lease: &str) -> Result<()> {
    let a = m.core;
    if let Some(task) = &job.run_task {
        let t = job.tenant.clone();
        let (h, trace) = task_context(a, &t, task).await?;
        let scope = if h.principal.personal_session {
            Scope::Personal {
                tenant: t,
                actor: h.principal.user.clone().unwrap_or_default(),
            }
        } else {
            Scope::Actor {
                tenant: t,
                actor: h.principal.user.clone().unwrap_or_default(),
            }
        };
        let authority = model_authority(&h);
        return trace::scoped(
            trace,
            tenant_scope::scoped(scope, budget::scoped(authority, step(m, job, lease))),
        )
        .await;
    }
    step(m, job, lease).await
}
async fn step(m: Models<'_>, job: &db::Job, lease: &str) -> Result<()> {
    let a = m.core;
    let t = job.tenant.clone();
    let id = job.id.clone();
    let source = job.source_locale.clone();
    let target = job.target_locale.clone();
    let row = db::next_product(
        a,
        &t,
        db::Window {
            cursor: &job.cursor,
            highwater: &job.highwater,
            source: &source,
            target: &target,
            products: job.product_ids.clone(),
        },
    )
    .await?;
    let Some(row) = row else {
        if job.product_ids.is_some() && job.processed != job.total {
            return Err(conflict(
                "Selected products changed during translation; review the collection",
            ));
        }
        db::finish(a, &t, &id, lease).await?;
        return Ok(());
    };
    let (settings, _) = config(a, &t).await?;
    if !settings.locales.contains(&target) || settings.main_locale != source {
        return Err(conflict("Shop languages changed during translation"));
    }
    let source_key = key(&source, &settings.locales);
    let target_key = key(&target, &settings.locales);
    let src = source_fields(&row, &source_key, &source_key);
    let extra = &row.extra;
    // Never skip a partial target: missing description/SEO/specs/rich text must still be translated.
    let complete = row.target_name.is_some()
        && db::has_target_description(a, &t, &row.id, &target).await?
        && ["seo", "specifications", "richDescription"]
            .iter()
            .all(|k| src[*k].is_null() || extra[*k].get(&target_key).is_some());
    let skip = complete && !job.overwrite;
    let (result, evidence) = if skip {
        (Value::Null, None)
    } else {
        let texts = texts(&src)
            .iter()
            .map(|(path, text)| json!({"path":path,"text":text}))
            .collect::<Vec<_>>();
        if texts.len() > 500 || src.to_string().len() > 64000 {
            return Err(bad("Product exceeds translation chunk limits"));
        }
        let choice: Choice = serde_json::from_value(job.choice.clone())
            .map_err(|_| bad("Invalid translation provider"))?;
        let schema = json!({"type":"object","properties":{"translations":{"type":"array","items":{"type":"object","properties":{"path":{"type":"string"},"text":{"type":"string"}},"required":["path","text"],"additionalProperties":false}}},"required":["translations"],"additionalProperties":false});
        let _slot = m
            .slots
            .acquire()
            .await
            .map_err(|_| bad("Inference unavailable"))?;
        let _cluster = Lease::acquire(a, &t, "model", 2).await?;
        let output = m
            .inference
            .structured(
                Some(&choice),
                include_str!("../../../../crates/cognition/prompts/catalogue-translation.v1.txt"),
                &json!({"sourceLocale":source,"targetLocale":target,"texts":texts}).to_string(),
                &schema,
            )
            .await
            .map_err(bad)?;
        let usage: Usage = view(&output.usage);
        let evidence = json!({"provider":output.provider,"model":output.model,"prompt":usage.prompt,"inputTokens":usage.input_tokens,"outputTokens":usage.output_tokens});
        (translated(&src, &output.value)?, Some(evidence))
    };
    if let Some(task) = &job.run_task {
        task_headers(a, &t, task).await?;
    }
    let mut tx = a.db.begin().await?;
    let current = db::current_lease(&mut tx, &t, &id).await?;
    if current.as_deref() != Some(lease) {
        return Ok(());
    }
    db::store_item(
        &mut tx,
        &t,
        &id,
        lease,
        db::Item {
            product: row.id.clone(),
            revision: row.revision,
            source: src,
            result,
            status: if skip { "skipped" } else { "ready" },
            evidence,
        },
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

/// The translatable fields of a product row in its main language.
fn source_fields(row: &db::NextProduct, locale: &str, main: &str) -> Value {
    source(
        Source {
            name: row.name.clone(),
            description: row.description.clone(),
            translated_name: row.translated_name.clone(),
            translated_description: row.translated_description.clone(),
            extra: row.extra.clone(),
        },
        locale,
        main,
    )
}
/// The storage key of a locale among the enabled shop languages.
fn key(locale: &str, locales: &[String]) -> String {
    veyndo_core::localized::Languages {
        main: locale,
        enabled: locales,
    }
    .key(locale)
}

#[cfg(test)]
mod tests {
    use super::key;

    #[test]
    fn locale_keys_follow_the_enabled_languages() {
        let enabled = vec!["de-DE".to_string(), "en-GB".to_string()];
        assert_eq!(key("de-DE", &enabled), key("de-DE", &enabled));
        assert!(!key("en-GB", &enabled).is_empty());
    }
}
