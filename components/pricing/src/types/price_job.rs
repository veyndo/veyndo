//! One run that writes fixed prices of a currency into the catalogue, as the merchant API shows it.
use serde::Serialize;
use serde_json::Value;
use sqlx::{Row, postgres::PgRow};

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct PriceJob {
    pub id: String,
    pub state: String,
    pub processed: i64,
    pub data: Value,
    pub created: String,
}

impl sqlx::FromRow<'_, PgRow> for PriceJob {
    fn from_row(row: &PgRow) -> sqlx::Result<Self> {
        Ok(Self {
            id: row.try_get("id")?,
            state: row.try_get("state")?,
            processed: row.try_get("processed")?,
            data: row.try_get("data")?,
            created: row.try_get("created")?,
        })
    }
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

impl sqlx::FromRow<'_, PgRow> for QueuedJob {
    fn from_row(row: &PgRow) -> sqlx::Result<Self> {
        Ok(Self {
            tenant: row.try_get("tenant")?,
            id: row.try_get("id")?,
            data: row.try_get("data")?,
            cursor: row.try_get("cursor")?,
            ceiling: row.try_get("ceiling")?,
            snapshot: row.try_get("snapshot_started")?,
        })
    }
}
