//! Explicit product selection; the rule and the ownership check are owned by the catalog component.
use super::*;
pub(crate) use veyndo_catalog::domain::selection::{parse, validate};
pub(crate) async fn admit(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    t: &str,
    ids: Option<&[String]>,
) -> Result<()> {
    veyndo_catalog::service::translation_apply::admit_selection(tx, t, ids).await
}
