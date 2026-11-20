//! Reusable input-only selections resolve current package/library revisions before one native cart mutation.
use super::*;
pub(crate) use veyndo_app_platform::domain::configuration_selection::{Selection, selection_shape};
pub(crate) async fn resolve_selection(
    a: &App,
    t: &str,
    selection: &Selection,
) -> Result<Configuration> {
    let m = package(a, t, &selection.app, true).await?;
    let contract = m
        .configuration
        .as_ref()
        .ok_or(bad("App configuration is unavailable"))?;
    let record = configuration_inputs::record(contract, &selection.fields)?;
    configuration_inputs::arguments(contract, &selection.fields)?;
    let guest = runtime::prepare(contract).await?;
    let mut tx = a.db.begin().await?;
    let rule = records::get(&mut tx, t, &selection.app, &contract.entity, record, false)
        .await?
        .ok_or(bad("App configuration data missing"))?;
    tx.rollback().await?;
    let fee = runtime::configured_contribution(
        guest,
        contract,
        rule[&contract.price_field]
            .as_i64()
            .ok_or(bad("App price missing"))?,
        &selection.fields,
    )
    .await?;
    let rule_fields = contract
        .inputs
        .as_ref()
        .filter(|inputs| inputs.record_field.is_some())
        .and_then(|_| {
            m.entities
                .iter()
                .find(|entity| entity.name == contract.entity && entity.public_read)
        })
        .map(|entity| records::project(entity, &rule));
    let main_locale = commerce::config(a, t).await?.0.main_locale.clone();
    let presentation =
        configuration_summary::snapshot(&m, &selection.fields, rule_fields.as_ref(), main_locale);
    Ok(Configuration {
        app: selection.app.clone(),
        product_id: selection.product_id.clone(),
        fields: selection.fields.clone(),
        fee_minor: fee,
        currency: contract.currency.clone(),
        rule_fields,
        presentation,
        rule_revision: rule["revision"]
            .as_i64()
            .ok_or(bad("App revision missing"))?,
        app_version: m.version,
        entity: contract.entity.clone(),
        record: record.into(),
    })
}
pub(crate) async fn resolve_selections(
    a: &App,
    c: &StoredCart,
    selections: &[Selection],
) -> Result<HashMap<String, Configuration>> {
    selection_shape(&c.data.items, selections)?;
    let mut resolved = HashMap::new();
    for selection in selections {
        let config = resolve_selection(a, &c.tenant, selection).await?;
        resolved.insert(format!("{}:{}", config.product_id, config.app), config);
    }
    Ok(resolved)
}
