//! Public synthetic customer fixture for newly provisioned demo shops; never fills real customer addresses.
use crate::*;
pub(crate) async fn seed_address(conn: &mut sqlx::PgConnection, t: &str) -> Result<()> {
    let fixture: Value = serde_json::from_str(include_str!("../../fixtures/demo-customer.json"))
        .expect("static demo customer");
    veyndo_customers::store::customers::seed_demo_profile(conn, t, &fixture["profile"]).await?;
    let mut address: commerce::Address =
        serde_json::from_value(fixture["address"].clone()).expect("static demo address");
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
