//! The server's implementations of component ports whose owners the declaring component may not depend on; one file per declaring component.
use crate::foundation::App;

pub(crate) mod apps;
mod b2b;
mod catalog;
mod checkout;
mod customers;
pub(crate) mod inference;
mod marketing;
mod payments;
mod pricing;
mod shipping;
mod shop;

/// Registers the ports that lower components look up at run time instead of receiving them from their caller.
pub(crate) fn register(a: &App) {
    catalog::register(a);
    customers::register(a);
    shop::register(a);
}

/// Registers every process-wide port implementation; repeated calls keep the first registration.
pub(crate) fn install() {
    veyndo_cognition::ports::install(Box::new(inference::Host));
    veyndo_app_platform::ports::install_directory(Box::new(apps::Directory));
    veyndo_b2b::ports::install(Box::new(b2b::ProcurementRecords));
    veyndo_marketing::ports::install(Box::new(marketing::AutomationRecords));
}
