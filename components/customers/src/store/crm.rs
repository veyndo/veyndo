//! Merchant CRM writes: creation on native contact and address storage, and revision-checked changes; credentials never leave storage.
use crate::service::address_book::{self, Save};
use crate::store::address_restore;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::Row;
use veyndo_core::document::{lenient, view};
use veyndo_core::shop_scope::header;
use veyndo_core::{RequestContext, Result, State, bad, conflict, hash, history_context, uid};
use veyndo_identity::domain::credentials::email as email_of;
use veyndo_pricing::customer::{Address, Contact};
use veyndo_shipping::domain::geography::validate_address;
use veyndo_shop::store::settings::scoped_locked;

/// Checks the rule conditions inside the settings that are read under the lock.
pub type Conditions<'a> = &'a (dyn Fn(&Value) -> Result<()> + Sync);

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Input {
    email: String,
    profile: Contact,
    customer_group: String,
    billing_address: Option<Address>,
    shipping_address: Option<Address>,
    #[serde(default)]
    additional_addresses: Vec<Address>,
    request_key: String,
}

/// Creates a contact without storefront credentials; the same request key returns the first result.
pub async fn create(
    a: &State,
    h: &RequestContext,
    t: &str,
    v: Value,
    stable_id: Option<&str>,
    source: Option<&Value>,
    conditions: Conditions<'_>,
) -> Result<Value> {
    if v["profile"].to_string().len() > 4000 {
        return Err(bad("Invalid customer profile"));
    }
    let mut input: Input = serde_json::from_value(v.clone())
        .map_err(|_| bad("Complete customer profile and billing address required"))?;
    if source.is_none() && input.billing_address.is_none() {
        return Err(bad(
            "Complete customer profile and billing address required",
        ));
    }
    if input.additional_addresses.len()
        + usize::from(input.billing_address.is_some())
        + usize::from(input.shipping_address.is_some())
        > 50
    {
        return Err(bad(
            "Customer address book supports at most 50 addresses per creation",
        ));
    }
    let email = email_of(&json!({"email":input.email}))?;
    input.profile.validate()?;
    if !(8..=128).contains(&input.request_key.len()) {
        return Err(bad("Customer request key required"));
    }
    let id = stable_id.map(str::to_owned).unwrap_or_else(|| {
        hash(&format!(
            "{t}:{}:{}",
            header(h, "x-rac-user").unwrap_or(""),
            input.request_key
        ))
    });
    let fingerprint = hash(&v.to_string());
    let mut tx = a.db.begin().await?;
    history_context::context(&mut tx, h, "merchant").await?;
    let (settings, _) = scoped_locked(&mut tx, t, "default", conditions).await?;
    if !settings
        .customer_groups
        .iter()
        .any(|g| g.id == input.customer_group)
    {
        return Err(bad("Unknown customer group"));
    }
    for address in input
        .billing_address
        .iter_mut()
        .chain(input.shipping_address.iter_mut())
        .chain(input.additional_addresses.iter_mut())
    {
        address.validate()?;
        validate_address(
            &address.country,
            &address.country_state_id,
            &settings.country_definitions,
        )?;
        if !settings.countries.contains(&address.country) {
            return Err(bad("Address country unavailable"));
        }
    }
    if let Some(row) =
        sqlx::query("SELECT email,automation FROM customers WHERE tenant=$1 AND id=$2")
            .bind(t)
            .bind(&id)
            .fetch_optional(&mut *tx)
            .await?
    {
        if row.get::<Value, _>("automation")["adminCreationFingerprint"] != fingerprint {
            return Err(conflict(
                "Customer request key already used for different input",
            ));
        }
        if source.is_some_and(|s| row.get::<Value, _>("automation")["importSource"] != *s) {
            return Err(conflict(
                "Customer source snapshot changed; review an explicit update",
            ));
        }
        return Ok(json!({"id":id,"email":row.get::<String,_>("email"),"created":true}));
    }
    // An invalid credential hash cannot be used to sign in. Storefront access is not granted by a CRM creation.
    let n = sqlx::query("INSERT INTO customers(tenant,id,email,password_hash,profile,company,group_name,automation) VALUES($1,$2,$3,'!',$4,NULLIF($5,''),$6,$7) ON CONFLICT DO NOTHING")
        .bind(t).bind(&id).bind(&email).bind(json!(input.profile)).bind(&input.profile.company).bind(&input.customer_group).bind(json!({"adminCreationFingerprint":fingerprint,"importSource":source})).execute(&mut *tx).await?.rows_affected();
    if n != 1 {
        return Err(conflict(
            "Customer already exists; select the existing customer",
        ));
    }
    for (address, flags) in address_plan(&json!(input))? {
        save_address(&mut tx, t, &email, source.is_some(), &flags, &address).await?;
    }
    veyndo_core::outbox::publish_on(
        &mut *tx,
        t,
        if source.is_some() {
            "migration.record.imported"
        } else {
            "customer.created"
        },
        &json!({"customerId":id,"actor":header(h,"x-rac-user"),"source":"merchant"}),
    )
    .await?;
    tx.commit().await?;
    Ok(json!({"id":id,"email":email,"created":true}))
}

// Imported address books share native validation/storage, without ordinary address-change workflows.
/// The JSON fields `save_address` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct SaveAddressInput {
    #[serde(deserialize_with = "lenient")]
    revision: Option<i64>,
    #[serde(deserialize_with = "lenient")]
    default_billing: Option<bool>,
    #[serde(deserialize_with = "lenient")]
    default_shipping: Option<bool>,
}

