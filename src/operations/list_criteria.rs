//! Bounded native order/customer filter and sort contracts, reused by HTTP, MCP and reviewed grid plans.
use crate::*;
pub(crate) use veyndo_orders::types::list_criteria::Criteria;

/// The native bounds plus the admission of app-contributed filters.
pub(crate) fn validate(c: &Criteria) -> Result<i64> {
    let limit = c.validate()?;
    apps::extension_filter::decode(&c.app_filter)?;
    Ok(limit)
}
