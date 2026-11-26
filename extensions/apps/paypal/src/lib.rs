//! The bundled PayPal adapter: account configuration, the attributed Orders API client, receipt admission and signed webhooks; the payments component owns the ledger.
pub mod adapter;
pub mod compatibility;
pub mod receipts;
pub mod webhooks;
