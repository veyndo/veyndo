//! Checked item quantities for inventory writes. Availability and locks belong to the host.
use veyndo_core::{Result, bad};
use veyndo_pricing::verified_kernel;

/// One ordered batch for both the allocation ledger and the stock write.
/// Each line passes the proved per-order bound (`reservation_admissible`): the new order
/// holds nothing yet and reserves exactly its positive ordered quantity, so the ledger never
/// holds more than the order ordered. Release (`release.sql`) is idempotent.
/// The host still owns distinct product admission and transaction locking.
pub struct InventoryLines<'a> {
    ids: Vec<&'a str>,
    quantities: Vec<i32>,
}

impl<'a> InventoryLines<'a> {
    pub fn new(lines: impl ExactSizeIterator<Item = (&'a str, u32)>) -> Result<Self> {
        let mut batch = Self {
            ids: Vec::with_capacity(lines.len()),
            quantities: Vec::with_capacity(lines.len()),
        };
        for (id, quantity) in lines {
            // The order is new, so it holds nothing yet; it reserves exactly what the line orders.
            let ordered = u64::from(quantity);
            let quantity = i32::try_from(quantity)
                .ok()
                .filter(|_| verified_kernel::reservation_admissible(ordered, 0, ordered))
                .ok_or(bad("Invalid inventory quantity"))?;
            batch.ids.push(id);
            batch.quantities.push(quantity);
        }
        Ok(batch)
    }

    pub fn ids(&self) -> &[&'a str] {
        &self.ids
    }

    pub fn quantities(&self) -> &[i32] {
        &self.quantities
    }
}

#[cfg(test)]
mod tests {
    use super::InventoryLines;

    #[test]
    fn signed_quantities_preserve_line_alignment_without_wrapping() {
        let batch =
            InventoryLines::new([("lamp", 2), ("mug", i32::MAX as u32)].into_iter()).unwrap();
        assert_eq!(batch.ids(), ["lamp", "mug"]);
        assert_eq!(batch.quantities(), [2, i32::MAX]);
        for quantity in [0, i32::MAX as u32 + 1, u32::MAX] {
            assert!(InventoryLines::new([("lamp", quantity)].into_iter()).is_err());
        }
    }
}
