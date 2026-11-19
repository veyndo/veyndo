//! One versioned record of a source system as an adapter hands it over; its identity is stable per tenant.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use veyndo_core::{Result, bad};

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceRecord {
    pub version: u8,
    pub source: String,
    pub source_shop: String,
    pub kind: String,
    pub source_id: String,
    pub data: Value,
    pub warnings: Vec<String>,
}
impl SourceRecord {
    pub fn validate(&self) -> Result<()> {
        if self.version != 1
            || [&self.source, &self.source_shop, &self.source_id]
                .iter()
                .any(|s| s.is_empty() || s.len() > 200 || s.chars().any(char::is_control))
            || !self.data.is_object()
            || self.data.to_string().len() > 512 * 1024
            || self.warnings.len() > 20
        {
            return Err(bad("Invalid versioned import identity or record"));
        }
        Ok(())
    }
    pub fn identity(&self, t: &str) -> String {
        use sha2::Digest;
        let bytes = sha2::Sha256::digest(
            serde_json::to_vec(&(
                t,
                &self.source,
                &self.source_shop,
                &self.kind,
                &self.source_id,
            ))
            .unwrap(),
        );
        let mut id = [0; 16];
        id.copy_from_slice(&bytes[..16]);
        uuid::Uuid::from_bytes(id).to_string()
    }
}
