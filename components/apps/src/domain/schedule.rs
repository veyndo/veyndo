//! The bounded cron grammar of app schedules.
use crate::domain::model_contract;
use veyndo_core::Result;

/// Parses a schedule expression; too frequent or impossible ones are refused.
pub fn parse(expression: &str) -> Result<cron::Schedule> {
    veyndo_app_model::triggers::parse(expression).map_err(model_contract::error)
}

#[cfg(test)]
mod tests {
    use super::parse;
    #[test]
    fn cron_is_bounded_and_real() {
        for c in ["* * * * * *", "0 * * * *", "61 * * * * *", "0 99 * * * *"] {
            assert!(parse(c).is_err());
        }
        let s = parse("0 */15 * * * *").unwrap();
        let start = chrono::DateTime::parse_from_rfc3339("2026-10-05T10:01:00Z").unwrap();
        assert_eq!(
            s.after(&start).next().unwrap().to_rfc3339(),
            "2026-10-05T10:15:00+00:00"
        );
    }
}
