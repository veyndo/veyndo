//! Native revision-bound proposal validation and persistence shared by every design agent.
use super::DesignRequest;
use serde_json::Value;
use serde_json::json;
use veyndo_app_model::Entity;
use veyndo_app_model::manifest::Manifest;
use veyndo_app_platform::domain::field_values;
use veyndo_app_platform::domain::planning::AppChange;
use veyndo_app_platform::store::managed_data::record_revision;
use veyndo_core::Result;
use veyndo_core::State;
use veyndo_core::bad;
use veyndo_core::conflict;
use veyndo_core::request_context::RequestContext;
use veyndo_core::uid;
use veyndo_identity::merchant;
use veyndo_studio::domain::proposal::Proposal;
pub(super) struct ResultContext<'a> {
    pub instruction: &'a str,
    pub output: &'a Value,
    pub sources: &'a Value,
    pub model: &'a str,
    pub usage: &'a Value,
}
pub(super) async fn prepare(
    a: &State,
    h: &RequestContext,
    m: &Manifest,
    entity: &Entity,
    fields: &Value,
    v: &DesignRequest,
    result: ResultContext<'_>,
) -> Result<Value> {
    let ResultContext {
        instruction,
        output,
        sources,
        model,
        usage,
    } = result;
    let t = merchant(h)?;
    let selected = v.selected.as_str().filter(|s| !s.is_empty());
    let variants = output["variants"]
        .as_array()
        .filter(|r| (1..=3).contains(&r.len()))
        .ok_or(bad("Design provider returned no valid alternatives"))?;
    let mut prepared = vec![];
    let package_revision: i64 = sqlx::query_scalar(
        "SELECT revision FROM app_packages WHERE tenant=$1 AND id='site_builder' AND active",
    )
    .bind(&t)
    .fetch_one(&a.db)
    .await?;
    for candidate in variants {
        let mut next = fields.clone();
        let mut content: Value = if !v.structure() && !v.content() {
            // Style-only generation never needs to reproduce merchant documents.
            fields["content"].clone()
        } else {
            serde_json::from_str(candidate["content_json"].as_str().unwrap_or(""))
                .map_err(|_| bad("Invalid design blocks"))?
        };
        let theme: Value = serde_json::from_str(candidate["theme_json"].as_str().unwrap_or(""))
            .map_err(|_| bad("Invalid design theme"))?;
        if let Some(id) = selected {
            let blocks = content["blocks"]
                .as_array()
                .filter(|r| r.len() == 1 && r[0]["id"] == id)
                .ok_or(bad("Agent changed an unselected section"))?;
            let replacement = blocks[0].clone();
            content = fields["content"].clone();
            let block = content["blocks"]
                .as_array_mut()
                .and_then(|r| r.iter_mut().find(|b| b["id"] == id))
                .ok_or(bad("Selected block disappeared"))?;
            *block = replacement;
            // A section request cannot silently change the whole shop's theme.
            next["theme"] = fields["theme"].clone();
        } else {
            next["theme"] = theme;
        }
        // Reference choices are enforced, not merely included in the model prompt.
        if !v.structure() {
            let before = fields["content"]["blocks"]
                .as_array()
                .ok_or(bad("Invalid current page"))?;
            let after = content["blocks"]
                .as_array()
                .ok_or(bad("Invalid design blocks"))?;
            if before.len() != after.len()
                || before
                    .iter()
                    .zip(after)
                    .any(|(a, b)| a["id"] != b["id"] || a["kind"] != b["kind"])
            {
                return Err(bad("Structure changes were not selected"));
            }
        }
        if !v.content() {
            for block in content["blocks"]
                .as_array_mut()
                .ok_or(bad("Invalid blocks"))?
            {
                if let Some(old) = fields["content"]["blocks"]
                    .as_array()
                    .and_then(|rows| rows.iter().find(|old| old["id"] == block["id"]))
                {
                    for key in [
                        "text",
                        "rich",
                        "products",
                        "productMode",
                        "fields",
                        "image",
                        "extension",
                    ] {
                        if let Some(value) = old.get(key) {
                            block[key] = value.clone();
                        } else {
                            block.as_object_mut().unwrap().remove(key);
                        }
                    }
                } else {
                    return Err(bad("New content requires explicit selection"));
                }
            }
        }
        if !v.style() {
            next["theme"] = fields["theme"].clone();
        }
        next["content"] = content;
        next = crate::normalize(&next)?;
        crate::validate("pages", &next)?;
        field_values::validate(entity, &next)?;
        let mut validation = a.db.begin().await?;
        crate::validate_owned(
            &mut validation,
            &t,
            &next,
            v.id.as_str().ok_or(bad("Page required"))?,
        )
        .await?;
        validation.rollback().await?;
        let args = json!({"id":v.id,"revision":v.revision,"fields":next});
        let p = Proposal {
            navigation: None,
            operations: vec![],
            view: None,
            summary: candidate["summary"]
                .as_str()
                .unwrap_or("")
                .chars()
                .take(500)
                .collect(),
            changes: vec![],
            experience: None,
            expected_experience_revision: None,
            translation: None,
            translation_products: None,
            app_action: Some(AppChange {
                app: "site_builder".into(),
                action: "save_page".into(),
                arguments_json: args.to_string(),
                expected_app_revision: package_revision,
            }),
        };
        prepared.push((uid(), next, p));
    }
    // All alternatives are validated before any review task becomes visible.
    if record_revision(a, &t, m, entity, v.id.as_str().unwrap_or_default()).await?
        != v.revision.as_i64().unwrap_or(-1)
    {
        return Err(conflict("Page changed during design generation"));
    }
    let mut tx = a.db.begin().await?;
    for (id, _, p) in &prepared {
        sqlx::query("INSERT INTO tasks(id,tenant,proposal) VALUES($1,$2,$3)").bind(id).bind(&t).bind(json!({"source":"site.designer","actor":h.principal.user,"instruction":instruction,"proposal":p,"interpretation":output["interpretation"],"sources":sources,"model":model,"usage":usage,"applied":false})).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(
        json!({"interpretation":output["interpretation"],"sources":sources,"variants":prepared.iter().map(|(id,fields,p)|json!({"taskId":id,"fields":fields,"summary":p.summary})).collect::<Vec<_>>(),"approvalRequired":true}),
    )
}
