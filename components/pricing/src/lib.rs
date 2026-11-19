//! Pricing component: amounts, price calculation, discounts and the cart, product and customer contracts they work on.
pub use veyndo_kernel::verified_kernel;
pub mod domain;
pub use domain::validation::ValidationError;
pub use domain::{cart, catalog, customer, discount, external_order, inventory, money, pricing};
#[cfg(feature = "service")]
mod conversions;
#[cfg(feature = "service")]
pub mod service;
#[cfg(feature = "service")]
pub mod types;
