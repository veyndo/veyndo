//! One run that writes fixed prices of a currency into the catalogue, as the merchant API shows it.
use serde::Serialize;
use serde_json::Value;

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
