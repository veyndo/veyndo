//! Pure rules and data contracts: no database, no transport. This is the part that is proved.
pub mod cart;
pub mod catalog;
pub mod context;
pub mod customer;
pub mod discount;
pub mod external_order;
pub mod money;
pub mod pricing;
pub mod quote;
pub(crate) mod validation;
