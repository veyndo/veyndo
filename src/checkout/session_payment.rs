//! The Checkout test payment classification is owned by the checkout component; the deployment says whether fixtures count.
pub(crate) use veyndo_checkout::domain::session_payment::{has_saved_funding, provider};
pub(crate) fn test_environment(environment: &str) -> bool {
    veyndo_checkout::domain::session_payment::test_environment(
        environment,
        crate::runtime_config::get().strict,
    )
}
