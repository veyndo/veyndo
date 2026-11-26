//! Typed admission failures survive provider adapters; HTTP status never depends on parsing message text.
#[derive(Clone, Debug)]
pub enum Admission {
    Exhausted,
    Denied(&'static str),
    Unavailable(&'static str),
    Invalid(String),
}
impl Admission {
    pub fn status(&self) -> u16 {
        match self {
            Self::Exhausted => 429,
            Self::Denied(_) => 403,
            Self::Unavailable(_) => 503,
            Self::Invalid(_) => 400,
        }
    }
    pub fn detail(&self) -> &str {
        match self {
            Self::Exhausted => {
                "Agent allocation budget exhausted; review its usage before issuing new authority"
            }
            Self::Denied(s) | Self::Unavailable(s) => s,
            Self::Invalid(s) => s,
        }
    }
}
impl From<&'static str> for Admission {
    fn from(s: &'static str) -> Self {
        Self::Unavailable(s)
    }
}
impl From<String> for Admission {
    fn from(s: String) -> Self {
        Self::Invalid(s)
    }
}
#[derive(Clone, Debug)]
pub enum InferenceError {
    Provider(String),
    Admission(Admission),
}
impl From<Admission> for InferenceError {
    fn from(e: Admission) -> Self {
        Self::Admission(e)
    }
}
impl From<String> for InferenceError {
    fn from(s: String) -> Self {
        Self::Provider(s)
    }
}
impl From<&str> for InferenceError {
    fn from(s: &str) -> Self {
        Self::Provider(s.into())
    }
}
impl From<InferenceError> for String {
    fn from(e: InferenceError) -> Self {
        e.to_string()
    }
}
impl std::fmt::Display for InferenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Provider(s) => f.write_str(s),
            Self::Admission(e) => f.write_str(e.detail()),
        }
    }
}
impl std::error::Error for InferenceError {}
// Provider transport and deterministic admission keep distinct problem codes in the shared error contract.
impl From<InferenceError> for veyndo_core::Error {
    fn from(error: InferenceError) -> Self {
        let status = match &error {
            InferenceError::Admission(e) => veyndo_core::StatusCode::from_u16(e.status()).unwrap(),
            _ => veyndo_core::StatusCode::BAD_GATEWAY,
        };
        let failure = veyndo_core::Error(status, error.to_string());
        if matches!(error, InferenceError::Admission(Admission::Exhausted)) {
            return failure.exhausted("/agentBudget");
        }
        failure
    }
}
#[cfg(test)]
mod failure_tests {
    use super::*;
    #[test]
    fn exhausted_lifetime_allocation_maps_to_a_final_quota_error() {
        let error = veyndo_core::Error::from(InferenceError::from(Admission::Exhausted));
        assert_eq!(error.problem()["kind"], "quota_exceeded");
        assert_eq!(error.problem()["retryable"], false);
        assert_eq!(error.problem()["pointer"], "/agentBudget");
        assert_eq!(error.0, veyndo_core::StatusCode::TOO_MANY_REQUESTS);
        let provider = veyndo_core::Error::from(InferenceError::Provider("down".into()));
        assert_eq!(provider.problem()["kind"], "provider_unavailable");
    }
}
