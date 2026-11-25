//! Property tests for stock: reservations against the proved stock rule, checked inventory batches and the reorder ceiling.
use proptest::prelude::*;
use serde_json::json;
use veyndo_inventory::InventoryLines;
use veyndo_inventory::domain::reorder::{admit, within_ceiling};
use veyndo_inventory::domain::supply::Link;
use veyndo_pricing::verified_kernel::stock_admissible;

/// A step against one product's stock: reserve for an order, or release that order's hold.
#[derive(Clone, Debug)]
enum Step {
    Reserve { order: usize, quantity: u64 },
    Release { order: usize },
}

fn step() -> impl Strategy<Value = Step> {
    prop_oneof![
        (0_usize..4, 0_u64..40).prop_map(|(order, quantity)| Step::Reserve { order, quantity }),
        (0_usize..4).prop_map(|order| Step::Release { order }),
    ]
}

/// Stock and per-order holds as the allocation ledger keeps them, admitted only by `stock_admissible`.
struct Ledger {
    stock: u64,
    held: [u64; 4],
    ordered: [u64; 4],
}

impl Ledger {
    fn apply(&mut self, step: &Step) {
        match *step {
            Step::Reserve { order, quantity } => {
                if stock_admissible(self.stock, quantity) {
                    // The admitted quantity is never above the stock: a checked subtraction cannot fail.
                    self.stock = self.stock.checked_sub(quantity).expect("stock below zero");
                    self.held[order] += quantity;
                    self.ordered[order] += quantity;
                }
            }
            Step::Release { order } => {
                self.stock += std::mem::take(&mut self.held[order]);
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Lean `stock_exact`, `stock_positive_bounded`, `stock_conservation`: admitted means 0 < quantity <= stock.
    #[test]
    fn the_stock_rule_is_exact(stock in any::<u64>(), quantity in any::<u64>()) {
        let admitted = stock_admissible(stock, quantity);
        prop_assert_eq!(admitted, quantity > 0 && quantity <= stock);
        if admitted {
            prop_assert_eq!(stock - quantity + quantity, stock);
        }
    }

    /// Any sequence of reservations and releases keeps stock non-negative, conserves units
    /// (stock plus holds is the opening stock), keeps every hold within what the order ordered,
    /// and releasing every order restores the opening quantity (Lean `stock_sequence_*`).
    #[test]
    fn reservations_never_go_below_zero_and_release_restores(
        opening in 0_u64..100,
        steps in prop::collection::vec(step(), 0..40),
    ) {
        let mut ledger = Ledger { stock: opening, held: [0; 4], ordered: [0; 4] };
        for step in &steps {
            ledger.apply(step);
            prop_assert_eq!(ledger.stock + ledger.held.iter().sum::<u64>(), opening);
            for order in 0..4 {
                prop_assert!(ledger.held[order] <= ledger.ordered[order]);
            }
        }
        for order in 0..4 {
            ledger.apply(&Step::Release { order });
        }
        prop_assert_eq!(ledger.stock, opening);
    }

    /// A batch keeps ids and quantities aligned and admits only quantities in 1..=i32::MAX.
    #[test]
    fn inventory_batches_are_aligned_and_positive(lines in prop::collection::vec(("[a-z]{1,8}", any::<u32>()), 0..20)) {
        let batch = InventoryLines::new(lines.iter().map(|(id, quantity)| (id.as_str(), *quantity)));
        let valid = lines.iter().all(|(_, quantity)| (1..=i32::MAX as u32).contains(quantity));
        prop_assert_eq!(batch.is_ok(), valid);
        if let Ok(batch) = batch {
            prop_assert_eq!(batch.ids().len(), lines.len());
            for ((id, quantity), (batch_id, batch_quantity)) in lines.iter().zip(batch.ids().iter().zip(batch.quantities())) {
                prop_assert_eq!(id.as_str(), *batch_id);
                prop_assert_eq!(i64::from(*quantity), i64::from(*batch_quantity));
            }
        }
    }

    /// A reorder proposal is approvable only in the pricing currency, with both an exact total and a ceiling, at or below it.
    #[test]
    fn reorder_approval_respects_the_ceiling(same in any::<bool>(), total in prop::option::of(any::<u64>()), ceiling in prop::option::of(any::<u64>())) {
        prop_assert_eq!(
            within_ceiling(same, total, ceiling),
            same && matches!((total, ceiling), (Some(t), Some(c)) if t <= c)
        );
    }

    /// An admitted supplier offer is the saved line and never above the ceiling.
    #[test]
    fn an_admitted_offer_is_never_above_the_ceiling(
        total in any::<u64>(),
        ceiling in any::<u64>(),
        quantity in 1_i32..1000,
        offered in 1_i64..1000,
        same_product in any::<bool>(),
    ) {
        let link = Link {
            product_id: "desk".into(),
            supplier_shop: "supplier".into(),
            supplier_product_id: "lamp".into(),
            reorder_point: 5,
            reorder_quantity: quantity,
        };
        let offer = json!({
            "lineItems":[{"id": if same_product { "lamp" } else { "mug" }, "quantity": offered}],
            "totalMinor": total.to_string(),
            "currency": "EUR"
        });
        let admitted = admit(&offer, &link, "EUR", ceiling);
        prop_assert_eq!(admitted.is_ok(), same_product && offered == i64::from(quantity) && total <= ceiling);
        if let Ok(value) = admitted {
            prop_assert_eq!(value, total);
        }
    }
}
