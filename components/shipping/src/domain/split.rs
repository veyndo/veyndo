//! Provider-neutral partial-package allocation; quantities and fiscal order snapshots remain conserved.
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{lenient, merge, view};
use veyndo_core::{Result, bad, conflict};
use veyndo_pricing::verified_kernel;

/// Splits the requested quantities off an open package: the original keeps the rest, the new package costs nothing.
/// `terminal` says the order's delivery is already closed.
/// The shipment being split: its positions, state, label and carrier booking.
#[derive(Deserialize, Default)]
#[serde(default)]
struct ShipmentSource {
    #[serde(deserialize_with = "lenient")]
    positions: Option<Vec<Value>>,
    state: Value,
    label: Value,
    booking: Value,
}

/// One shipment position: a product and its quantity.
#[derive(Deserialize, Default)]
#[serde(default)]
struct ShipmentPosition {
    #[serde(deserialize_with = "lenient")]
    id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    quantity: Option<u64>,
}

pub fn allocate(
    source: &Value,
    requested: &Value,
    terminal: bool,
    id: &str,
) -> Result<(Value, Value)> {
    let shipment: ShipmentSource = view(source);
    let positions = shipment
        .positions
        .as_ref()
        .ok_or(bad("Shipment has no positions"))?;
    let listed = positions
        .iter()
        .map(view::<ShipmentPosition>)
        .collect::<Vec<_>>();
    let requested = requested
        .as_array()
        .filter(|v| !v.is_empty() && v.len() <= 100)
        .ok_or(bad("Select shipment positions and quantities"))?
        .iter()
        .map(view::<ShipmentPosition>)
        .collect::<Vec<_>>();
    let mut amounts = std::collections::BTreeMap::new();
    let mut total = 0_u64;
    for p in &listed {
        let sku = p.id.as_deref().ok_or(bad("Invalid shipment position"))?;
        let q = p
            .quantity
            .filter(|q| *q > 0 && *q <= u32::MAX as u64)
            .ok_or(bad("Invalid shipment quantity"))?;
        if amounts.insert(sku, q).is_some() {
            return Err(bad("Duplicate shipment position"));
        }
        total = total
            .checked_add(q)
            .ok_or(bad("Shipment quantity overflow"))?;
    }
    let mut moved = 0_u64;
    let mut chosen = std::collections::BTreeMap::new();
    for p in &requested {
        let sku = p.id.as_deref().ok_or(bad("Product identity required"))?;
        let q = p
            .quantity
            .filter(|q| *q > 0)
            .ok_or(bad("Shipment quantities must be positive integers"))?;
        if !amounts.get(sku).is_some_and(|available| q <= *available)
            || chosen.insert(sku, q).is_some()
        {
            return Err(bad(
                "Shipment quantities exceed an available position or repeat it",
            ));
        }
        moved = moved
            .checked_add(q)
            .ok_or(bad("Shipment quantity overflow"))?;
    }
    if !verified_kernel::delivery_split_admissible(shipment.state == "open", terminal, total, moved)
    {
        return Err(conflict(
            "Only open shipments can be split; keep at least one item in the original package",
        ));
    }
    if !shipment.label.is_null() || !shipment.booking.is_null() {
        return Err(conflict(
            "Reconcile the existing carrier label or booking before changing its package",
        ));
    }
    let mut original = source.clone();
    let mut parcel = source.clone();
    let mut kept = Vec::new();
    let mut split = Vec::new();
    for (stored, p) in positions.iter().zip(&listed) {
        let (Some(sku), Some(quantity)) = (p.id.as_deref(), p.quantity) else {
            continue;
        };
        let moved = chosen.get(sku).copied().unwrap_or(0);
        if quantity > moved {
            kept.push(with_quantity(stored, quantity - moved)?);
        }
        if moved > 0 {
            split.push(with_quantity(stored, moved)?);
        }
    }
    merge(&mut original, json!({"positions": kept}))?;
    merge(&mut parcel, json!({"id": id, "positions": split}))?;
    // Splitting fulfillment does not duplicate the original shipping charge or recalculate the tax snapshot.
    merge(
        &mut parcel,
        json!({"shippingCosts":{"unitPrice":0,"totalPrice":0,"netPrice":0,"tax":0,"calculatedTaxes":[]}}),
    )?;
    for field in ["trackingCode", "trackingUrl", "label"] {
        parcel.as_object_mut().unwrap().remove(field);
    }
    Ok((original, parcel))
}

/// A stored position with another quantity; every other field stays as stored.
fn with_quantity(position: &Value, quantity: u64) -> Result<Value> {
    let mut position = position.clone();
    merge(&mut position, json!({"quantity": quantity}))?;
    Ok(position)
}

#[cfg(test)]
mod tests {
    use super::allocate;
    use serde_json::json;
    #[test]
    fn conservation_and_label_fence() {
        let source = json!({"state":"open","positions":[{"id":"a","quantity":3},{"id":"b","quantity":2}],"shippingCosts":{"totalPrice":4.9}});
        let (left, next) = allocate(
            &source,
            &json!([{"id":"a","quantity":2}]),
            false,
            "package-2",
        )
        .unwrap();
        assert_eq!(left["positions"][0]["quantity"], 1);
        assert_eq!(next["positions"][0]["quantity"], 2);
        assert_eq!(next["shippingCosts"]["totalPrice"], 0);
        assert_eq!(left["shippingCosts"]["totalPrice"], 4.9);
        for p in [
            json!([]),
            json!([{"id":"c","quantity":1}]),
            json!([{"id":"a","quantity":4}]),
            json!([{"id":"a","quantity":1},{"id":"a","quantity":1}]),
            json!([{"id":"a","quantity":3},{"id":"b","quantity":2}]),
        ] {
            assert!(allocate(&source, &p, false, "next").is_err());
        }
        let mut labelled = source.clone();
        labelled["label"] = json!({"id":"label"});
        assert!(allocate(&labelled, &json!([{"id":"a","quantity":1}]), false, "next").is_err());
        assert!(allocate(&source, &json!([{"id":"a","quantity":1}]), true, "next").is_err());
    }
}
