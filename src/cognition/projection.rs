//! Exactly-once local observation projection is owned by the intelligence component; the payment kind of an order is read here.
use super::*;
pub(crate) use veyndo_intelligence::store::observations::observations;
pub(crate) async fn project(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    t: &str,
    event: i64,
    kind: &str,
    data: &Value,
) -> Result<()> {
    veyndo_intelligence::store::observations::project(
        tx,
        t,
        event,
        kind,
        data,
        crate::payments::external,
    )
    .await
}
