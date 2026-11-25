//! The server's implementations of the ports that lower components declare for tables of components above them; one file per declaring component.
use crate::foundation::App;

mod catalog;
mod customers;
mod pricing;
mod shipping;
mod shop;

/// Registers the ports that lower components look up at run time instead of receiving them from their caller.
pub(crate) fn register(a: &App) {
    catalog::register(a);
    customers::register(a);
    shop::register(a);
}
