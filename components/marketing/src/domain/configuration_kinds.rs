//! The kinds of marketing configuration: the right each needs and the table each lives in.
use veyndo_core::{Result, bad};

/// The permission to read or change one kind of configuration.
pub fn configuration_right(kind: &str, write: bool) -> Result<&'static str> {
    match (kind, write) {
        ("channels", false) => Ok("channels.read"),
        ("channels", true) => Ok("channels.manage"),
        ("rules" | "flows" | "promotions", false) => Ok("automation.read"),
        ("rules" | "flows" | "promotions", true) => Ok("automation.manage"),
        _ => Err(bad("Unknown configuration type")),
    }
}
/// The table of one kind of configuration.
pub fn table(kind: &str) -> Result<&'static str> {
    match kind {
        "rules" => Ok("commerce_rules"),
        "promotions" => Ok("commerce_promotions"),
        "flows" => Ok("commerce_flows"),
        "channels" => Ok("sales_channels"),
        _ => Err(bad("Unknown configuration type")),
    }
}
