//! Admission facts of one shop and sales channel, read once per request and shared by every later check.
use serde_json::Value;

#[derive(Clone, Debug)]
pub struct ChannelSnapshot {
    pub data: Value,
    pub revision: i64,
}
#[derive(Clone, Debug)]
pub struct FrontendMount {
    pub origin: String,
    pub experience_alias: String,
}
#[derive(Clone, Debug)]
pub struct AccessSnapshot {
    pub tenant: String,
    pub channel_id: String,
    pub exists: bool,
    pub parent: Option<String>,
    pub status: Option<String>,
    pub channel: Option<ChannelSnapshot>,
    pub mount: Option<FrontendMount>,
}
