//! The behaviour consent filter rewritten for per-channel retention policies, so reports join a `policies` relation instead of one channel's parameters.
use super::behavior::ELIGIBLE;

/// `ELIGIBLE` with the channel, retention days and policy version taken from a joined `policies` row (`channel`, `days`, `version`).
pub fn eligible_per_channel() -> String {
    ELIGIBLE
        .replace(
            "FROM session_signal_events e",
            "FROM session_signal_events e JOIN policies p ON p.channel=e.channel_id",
        )
        .replace("e.channel_id=$2", "e.channel_id=p.channel")
        .replace("days=>$3", "days=>p.days")
        .replace("c.policy_version=$4", "c.policy_version=p.version")
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_parameter_of_the_filter_is_rewritten() {
        let sql = super::eligible_per_channel();
        assert!(sql.contains("JOIN policies p ON p.channel=e.channel_id"));
        for gone in ["e.channel_id=$2", "days=>$3", "c.policy_version=$4"] {
            assert!(!sql.contains(gone), "{gone}");
        }
    }
}
