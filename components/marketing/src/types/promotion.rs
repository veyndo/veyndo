//! Coupons and automatic campaigns as stored records, with the provenance an imported promotion keeps.
use crate::domain::rules;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use veyndo_core::{Result, bad, conflict};
use veyndo_pricing::types::currency as currencies;
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Promotion {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_import: Option<PromotionImport>,
    pub name: HashMap<String, String>,
    pub active: bool,
    pub code: Option<String>,
    pub kind: String,
    pub amount: f64,
    pub rule: rules::Condition,
    pub exclusive: bool,
    pub priority: i32,
    pub max_uses: Option<i64>,
    pub start: Option<String>,
    pub end: Option<String>,
}
pub fn validate_promotion(p: &Promotion) -> Result<()> {
    if let Some(source) = &p.source_import {
        if currencies::scale(&source.currency).is_none()
            || source.version != 1
            || [&source.source, &source.source_shop, &source.source_id]
                .iter()
                .any(|s| s.is_empty() || s.len() > 200 || s.chars().any(char::is_control))
            || source.record_digest.len() != 64
            || !source.record_digest.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(bad("Invalid promotion source provenance"));
        }
        if !veyndo_pricing::verified_kernel::source_activation_admissible(
            true,
            source.activation_reviewed,
            p.active,
        ) {
            return Err(bad(
                "Review original coupon conditions and remaining usage before activation",
            ));
        }
    }
    p.rule.validate(0)?;
    if !["percentage", "absolute", "free_shipping"].contains(&p.kind.as_str())
        || !p.amount.is_finite()
        || p.amount < 0.
        || p.amount > 1_000_000.
        || (p.kind == "percentage" && p.amount > 100.)
        || p.code
            .as_ref()
            .is_some_and(|c| c.is_empty() || c.len() > 64 || !c.is_ascii())
        || p.max_uses
            .is_some_and(|n| n < 0 || (n == 0 && (p.source_import.is_none() || p.active)))
    {
        return Err(bad("Invalid promotion"));
    }
    Ok(())
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PromotionImport {
    pub version: u8,
    pub source: String,
    pub source_shop: String,
    pub source_id: String,
    pub currency: String,
    pub record_digest: String,
    pub activation_reviewed: bool,
}
pub fn preserve_import(previous: &Value, next: &Value) -> Result<()> {
    if let Some(old) = previous.get("sourceImport").filter(|v| !v.is_null()) {
        let mut old = old.clone();
        let mut new = next["sourceImport"].clone();
        old.as_object_mut()
            .ok_or(bad("Invalid promotion provenance"))?
            .remove("activationReviewed");
        if let Some(fields) = new.as_object_mut() {
            fields.remove("activationReviewed");
        }
        if old != new {
            return Err(conflict(
                "Imported promotion provenance cannot be removed or changed",
            ));
        }
    }
    Ok(())
}
