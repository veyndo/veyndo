//! Server implementations of component record ports, installed once at startup before any request or worker runs.
mod b2b;
mod marketing;

/// Installs every component records port the server implements.
pub(crate) fn install() {
    veyndo_b2b::ports::install(Box::new(b2b::ProcurementRecords));
    veyndo_marketing::ports::install(Box::new(marketing::AutomationRecords));
}
