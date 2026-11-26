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

#[cfg(test)]
mod tests {
    use super::{configuration_right, table};

    #[test]
    fn every_kind_has_its_rights_and_its_table() {
        assert_eq!(
            configuration_right("channels", false).ok(),
            Some("channels.read")
        );
        assert_eq!(
            configuration_right("channels", true).ok(),
            Some("channels.manage")
        );
        for kind in ["rules", "flows", "promotions"] {
            assert_eq!(
                configuration_right(kind, false).ok(),
                Some("automation.read")
            );
            assert_eq!(
                configuration_right(kind, true).ok(),
                Some("automation.manage")
            );
        }
        assert!(configuration_right("orders", false).is_err());
        assert_eq!(table("rules").ok(), Some("commerce_rules"));
        assert_eq!(table("promotions").ok(), Some("commerce_promotions"));
        assert_eq!(table("flows").ok(), Some("commerce_flows"));
        assert_eq!(table("channels").ok(), Some("sales_channels"));
        assert!(table("orders; DROP").is_err());
    }
}
