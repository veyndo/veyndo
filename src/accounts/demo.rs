//! Public synthetic customer fixture for newly provisioned demo shops; never fills real customer addresses.
use crate::{accounts, commerce};
use serde::Deserialize;
use serde_json::Value;
use serde_json::json;
use veyndo_core::Result;
use veyndo_core::document::view;
/// The bundled demo customer: the profile and the address it is seeded with.
#[derive(Deserialize, Default)]
#[serde(default)]
struct DemoCustomer {
    profile: Value,
    address: Value,
}
pub(crate) async fn seed_address(conn: &mut sqlx::PgConnection, t: &str) -> Result<()> {
    let fixture: Value = serde_json::from_str(include_str!("../../fixtures/demo-customer.json"))
        .expect("static demo customer");
    let fixture: DemoCustomer = view(&fixture);
    veyndo_customers::store::customers::seed_demo_profile(conn, t, &fixture.profile).await?;
    let mut address: commerce::Address =
        serde_json::from_value(fixture.address).expect("static demo address");
    address.validate()?;
    accounts::address_save_conn(
        conn,
        t,
        "buyer@example.test",
        None,
        &json!({"defaultBilling":true,"defaultShipping":true}),
        &address,
    )
    .await?;
    Ok(())
}
