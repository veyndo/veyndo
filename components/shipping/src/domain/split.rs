//! Provider-neutral partial-package allocation; quantities and fiscal order snapshots remain conserved.
use serde_json::{Value, json};
use veyndo_core::{Result, bad, conflict};
use veyndo_pricing::verified_kernel;

/// Splits the requested quantities off an open package: the original keeps the rest, the new package costs nothing.
/// `terminal` says the order's delivery is already closed.
pub fn allocate(
    source: &Value,
    requested: &Value,
    terminal: bool,
    id: &str,
) -> Result<(Value, Value)> {
    let positions = source["positions"]
        .as_array()
        .ok_or(bad("Shipment has no positions"))?;
    let requested = requested
        .as_array()
        .filter(|v| !v.is_empty() && v.len() <= 100)
        .ok_or(bad("Select shipment positions and quantities"))?;
    let mut amounts = std::collections::BTreeMap::new();
    let mut total = 0_u64;
    for p in positions {
        let sku = p["id"].as_str().ok_or(bad("Invalid shipment position"))?;
        let q = p["quantity"]
            .as_u64()
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
    for p in requested {
        let sku = p["id"].as_str().ok_or(bad("Product identity required"))?;
        let q = p["quantity"]
            .as_u64()
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
    if !verified_kernel::delivery_split_admissible(
        source["state"] == "open",
        terminal,
        total,
        moved,
    ) {
        return Err(conflict(
            "Only open shipments can be split; keep at least one item in the original package",
        ));
    }
    if !source["label"].is_null() || !source["booking"].is_null() {
        return Err(conflict(
            "Reconcile the existing carrier label or booking before changing its package",
        ));
    }
    let mut original = source.clone();
    let mut parcel = source.clone();
    original["positions"] = json!(
        positions
            .iter()
            .filter_map(|p| {
                let left = p["quantity"].as_u64().unwrap()
                    - chosen.get(p["id"].as_str().unwrap()).copied().unwrap_or(0);
                (left > 0).then(|| {
                    let mut p = p.clone();
                    p["quantity"] = json!(left);
                    p
                })
            })
            .collect::<Vec<_>>()
    );
    parcel["id"] = json!(id);
    parcel["positions"] = json!(
        positions
            .iter()
            .filter_map(|p| chosen.get(p["id"].as_str().unwrap()).map(|q| {
                let mut p = p.clone();
                p["quantity"] = json!(q);
                p
            }))
            .collect::<Vec<_>>()
    );
    // Splitting fulfillment does not duplicate the original shipping charge or recalculate the tax snapshot.
    parcel["shippingCosts"] =
        json!({"unitPrice":0,"totalPrice":0,"netPrice":0,"tax":0,"calculatedTaxes":[]});
    for field in ["trackingCode", "trackingUrl", "label"] {
        parcel.as_object_mut().unwrap().remove(field);
    }
    Ok((original, parcel))
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
