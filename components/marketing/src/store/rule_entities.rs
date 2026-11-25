//! Writes rule metadata onto the product, customer or order it describes, bound to the revision the merchant read. Products are written by the catalog store, customers and orders through the records port.
use crate::ports::{RuleEntity, records};
use serde_json::Value;
use veyndo_catalog::store::rule_products;
use veyndo_core::{Result, State, conflict};

pub async fn save(
    a: &State,
    t: &str,
    entity: &str,
    id: &str,
    data: &Value,
    rev: i64,
) -> Result<()> {
    let n = match entity {
        "products" => rule_products::save_automation_at(a, t, id, data, rev).await?,
        "customers" => {
            records()?
                .save_automation_at(a, RuleEntity::Customer, (t, id), data, rev)
                .await?
        }
        _ => {
            records()?
                .save_automation_at(a, RuleEntity::Order, (t, id), data, rev)
                .await?
        }
    };
    if n != 1 {
        return Err(conflict("Rule entity revision changed"));
    }
    Ok(())
}
