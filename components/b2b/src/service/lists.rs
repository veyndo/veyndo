//! Reusable private/shared procurement lists; applying them reuses native cart admission and pricing.
use crate::ports::ListCarts;
use crate::store::access::{access, identity, require};
use crate::store::lists as list_rows;
use crate::store::{catalog, organization};
use crate::types::events::ListChanged;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use veyndo_app_model::declarations::identifier;
use veyndo_core::document::{lenient, view};
use veyndo_core::shop_scope::{channel, tenant};
use veyndo_core::store::history_context::{context, customer_context};
use veyndo_core::{Error, RequestContext, Result, State, StatusCode, bad, conflict};
use veyndo_shop::domain::cart_lines::validate as validate_items;
use veyndo_shop::domain::configuration_selection::{Selection, selection_shape};
use veyndo_shop::ports::Carts;
use veyndo_shop::service::read_cache::conditions;
use veyndo_shop::store::settings::scoped_locked;
use veyndo_shop::types::cart::Item;
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// A saved list: localized name, sharing, lines and app configuration selections.
struct ListData {
    name: HashMap<String, String>,
    shared: bool,
    items: Vec<Item>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    configurations: Vec<Selection>,
}
/// The list operations of a unit member: list, save, delete and apply to the cart.
/// The input of a shopping list operation: the unit, the list, its revision and its fields.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ListInput {
    #[serde(deserialize_with = "lenient")]
    unit_id: Option<String>,
    data: Value,
    #[serde(deserialize_with = "lenient")]
    id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    revision: Option<i64>,
}

/// The JSON fields `invoke` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct InvokeInput {
    #[serde(deserialize_with = "lenient")]
    cart_revision: Option<i64>,
}

pub async fn invoke(
    a: &State,
    ports: (&dyn Carts, &dyn ListCarts),
    h: &RequestContext,
    op: &str,
    v: &Value,
) -> Result<Value> {
    let args: InvokeInput = view(v);
    let (carts, lists) = ports;
    let t = tenant(h)?;
    let input: ListInput = view(v);
    let unit = input.unit_id.as_deref().ok_or(bad("Unit required"))?;
    let candidate = if op == "list.save" {
        let data: ListData =
            serde_json::from_value(input.data.clone()).map_err(|_| bad("Invalid list fields"))?;
        validate_items(&data.items)?;
        selection_shape(&data.items, &data.configurations)?;
        let mut admission = a.db.begin().await?;
        let customer = identity(&mut admission, h).await?;
        let policy = access(&mut admission, &t, unit, &customer).await?;
        require(&policy, "lists")?;
        for item in &data.items {
            catalog::admit_unit(&mut admission, &t, unit, &item.id, &policy).await?;
        }
        admission.rollback().await?;
        for selection in &data.configurations {
            lists.check(&t, selection).await?;
        }
        Some(data)
    } else {
        None
    };
    let mut tx = a.db.begin().await?;
    context(&mut tx, h, "b2b.procurement").await?;
    let customer = identity(&mut tx, h).await?;
    customer_context(&mut tx, &customer).await?;
    let policy = access(&mut tx, &t, unit, &customer).await?;
    require(&policy, "lists")?;
    if op == "lists" {
        let rows = list_rows::visible(&mut tx, &t, unit, &customer).await?;
        return Ok(
            json!({"elements":rows.into_iter().map(|r|json!({"id":r.id,"data":r.data,"revision":r.revision,"ownerId":r.owner_id})).collect::<Vec<_>>()}),
        );
    }
    let id = input
        .id
        .as_deref()
        .filter(|s| identifier(s))
        .ok_or(bad("Valid list ID required"))?;
    let expected = input
        .revision
        .filter(|r| *r >= 0)
        .ok_or(bad("List revision required"))?;
    list_rows::serialize(&mut tx, &t, id).await?;
    let row = list_rows::locked(&mut tx, &t, id).await?;
    if let Some(r) = &row
        && (r.unit_id != unit || (r.owner_id != customer && !r.shared))
    {
        return Err(Error(StatusCode::NOT_FOUND, "List not found".into()));
    }
    if row.as_ref().map(|r| r.revision).unwrap_or(0) != expected {
        return Err(conflict("List changed; reload before editing"));
    }
    if op == "list.apply" {
        let r = row.ok_or(Error(StatusCode::NOT_FOUND, "List not found".into()))?;
        let data: ListData =
            serde_json::from_value(r.data).map_err(|_| bad("Invalid saved list"))?;
        tx.commit().await?;
        let c = carts.load(h).await?;
        if c.data.b2b.as_ref().is_none_or(|c| c.unit_id != unit) {
            return Err(bad("Select the list's purchasing unit before applying it"));
        }
        let expected = args
            .cart_revision
            .ok_or(bad("Current cart revision required"))?;
        return lists
            .apply(h, data.items, expected, &data.configurations)
            .await;
    }
    // Shared lists are readable by peers, but only their owner can edit/delete them.
    if row.as_ref().is_some_and(|r| r.owner_id != customer) {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Only the list owner may edit or delete it".into(),
        ));
    }
    if op == "list.delete" {
        if row.is_none() {
            return Err(Error(StatusCode::NOT_FOUND, "List not found".into()));
        }
        list_rows::delete(&mut tx, &t, id).await?;
    } else {
        let data = candidate.ok_or(bad("Invalid list operation"))?;
        let (settings, _) = scoped_locked(&mut tx, &t, channel(h), &conditions).await?;
        organization::name(&data.name, &settings)?;
        validate_items(&data.items)?;
        selection_shape(&data.items, &data.configurations)?;
        let ids: Vec<_> = data.items.iter().map(|i| i.id.clone()).collect();
        let count = list_rows::products_found(&mut tx, &t, &ids).await?;
        if count != data.items.len() as i64 {
            return Err(bad("List contains missing, duplicate or foreign products"));
        }
        let owner = list_rows::Owner {
            unit,
            owner: &customer,
        };
        list_rows::upsert(&mut tx, &t, id, owner, &json!(data)).await?;
    }
    let changed = ListChanged {
        list_id: id,
        unit_id: unit,
        operation: op,
        revision: expected + 1,
    };
    organization::event(&mut tx, &t, &changed).await?;
    tx.commit().await?;
    Ok(json!({"id":id,"revision":expected+1,"deleted":op=="list.delete"}))
}
