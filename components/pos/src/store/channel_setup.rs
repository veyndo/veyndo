//! A new POS channel brings its cash payment and collection delivery as channel overrides; there is no second settings store. The settings rows are read and written through the shop store.
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::{Postgres, Transaction};
use veyndo_core::document::view;
use veyndo_core::{Result, bad};
use veyndo_marketing::types::channel::validate_pos;
use veyndo_shop::Settings;
use veyndo_shop::domain::defaults::decode;
use veyndo_shop::domain::patch::difference;
use veyndo_shop::domain::validation::validate;
use veyndo_shop::service::read_cache::conditions;
use veyndo_shop::store::channel_settings::insert_override;
use veyndo_shop::store::settings::basis;

/// The settings a POS channel runs with: cash payment and immediate collection, priced at zero.
pub fn pos_settings(base: &Settings) -> Result<Settings> {
    let mut next = base.clone();
    if next.payments.iter().any(|p| p.id == "pos-cash") {
        return Err(bad("Reserved POS cash method already exists"));
    }
    next.payments.push(serde_json::from_value(json!({"id":"pos-cash","name":"Cash","translations":{"en":{"name":"Cash"},"de":{"name":"Barzahlung"},"fr":{"name":"Espèces"},"es":{"name":"Efectivo"}},"active":true,"businessOnly":false,"mode":"cash"})).map_err(|_| bad("Invalid POS cash method"))?);
    // Immediate handover shares the ordinary delivery owner. Never reinterpret a paid shipping method.
    next.shipping.retain(|s| s.id != "pickup");
    next.shipping.push(serde_json::from_value(json!({"id":"pickup","name":"Collection","translations":{"en":{"name":"Collection"},"de":{"name":"Mitnahme"},"fr":{"name":"Retrait"},"es":{"name":"Recogida"}},"active":true,"price":0,"freeAbove":null,"minDays":0,"maxDays":0,"countries":base.countries,"taxType":"highest"})).map_err(|_| bad("Invalid POS collection method"))?);
    Ok(next)
}

/// The JSON fields `configure` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ConfigureInput {
    kind: Value,
}

/// Installs the overrides of a newly saved POS channel; called inside the channel save with the settings locked first.
pub async fn configure(
    tx: &mut Transaction<'_, Postgres>,
    t: &str,
    id: &str,
    data: &Value,
    new: bool,
) -> Result<()> {
    let input: ConfigureInput = view(data);
    if input.kind != "pos" || !new {
        return Ok(());
    }
    let base = decode(basis(tx, t).await?)?;
    validate_pos(
        &serde_json::from_value(data.clone()).map_err(|_| bad("Invalid POS channel"))?,
        &base,
    )?;
    let next = pos_settings(&base)?;
    validate(&next, &conditions)?;
    insert_override(tx, t, id, &difference(&base, &next)).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::pos_settings;
    use veyndo_shop::Settings;

    #[test]
    fn a_pos_channel_adds_cash_and_free_collection_once() {
        let base: Settings =
            serde_json::from_str(include_str!("../../../../fixtures/demo-settings.json")).unwrap();
        let next = pos_settings(&base).unwrap();
        assert!(next.payments.iter().any(|p| p.id == "pos-cash"));
        let pickup = next.shipping.iter().filter(|s| s.id == "pickup").count();
        assert_eq!(pickup, 1);
        assert!(pos_settings(&next).is_err());
    }
}
