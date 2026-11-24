//! The installed payment registry is owned by the payments component; the server keeps these paths for its callers.
pub(crate) use veyndo_orders::domain::order_workflow::external;
pub(crate) use veyndo_payments::store::provider_registry::{
    available, currency_methods, currency_methods_conn, install_methods,
};
