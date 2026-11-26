//! B2B feature switches live in shared commerce settings; disabling never relaxes existing financial policy.
use serde::{Deserialize, Serialize};
use veyndo_core::{Error, Result, StatusCode};

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Config {
    pub enabled: bool,
    pub quotes: bool,
    pub approvals: bool,
    pub lists: bool,
    pub quick_order: bool,
    pub edi: bool,
    pub catalogs: bool,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            quotes: true,
            approvals: true,
            lists: true,
            quick_order: true,
            edi: true,
            catalogs: true,
        }
    }
}
impl Config {
    pub fn admit(&self, operation: &str) -> Result<()> {
        if [
            "configuration",
            "settings.save",
            "companies",
            "company",
            "company.save",
            "company.create",
            "unit.save",
            "member.save",
        ]
        .contains(&operation)
        {
            return Ok(());
        }
        let feature = if operation.starts_with("quote") {
            self.quotes
        } else if operation.starts_with("approval") {
            self.approvals
        } else if operation.starts_with("list") {
            self.lists
        } else if operation == "quick-order" {
            self.quick_order
        } else if operation == "requisition.import" {
            self.edi
        } else if operation == "catalog.save" {
            self.catalogs
        } else {
            true
        };
        if !self.enabled || !feature {
            return Err(Error(
                StatusCode::FORBIDDEN,
                "This B2B feature is disabled in shop settings".into(),
            ));
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::Config;
    #[test]
    fn disabled_features_do_not_silently_admit_financial_operations() {
        let s = Config {
            enabled: false,
            ..Default::default()
        };
        assert!(s.admit("configuration").is_ok());
        assert!(s.admit("select").is_err());
        let s = Config {
            quotes: false,
            ..Default::default()
        };
        assert!(s.admit("quote.accept").is_err());
        assert!(s.admit("quick-order").is_ok());
    }
}
