//! Model navigation names native views, never URLs or authority. Admission rechecks current rights and tenant-owned records.
use crate::store::product_reads::entity_exists;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use veyndo_core::{RequestContext, Result, State, bad};
use veyndo_identity::{merchant, permit};

/// The list view entity a reviewed working view addresses.
pub trait Entity {
    /// `products`, `orders` or `customers`.
    fn entity(&self) -> &str;
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
/// A native Studio place, optionally with one owned record.
pub struct Navigation {
    /// The native view name.
    pub view: String,
    /// The record the view opens.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}
const VIEWS: &[(&str, &str)] = &[
    ("productData", "catalog.read"),
    ("orders", "orders.read"),
    ("customers", "customers.read"),
    ("overview", "overview.read"),
    ("knowledge", "knowledge.read"),
    ("commerce", "settings.read"),
    ("channels", "channels.read"),
    ("design", "channels.read"),
    ("automation", "automation.read"),
    ("apps", "apps.read"),
    ("storyfronts", "apps.read"),
    ("developers", "developer.read"),
    ("environments", "environments.read"),
    ("users", "team.manage"),
    ("b2b", "b2b.read"),
    ("pos", "pos.use"),
];
/// The schema of a navigation target.
pub fn schema() -> Value {
    json!({"type":["object","null"],"additionalProperties":false,"properties":{
        "view":{"type":"string","enum":VIEWS.iter().map(|(v,_)|*v).collect::<Vec<_>>()},
        "id":{"type":["string","null"],"minLength":1,"maxLength":160}
    },"required":["view","id"]})
}
/// A navigation target and a working view must address the same entity.
pub fn validate_view(target: Option<&Navigation>, view: Option<&impl Entity>) -> Result<()> {
    if let (Some(target), Some(view)) = (target, view) {
        let entity = match target.view.as_str() {
            "productData" => Some("products"),
            "orders" => Some("orders"),
            "customers" => Some("customers"),
            _ => None,
        };
        if entity.is_some_and(|entity| entity != view.entity()) {
            return Err(bad(
                "Navigation and working view address different entities; preserve the verified navigation target and use its matching working-view entity",
            ));
        }
    }
    Ok(())
}
/// Admits a navigation target with current rights and an owned record.
pub async fn admit(a: &State, h: &RequestContext, target: &Navigation) -> Result<()> {
    let right = VIEWS
        .iter()
        .find(|(v, _)| *v == target.view)
        .ok_or(bad("Unknown native view"))?
        .1;
    permit(h, right)?;
    if ["orders", "customers"].contains(&target.view.as_str()) {
        permit(h, "customers.pii")?;
    }
    if target.view == "design" {
        permit(h, "catalog.read")?;
    }
    if let Some(id) = &target.id {
        if id.is_empty() || id.len() > 160 {
            return Err(bad("Invalid navigation reference"));
        }
        if !["productData", "orders", "customers"].contains(&target.view.as_str()) {
            return Err(bad("This native view does not accept an entity reference"));
        }
        let exists = entity_exists(&a.db, &target.view, &merchant(h)?, id)
            .await?
            .ok_or(bad("This native view does not accept an entity reference"))?;
        if !exists {
            return Err(bad("Navigation reference is unavailable in this shop"));
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn product_navigation_cannot_open_an_order_working_view() {
        let target = Navigation {
            view: "productData".into(),
            id: Some("bag".into()),
        };
        struct View {
            entity: &'static str,
        }
        impl Entity for View {
            fn entity(&self) -> &str {
                self.entity
            }
        }
        let mut view = View { entity: "orders" };
        assert!(validate_view(Some(&target), Some(&view)).is_err());
        view.entity = "products";
        assert!(validate_view(Some(&target), Some(&view)).is_ok());
        assert!(validate_view(None, Some(&view)).is_ok());
    }
    #[test]
    fn navigation_cannot_supply_urls_or_permissions() {
        veyndo_core::model_contract::validate_native(
            &schema(),
            &json!({"view":"productData","id":"sku"}),
        )
        .unwrap();
        for value in [
            json!({"view":"https://example.com","id":null}),
            json!({"view":"orders","id":null,"tenant":"foreign"}),
            json!({"view":"commerce","id":null,"script":"alert(1)"}),
        ] {
            assert!(veyndo_core::model_contract::validate_native(&schema(), &value).is_err());
        }
    }
}
