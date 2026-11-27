//! Typed read views of the JSON documents studio reads repeatedly: unit IDs, saved proposal evidence, grid criteria, quotes and model outputs.
use serde::Deserialize;
use serde_json::Value;
use veyndo_core::document::lenient;

/// A document's string ID (release units, products, tasks).
#[derive(Deserialize, Default)]
#[serde(default)]
pub struct Identified {
    #[serde(deserialize_with = "lenient")]
    pub id: Option<String>,
}

/// A document's ID as stored, whatever its JSON type.
#[derive(Deserialize, Default)]
#[serde(default)]
pub struct IdValue {
    pub id: Value,
}

/// Saved proposal evidence of a task.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Evidence {
    pub proposal: Value,
    pub translation_inference: Value,
    #[serde(deserialize_with = "lenient")]
    pub policy_revision: Option<i64>,
    /// The shop twin's stored result, absent on proposals saved before simulation existed.
    pub simulation: Value,
}

/// The parts of a proposal that decide how it is applied.
#[derive(Deserialize, Default)]
#[serde(default)]
pub struct ProposalParts {
    pub translation: Value,
    pub operations: Value,
}

/// The app extension filter of validated grid criteria.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Criteria {
    #[serde(deserialize_with = "lenient")]
    pub app_filter: Option<String>,
}

/// The payment mode of a cart quote.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Quote {
    #[serde(deserialize_with = "lenient")]
    pub payment_method: PaymentMethod,
}

/// A quoted payment method.
#[derive(Deserialize, Default)]
#[serde(default)]
pub struct PaymentMethod {
    #[serde(deserialize_with = "lenient")]
    pub mode: Option<String>,
}

/// Whether a channel is visible, with its ID.
#[derive(Deserialize, Default)]
#[serde(default)]
pub struct Visibility {
    pub id: Value,
    pub visible: Value,
}

/// The email key of a CRM directory entry.
#[derive(Deserialize, Default)]
#[serde(default)]
pub struct EmailKey {
    pub email: Value,
}

/// A queued agent run's task.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Queued {
    #[serde(deserialize_with = "lenient")]
    pub task_id: Option<String>,
}

/// Token usage a model call reports.
#[derive(Deserialize, Default)]
#[serde(default)]
pub struct Usage {
    pub prompt: Value,
    #[serde(deserialize_with = "lenient")]
    pub input_tokens: Option<u64>,
    #[serde(deserialize_with = "lenient")]
    pub output_tokens: Option<u64>,
}

/// The configured translation providers.
#[derive(Deserialize, Default)]
#[serde(default)]
pub struct Providers {
    #[serde(deserialize_with = "lenient")]
    pub providers: Option<Vec<Value>>,
}

/// One provider and whether it is configured.
#[derive(Deserialize, Default)]
#[serde(default)]
pub struct Provider {
    pub id: Value,
    pub configured: Value,
}

/// A category release unit.
#[derive(Deserialize, Default)]
#[serde(default)]
pub struct CategoryUnit {
    #[serde(deserialize_with = "lenient")]
    pub id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    pub parent_id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    pub position: Option<i64>,
    pub data: Value,
}

/// A media entry's URL.
#[derive(Deserialize, Default)]
#[serde(default)]
pub struct MediaUrl {
    #[serde(deserialize_with = "lenient")]
    pub url: Option<String>,
}

/// The editable customer fields a recorded version restores.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct CustomerVersion {
    pub profile: Value,
    pub company: Value,
    pub customer_group: Value,
    pub active: Value,
}

/// The currency scale a report computed.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct ReportScale {
    #[serde(deserialize_with = "lenient")]
    pub currency_scale: Option<i64>,
}
