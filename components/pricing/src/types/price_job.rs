//! One run that writes fixed prices of a currency into the catalogue, as the merchant API shows it.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use veyndo_core::document::lenient;

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct PriceJob {
    pub id: String,
    pub state: String,
    pub processed: i64,
    pub data: Value,
    pub created: String,
}

/// A queued job as the worker claims it.
pub struct QueuedJob {
    pub tenant: String,
    pub id: String,
    pub data: Value,
    pub cursor: String,
    pub ceiling: String,
    /// When the job was created; products created later are not touched.
    pub snapshot: String,
}

/// The fields of a job's `data` document the worker reads: who started it, the frozen currency settings, the target currency and whether existing fixed prices are replaced.
#[derive(Deserialize, Default)]
#[serde(default)]
pub struct JobData {
    #[serde(deserialize_with = "lenient")]
    pub actor: Option<String>,
    pub config: Value,
    #[serde(deserialize_with = "lenient")]
    pub currency: Option<String>,
    #[serde(deserialize_with = "lenient")]
    pub overwrite: Option<bool>,
}
