//! Inventory component: stock writes, the allocation ledger of orders, test stock and supplier links.
pub mod domain;
pub mod ports;
pub mod service;
pub mod store;
pub mod types;
pub use domain::lines::InventoryLines;
