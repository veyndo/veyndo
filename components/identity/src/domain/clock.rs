//! The wall clock in whole seconds, as signed assertions and authenticator codes count it.
use std::time::{SystemTime, UNIX_EPOCH};

pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::now;

    #[test]
    fn the_clock_counts_seconds_since_the_epoch_without_going_back() {
        let first = now();
        assert!(first > 1_700_000_000);
        assert!(now() >= first);
    }
}
