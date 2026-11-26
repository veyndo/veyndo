//! One explicit sandbox-only product creation delegates to native CAS/history/outbox; stable IDs reconcile retries without overwrites.
use super::{Draft, public_client};
use crate::ports::IntakeHost;
use crate::store::product_reads::extra;
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;
use veyndo_core::document::{lenient, view};
use veyndo_core::store::environments::parent;
use veyndo_core::{RequestContext, Result, bad, conflict, hash};
use veyndo_shop::Settings;

/// An approved catalogue intake draft with the language, currency and settings revision it was prepared for.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct DraftSave {
    #[serde(deserialize_with = "lenient")]
    approve: Option<bool>,
    #[serde(deserialize_with = "lenient")]
    main_locale: Option<String>,
    #[serde(deserialize_with = "lenient")]
    currency: Option<String>,
    #[serde(deserialize_with = "lenient")]
    id: Option<String>,
    draft: Value,
    #[serde(deserialize_with = "lenient")]
    tax_rate: Option<f64>,
    settings_revision: Value,
    demo: Value,
}

/// Creates one approved draft as a sandbox product; a replay of the same draft returns the saved product.
pub(super) async fn save(
    host: &dyn IntakeHost,
    h: &RequestContext,
    (t, s): (&str, &Settings),
    v: &Value,
) -> Result<Value> {
    let a = host.models().core;
    if parent(a, t).await?.is_none() {
        return Err(bad(
            "Catalogue intake saves require a private sandbox; live products were not changed",
        ));
    }
    let input: DraftSave = view(v);
    if input.approve != Some(true) {
        return Err(bad("Review and explicitly approve the product draft"));
    }
    if input.main_locale.as_deref() != Some(s.main_locale.as_str())
        || input.currency.as_deref() != Some(s.currencies.base_currency.as_str())
    {
        return Err(conflict(
            "Catalogue language or currency changed; prepare fresh drafts",
        ));
    }
    let id = input
        .id
        .as_deref()
        .filter(|s| Uuid::parse_str(s).is_ok())
        .ok_or(bad("Stable product UUID required"))?;
    let draft: Draft =
        serde_json::from_value(input.draft).map_err(|_| bad("Invalid product draft"))?;
    draft.validate()?;
    let rate = input
        .tax_rate
        .filter(|r| r.is_finite() && (0.0..=50.0).contains(r))
        .ok_or(bad("Review the product tax rate"))?;
    let digest = hash(&v.to_string());
    let existing = extra(&a.db, t, id).await?;
    if let Some(extra) = existing {
        if view::<IntakeExtra>(&extra).identity.catalog_intake_digest == digest {
            return Ok(json!({"id":id,"saved":true,"replayed":true}));
        }
        return Err(conflict(
            "Product ID already exists; no existing product was overwritten",
        ));
    }
    let locale = s.languages().key(&s.main_locale);
    let media = if draft.image_url.is_empty() {
        json!([])
    } else {
        public_client(
            host.egress(),
            &reqwest::Url::parse(&draft.image_url).map_err(|_| bad("Invalid image URL"))?,
        )
        .await?;
        json!([{"id":format!("intake-{id}"),"url":draft.image_url,"view":"","alt":{}}])
    };
    let payload = json!({"id":id,"revision":0,"settingsRevision":input.settings_revision,"translations":{locale:{"name":draft.name,"description":draft.description}},"extra":{"priceCurrency":s.currencies.base_currency,"demo":input.demo,"identity":{"catalogIntakeDigest":digest,"importSource":draft.source}},"catalog":{"active":true,"productNumber":if draft.product_number.is_empty(){format!("IMPORT-{id}")}else{draft.product_number},"categoryIds":["catalog-root"],"options":{}},"commerce":{"price":draft.price,"taxRate":rate,"stock":if input.demo==true{20}else{0},"minPurchase":1,"purchaseSteps":1,"maxPurchase":null,"deliveryDays":0,"listPrice":null,"advancedPrices":[],"media":media,"properties":{}}});
    host.create(h, payload).await
}

/// The intake identity a saved product carries in its `extra` document.
#[derive(Deserialize, Default)]
#[serde(default)]
struct IntakeExtra {
    #[serde(deserialize_with = "lenient")]
    identity: IntakeIdentity,
}

/// The digest of the intake request that created the product.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct IntakeIdentity {
    catalog_intake_digest: Value,
}
