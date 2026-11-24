//! Legal storefront and merchant operations: document acknowledgement, consent, withdrawal and data-rights requests, shared by HTTP and MCP.
use crate::service::consent::{policy, write};
use crate::service::run_journal::fence_commit;
use axum::Json;
use axum::extract::{Path, State as Shared};
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_checkout::store::cart_load::load as load_cart;
use veyndo_core::document::{lenient, view};
use veyndo_core::{RequestContext, Result, State, bad, conflict};
use veyndo_identity::{merchant, permit};
use veyndo_marketing::service::conditions::validate_condition;
use veyndo_shop::store::consumer_requests::{self as store, Intake};
use veyndo_shop::store::settings::scoped_locked;
use veyndo_shop::types::legal::version;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// An explicit document and digital-delivery acknowledgement.
pub struct Acceptance {
    policy_version: String,
    terms: bool,
    digital_immediate: bool,
}
/// Records the acknowledgement for the cart under the current policy version.
pub async fn accept(
    Shared(a): Shared<State>,
    h: RequestContext,
    Json(v): Json<Acceptance>,
) -> Result<Json<Value>> {
    let c = load_cart(&a, &h).await?;
    let mut tx = a.db.begin().await?;
    let (s, _) = scoped_locked(
        &mut tx,
        &c.tenant,
        &c.data.sales_channel,
        &validate_condition,
    )
    .await?;
    if v.policy_version != version(&s.legal) {
        return Err(conflict("Legal documents changed; review again"));
    }
    let mut data = json!({"policyVersion":v.policy_version,"terms":v.terms,"digitalImmediate":v.digital_immediate,"locale":c.data.locale,"recordedAt":chrono::Utc::now().to_rfc3339()});
    if let Some(actor) = &h.admin_order {
        data["source"] = json!("merchantRecordedCustomerAcknowledgement");
        data["actor"] = json!(actor);
    }
    sqlx::query("INSERT INTO legal_acceptances(tenant,cart_id,data) VALUES($1,$2,$3) ON CONFLICT(tenant,cart_id) DO UPDATE SET data=EXCLUDED.data")
      .bind(&c.tenant).bind(&c.id).bind(&data).execute(&mut *tx).await?;
    fence_commit(&a, &mut tx).await?;
    tx.commit().await?;
    Ok(Json(data))
}

/// Records a withdrawal or data-rights request of the cart's customer.
pub async fn create(
    Shared(a): Shared<State>,
    h: RequestContext,
    Json(v): Json<Intake>,
) -> Result<Json<Value>> {
    let c = load_cart(&a, &h).await?;
    Ok(Json(store::create(&a, &c, v).await?))
}
/// The customer-held receipt of a request.
pub async fn receipt(
    Shared(a): Shared<State>,
    h: RequestContext,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    let c = load_cart(&a, &h).await?;
    Ok(Json(store::receipt(&a, &c, &id).await?))
}
/// The shop's requests for review.
pub async fn list(Shared(a): Shared<State>, h: RequestContext) -> Result<Json<Value>> {
    let t = merchant(&h)?;
    permit(&h, "customers.read")?;
    Ok(Json(store::list(&a, &t).await?))
}
/// Reviews a request at its revision.
pub async fn update(
    Shared(a): Shared<State>,
    h: RequestContext,
    Path(id): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let t = merchant(&h)?;
    permit(&h, "customers.write")?;
    Ok(Json(store::review(&a, &h, &t, &id, &v).await?))
}

/// The right a merchant legal capability needs.
pub fn permission(name: &str) -> Option<&'static str> {
    match name {
        "merchant.legal.requests" => Some("customers.read"),
        "merchant.legal.review" => Some("customers.write"),
        _ => None,
    }
}
/// The input schema of a legal capability.
pub fn schema(name: &str) -> Option<Value> {
    let properties = match name {
        "privacy.policy" | "merchant.legal.requests" => json!({}),
        "privacy.consent" => {
            json!({"policyVersion":{"type":"string"},"choices":{"type":"object","additionalProperties":{"type":"boolean"}}})
        }
        "legal.accept" => {
            json!({"policyVersion":{"type":"string"},"terms":{"type":"boolean"},"digitalImmediate":{"type":"boolean"}})
        }
        "legal.request" => {
            json!({"kind":{"type":"string"},"name":{"type":"string"},"email":{"type":"string"},"reference":{"type":"string"},"message":{"type":"string"},"requestKey":{"type":"string"}})
        }
        "merchant.legal.review" => {
            json!({"id":{"type":"string"},"state":{"type":"string"},"revision":{"type":"integer"},"note":{"type":"string"}})
        }
        _ => return None,
    };
    let required = properties
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    Some(
        json!({"type":"object","properties":properties,"required":required,"additionalProperties":false}),
    )
}
/// The JSON fields `invoke` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct InvokeInput {
    #[serde(deserialize_with = "lenient")]
    id: Option<String>,
}

/// Runs a legal capability through the same handlers as HTTP.
pub async fn invoke(a: &State, h: &RequestContext, name: &str, v: &Value) -> Result<Value> {
    let input: InvokeInput = view(v);
    match name {
        "privacy.policy" => Ok(policy(Shared(a.clone()), h.clone()).await?.0),
        "privacy.consent" => Ok(write(
            Shared(a.clone()),
            h.clone(),
            Json(serde_json::from_value(v.clone()).map_err(|_| bad("Invalid consent"))?),
        )
        .await?
        .0),
        "legal.accept" => Ok(accept(
            Shared(a.clone()),
            h.clone(),
            Json(serde_json::from_value(v.clone()).map_err(|_| bad("Invalid acceptance"))?),
        )
        .await?
        .0),
        "legal.request" => Ok(create(
            Shared(a.clone()),
            h.clone(),
            Json(serde_json::from_value(v.clone()).map_err(|_| bad("Invalid request"))?),
        )
        .await?
        .0),
        "merchant.legal.requests" => Ok(list(Shared(a.clone()), h.clone()).await?.0),
        "merchant.legal.review" => Ok(update(
            Shared(a.clone()),
            h.clone(),
            Path(
                input
                    .id
                    .as_deref()
                    .ok_or(bad("Request ID required"))?
                    .into(),
            ),
            Json(v.clone()),
        )
        .await?
        .0),
        _ => Err(bad("Unknown legal capability")),
    }
}
