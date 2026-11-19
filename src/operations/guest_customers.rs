//! Merchant-only guest contact projection; order snapshots never create account authority.
use crate::*;

pub(super) async fn contact(a: &App, t: &str, email: &str) -> Result<Value> {
    veyndo_customers::service::directory::guest(&a.core, t, email).await
}
