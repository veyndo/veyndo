//! Restoring a version: rights, approval and the recorded state are checked here; the owning domain validates the replayed edit.
use super::*;
use veyndo_studio::domain::history_access::restoration;
use veyndo_studio::store::history::record;
pub(super) async fn restore(
    State(a): State<App>,
    h: RequestContext,
    Path((entity, id, version)): Path<(String, String, i64)>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let t = merchant(&a, &h)?;
    if requires_pii(&entity) {
        auth::permit(&h, "customers.pii")?;
    }
    let (read, write) = rights(&entity)?;
    auth::permit(&h, read)?;
    let write = write.ok_or(bad(
        "Order/payment history cannot be rewound; use state-machine actions",
    ))?;
    auth::permit(&h, write)?;
    let (rev, side) = restoration(&v)?;
    let record = record(&a.core, &t, &entity, &id, version).await?;
    let state = record[side].clone();
    if state.is_null() {
        return Err(bad("Cannot restore a non-existent entity"));
    }
    let mut h = h;
    h.insert(
        "x-rac-history-reason",
        format!("restore:{version}:{side}").parse().unwrap(),
    );
    restore::apply(a, h, &entity, id, state, rev).await
}
