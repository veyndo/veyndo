//! Side-effect-free rule comparison on bounded, tenant-owned frozen Flow order inputs.
use super::*;
pub(crate) const HISTORY_CAPABILITIES: &[(&str, &str)] = &[(
    "automation.compare_history",
    "Compare a saved rule and draft on frozen order.placed Flow inputs without replaying actions or reading current commerce data",
)];

const SCOPES: &[&str] = &[
    "automation.read",
    "orders.read",
    "catalog.read",
    "customers.read",
    "customers.pii",
    "payments.read",
    "documents.read",
    "b2b.read",
];

pub(super) fn visible(h: &RequestContext) -> bool {
    SCOPES.iter().all(|scope| auth::allowed(h, scope))
}

pub(super) async fn compare(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let t = merchant(&a, &h)?;
    for scope in SCOPES {
        auth::permit(&h, scope)?;
    }
    Ok(Json(
        veyndo_marketing::store::rule_history::compare(&a, &t, v).await?,
    ))
}
