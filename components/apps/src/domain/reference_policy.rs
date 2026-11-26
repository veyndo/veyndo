//! Which core records an app field may refer to; no app alters core tables.
use veyndo_kernel::verified_kernel;

/// Admits references to products and to records the app may read privately.
pub struct ReferencePolicy;
impl veyndo_app_model::declarations::ReferencePolicy for ReferencePolicy {
    fn admitted(&self, product: bool, private: bool) -> bool {
        verified_kernel::app_core_reference_admissible(product, private)
    }
}

#[cfg(test)]
mod tests {
    use super::ReferencePolicy;
    use veyndo_app_model::declarations::ReferencePolicy as _;
    use veyndo_kernel::verified_kernel;

    #[test]
    fn the_policy_is_the_verified_reference_rule() {
        for product in [false, true] {
            for private in [false, true] {
                assert_eq!(
                    ReferencePolicy.admitted(product, private),
                    verified_kernel::app_core_reference_admissible(product, private)
                );
            }
        }
    }
}