async fn save_address(
    conn: &mut sqlx::PgConnection,
    tenant: &str,
    email: &str,
    imported: bool,
    flags: &Value,
    address: &Address,
) -> Result<()> {
    let input: SaveAddressInput = view(flags);
    address_book::save(
        conn,
        tenant,
        email,
        Save {
            id: None,
            revision: input.revision,
            default_billing: input.default_billing.unwrap_or(false),
            default_shipping: input.default_shipping.unwrap_or(false),
            announce: !imported,
        },
        address,
    )
    .await?;
    Ok(())
}

/// The addresses of a customer record: billing, shipping and additional ones.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct AddressInput {
    billing_address: Value,
    shipping_address: Value,
    additional_addresses: Value,
}

/// One address creation plan preserves multiplicity and native first-row default assignment.
pub fn address_plan(v: &Value) -> Result<Vec<(Address, Value)>> {
    let input: AddressInput = view(v);
    let mut plan = vec![];
    let mut add = |value: &Value, flags: Value| -> Result<()> {
        if !value.is_null() {
            let mut address: Address =
                serde_json::from_value(value.clone()).map_err(|_| bad("Invalid source address"))?;
            address.validate()?;
            plan.push((address, flags));
        }
        Ok(())
    };
    add(
        &input.billing_address,
        json!({"defaultBilling":true,"defaultShipping":input.shipping_address.is_null()}),
    )?;
    add(&input.shipping_address, json!({"defaultShipping":true}))?;
    if let Some(xs) = input.additional_addresses.as_array() {
        for value in xs {
            add(value, json!({}))?;
        }
    }
    Ok(plan)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Edit {
    revision: i64,
    profile: Value,
    company: Option<String>,
    customer_group: String,
    active: bool,
}

/// Applies a merchant change at the expected revision; `book` restores the address aggregate of a historical state.
pub async fn save(
    a: &State,
    h: &RequestContext,
    t: &str,
    email: &str,
    v: Value,
    book: Option<Value>,
    conditions: Conditions<'_>,
) -> Result<Value> {
    let e: Edit = serde_json::from_value(v).map_err(|_| bad("Invalid customer update"))?;
    if e.company.as_ref().is_some_and(|s| s.len() > 200)
        || e.profile.to_string().len() > 4000
        || !e.profile.is_object()
    {
        return Err(bad("Invalid customer profile"));
    }
    let mut tx = a.db.begin().await?;
    history_context::context(&mut tx, h, "merchant").await?;
    let (settings, _) = scoped_locked(&mut tx, t, "default", conditions).await?;
    if !settings
        .customer_groups
        .iter()
        .any(|g| g.id == e.customer_group)
    {
        return Err(bad("Unknown customer group"));
    }
    let current: i64 = sqlx::query_scalar(
        "SELECT revision FROM customers WHERE tenant=$1 AND email=$2 FOR UPDATE",
    )
    .bind(t)
    .bind(email)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(conflict("Customer changed or unavailable"))?;
    if current != e.revision {
        return Err(conflict("Customer changed or unavailable"));
    }
    let mut contact: Contact = serde_json::from_value(e.profile.clone())
        .map_err(|_| bad("Invalid customer contact fields"))?;
    contact.validate()?;
    let n=sqlx::query("UPDATE customers SET profile=$1,company=$2,group_name=$3,active=$4,revision=revision+1 WHERE tenant=$5 AND email=$6 AND revision=$7").bind(json!(contact)).bind(e.company).bind(&e.customer_group).bind(e.active).bind(t).bind(email).bind(e.revision).execute(&mut *tx).await?.rows_affected();
    if n != 1 {
        return Err(conflict("Customer changed or unavailable"));
    }
    if let Some(book) = book {
        let automation = &book["automation"];
        if !automation.is_object() || automation.to_string().len() > 32768 {
            return Err(bad("Invalid customer automation data"));
        }
        sqlx::query("UPDATE customers SET automation=$1 WHERE tenant=$2 AND email=$3")
            .bind(automation)
            .bind(t)
            .bind(email)
            .execute(&mut *tx)
            .await?;
        address_restore::restore(&mut tx, t, email, &book, &|address| {
            validate_address(
                &address.country,
                &address.country_state_id,
                &settings.country_definitions,
            )?;
            if !settings.countries.contains(&address.country) {
                return Err(bad("Historical address country is unavailable"));
            }
            Ok(())
        })
        .await?;
    }
    let saved_revision: i64 =
        sqlx::query_scalar("SELECT revision FROM customers WHERE tenant=$1 AND email=$2")
            .bind(t)
            .bind(email)
            .fetch_one(&mut *tx)
            .await?;
    // Access and group changes revoke sessions and invalidate all open privileged cart contexts.
    sqlx::query("DELETE FROM customer_sessions WHERE tenant=$1 AND email=$2")
        .bind(t)
        .bind(email)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE carts SET token=md5(id||$1),data=jsonb_set(jsonb_set(jsonb_set(jsonb_set(data,'{email}','null'),'{customer_id}','null'),'{group}','\"consumer\"'),'{company}','null'),revision=revision+1 WHERE tenant=$2 AND data->>'email'=$3 AND status='open'").bind(uid()).bind(t).bind(email).execute(&mut *tx).await?;
    veyndo_core::outbox::publish_on(&mut *tx, t, "customer.updated", &json!({"actor":header(h,"x-rac-user"),"email":email,"revision":saved_revision,"active":e.active})).await?;
    tx.commit().await?;
    Ok(json!({"saved":true,"revision":saved_revision}))
}
