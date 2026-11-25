//! Tests that kill the mutants `cargo-mutants` found surviving in inventory `domain/`: each pins one boundary or clause of the supplier link rules.
use veyndo_inventory::domain::supply::{Config, Link};

fn link(product: &str) -> Link {
    Link {
        product_id: product.into(),
        supplier_shop: "supplier".into(),
        supplier_product_id: "lamp".into(),
        reorder_point: 5,
        reorder_quantity: 10,
    }
}

fn config(links: Vec<Link>, limit: Option<u64>) -> Config {
    Config {
        max_order_minor: limit,
        links,
    }
}

#[test]
fn identifiers_admit_dashes_and_underscores_only() {
    for id in ["desk-1", "desk_1", "Desk9"] {
        assert!(config(vec![link(id)], None).validate().is_ok(), "{id}");
    }
    for id in ["desk.1", "desk 1", ""] {
        assert!(config(vec![link(id)], None).validate().is_err(), "{id}");
    }
}

#[test]
fn at_most_two_hundred_links() {
    let links = |n: usize| (0..n).map(|i| link(&format!("p{i}"))).collect::<Vec<_>>();
    assert!(config(links(200), None).validate().is_ok());
    assert!(config(links(201), None).validate().is_err());
}

#[test]
fn the_ceiling_admits_exactly_one_trillion_minor_units() {
    assert!(config(vec![], Some(1_000_000_000_000)).validate().is_ok());
    assert!(config(vec![], Some(1_000_000_000_001)).validate().is_err());
}

#[test]
fn each_supplier_identifier_is_checked_on_its_own() {
    let mut shop = link("desk");
    shop.supplier_shop = "bad shop".into();
    assert!(config(vec![shop], None).validate().is_err());
    let mut product = link("desk");
    product.supplier_product_id = "bad/product".into();
    assert!(config(vec![product], None).validate().is_err());
}
