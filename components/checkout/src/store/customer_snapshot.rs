//! Checkout-owned immutable customer and address records; future account edits cannot rewrite an order.
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::view;
use veyndo_core::{Error, Result, StatusCode, uid};
use veyndo_customers::store::purchase_customers::share_snapshot;
use veyndo_shop::domain::defaults::decode;
use veyndo_shop::store::settings::basis;
use veyndo_shop::types::cart::StoredCart;
use veyndo_shop::types::settings::CheckoutSelection;

/// The customer profile fields the order snapshot copies.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct CustomerProfile {
    name: Value,
    first_name: Value,
    last_name: Value,
    salutation_id: Value,
    title: Value,
    vat_ids: Value,
}

pub async fn snapshot(
    conn: &mut sqlx::PgConnection,
    c: &StoredCart,
    selected: &CheckoutSelection,
) -> Result<Value> {
    let customer = if let Some(id) = &c.data.customer_id {
        let r = share_snapshot(&mut *conn, &c.tenant, id)
            .await?
            .ok_or(Error(
                StatusCode::UNAUTHORIZED,
                "Customer account unavailable".into(),
            ))?;
        let profile: CustomerProfile = view(&r.profile);
        json!({"customerId":id,"customerNumber":r.customer_number,"email":r.email,"name":profile.name,"firstName":profile.first_name,"lastName":profile.last_name,"salutationId":profile.salutation_id,"title":profile.title,"company":r.company,"vatIds":profile.vat_ids,"guest":false})
    } else {
        json!({"customerId":null,"customerNumber":null,"email":c.data.email,"name":selected.billing_address.as_ref().map(|a|a.name.clone()),"firstName":selected.billing_address.as_ref().map(|a|a.first_name.clone()),"lastName":selected.billing_address.as_ref().map(|a|a.last_name.clone()),"guest":true})
    };
    let settings = basis(&mut *conn, &c.tenant).await?;
    let settings = decode(settings)?;
    let billing_id = uid();
    let shipping_id = uid();
    let date: String = sqlx::query_scalar!(
        r#"SELECT to_char(now() AT TIME ZONE 'UTC','YYYY-MM-DD"T"HH24:MI:SS.US"Z"') AS "now!""#
    )
    .fetch_one(conn)
    .await?;
    Ok(
        json!({"orderCustomer":customer,"billingAddressId":if selected.billing_address.is_some(){Some(&billing_id)}else{None},"billingAddress":selected.billing_address,"shippingAddress":selected.address,"shippingAddressId":if selected.address.is_some(){Some(&shipping_id)}else{None},"orderDateTime":date,"salesChannelId":c.data.sales_channel,"currencyId":"EUR","currencyFactor":1,"taxStatus":if settings.is_business(&c.data.group){"net"}else{"gross"},"customerComment":null,"internalComment":null,"affiliateCode":null,"campaignCode":null}),
    )
}
