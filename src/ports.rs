//! The server's implementations of the ports that lower components declare for tables of components above them; one file per declaring component.
use crate::foundation::App;

mod customers;
mod pricing;
mod shipping;

/// Registers the ports that lower components look up at run time instead of receiving them from their caller.
pub(crate) fn register(a: &App) {
    customers::register(a);
}
