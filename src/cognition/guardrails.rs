//! The guardrail rules live in the intelligence component; plan validation reads the current settings here.
use super::*;
pub(crate) use veyndo_intelligence::domain::price_guardrails::check;
pub(crate) async fn validate_plan(a: &App, t: &str, p: &Proposal, ps: &[Product]) -> Result<i64> {
    let (settings, revision) = commerce::config(a, t).await?;
    for c in &p.changes {
        check(
            &settings,
            ps.iter()
                .find(|p| p.id == c.product_id)
                .ok_or(bad("Unknown product"))?,
            c,
            false,
        )?;
    }
    Ok(revision)
}
