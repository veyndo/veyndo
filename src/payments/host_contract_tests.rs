//! The server's payment host against the contract set of `veyndo-port-contracts`.
#[test]
fn the_payment_host_meets_the_contract() {
    // The host reads the operator's payment service map from the process configuration.
    crate::runtime_config::get();
    veyndo_port_contracts::payment_host::service(&super::host::PaymentHost, None);
}
