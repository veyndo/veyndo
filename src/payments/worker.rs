//! Leased payment jobs; the component owns the claim, the fence and the receipt, this file binds the server ports.
use crate::{App, Result};

pub(crate) async fn payment_once(a: &App) -> Result<bool> {
    veyndo_payments::service::worker::payment_once(
        &super::host::PaymentHost,
        &super::host::Gateway(a),
        a,
    )
    .await
}
