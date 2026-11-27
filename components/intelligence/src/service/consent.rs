//! Cart- and channel-bound affirmative consent choices; stale or expired policy receipts never authorize processing, and withdrawal deletes the personalization and analytics data at once. Successive carts share one revocable receipt, never copied grants.
use crate::store::consent as store;
use crate::store::consent::ReceiptView;
use crate::store::preferences::forget_many;
use crate::types::events::ConsentChanged;
use axum::Json;
use axum::extract::State as Shared;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use veyndo_checkout::store::cart_load::load as load_cart;
use veyndo_core::document::view;
use veyndo_core::shop_scope::{channel as channel_id, tenant};
use veyndo_core::{Error, RequestContext, Result, State, StatusCode, bad, conflict, hash};
use veyndo_kernel::verified_kernel;
use veyndo_marketing::service::channels::channel as admit_channel;
use veyndo_marketing::service::conditions::validate_condition;
use veyndo_shop::service::language::language_context;
use veyndo_shop::service::read_cache::settings;
use veyndo_shop::store::settings::scoped_locked;
use veyndo_shop::types::cart::StoredCart;
use veyndo_shop::types::legal::{Config, PURPOSES, version};

/// The public legal configuration of the storefront channel with its policy version.
pub async fn policy(Shared(a): Shared<State>, h: RequestContext) -> Result<Json<Value>> {
    let t = tenant(&h)?;
    let channel = channel_id(&h);
    let (s, _) = settings(&a, &t, channel).await?;
    admit_channel(&a, &h, channel, &language_context(&a, &h).await?.0).await?;
    Ok(Json(
        json!({"data":public_config(&s.legal),"policyVersion":version(&s.legal),"mainLocale":s.main_locale,"locales":s.locales,"salesChannelId":channel}),
    ))
}
/// The cart's current consent receipt, or an undecided one.
pub async fn read(Shared(a): Shared<State>, h: RequestContext) -> Result<Json<Value>> {
    let c = load_cart(&a, &h).await?;
    let (s, _) = settings(&a, &c.tenant, &c.data.sales_channel).await?;
    let r = store::receipt(&a.db, &c.tenant, &c.id, &c.data.sales_channel).await?;
    Ok(Json(
        r.filter(|r| r.policy_version == version(&s.legal) && r.fresh)
            .map(|r| r.data)
            .unwrap_or(json!({"choices":{},"policyVersion":version(&s.legal),"decided":false})),
    ))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// A storefront consent decision.
pub struct Choice {
    policy_version: String,
    choices: HashMap<String, bool>,
}
/// Records the storefront consent decision of the cart.
pub async fn write(
    Shared(a): Shared<State>,
    h: RequestContext,
    Json(v): Json<Choice>,
) -> Result<Json<Value>> {
    let c = load_cart(&a, &h).await?;
    let mut tx = a.db.begin().await?;
    store::lock_cart(&mut *tx, &c.tenant, &c.id).await?;
    let receipt = write_locked(&mut tx, &c, &v.policy_version, v.choices, false).await?;
    tx.commit().await?;
    Ok(Json(receipt))
}
/// Caller holds the native cart lock. Delta mode preserves only fresh native choices, never stale protocol claims.
pub async fn write_locked(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    c: &StoredCart,
    policy: &str,
    mut incoming: HashMap<String, bool>,
    delta: bool,
) -> Result<Value> {
    let owner = receipt_owner(tx, c).await?;
    let count = store::changes_today(&mut **tx, &c.tenant, &owner).await?;
    let (s, _) = scoped_locked(tx, &c.tenant, &c.data.sales_channel, &validate_condition).await?;
    if version(&s.legal) != policy {
        return Err(conflict("Privacy policy changed; review your choices"));
    }
    if incoming.keys().any(|p| !PURPOSES.contains(&p.as_str())) {
        return Err(bad("Unknown consent purpose"));
    }
    if delta
        && let Some(saved) =
            store::current_locked(&mut **tx, &c.tenant, &owner, &c.data.sales_channel, policy)
                .await?
    {
        let saved = view::<ReceiptView>(&saved);
        for purpose in PURPOSES {
            incoming
                .entry((*purpose).into())
                .or_insert(saved.grants(purpose));
        }
    }
    let choices = PURPOSES
        .iter()
        .map(|p| {
            (
                (*p).to_string(),
                incoming.get(*p) == Some(&true) && s.legal.purposes.get(*p) == Some(&true),
            )
        })
        .collect::<HashMap<_, _>>();
    // Revocation stays available even after admission is exhausted; identical choices do not renew expiry.
    if let Some(saved) =
        store::current_locked(&mut **tx, &c.tenant, &owner, &c.data.sales_channel, policy).await?
        && view::<ReceiptView>(&saved).choices == json!(choices)
    {
        return Ok(saved);
    }
    if count >= 100 && choices.values().any(|v| *v) {
        return Err(Error(
            StatusCode::TOO_MANY_REQUESTS,
            "Consent change limit reached".into(),
        ));
    }
    let receipt = json!({"choices":choices,"policyVersion":policy,"decided":true,"behaviorAnalyticsEnabled":s.legal.behavior_analytics.enabled,"recordedAt":chrono::Utc::now().to_rfc3339(),"expiresAt":(chrono::Utc::now()+chrono::Duration::days(s.legal.consent_days.into())).to_rfc3339()});
    store::save(
        &mut **tx,
        &c.tenant,
        &owner,
        &c.data.sales_channel,
        policy,
        &receipt,
        s.legal.consent_days as i32,
    )
    .await?;
    store::log(
        &mut **tx,
        &c.tenant,
        &owner,
        &c.data.sales_channel,
        &receipt,
    )
    .await?;
    let carts = receipt_carts(tx, c, &owner).await?;
    let signals = carts.iter().map(|id| hash(id)).collect::<Vec<_>>();
    if choices.get("personalization") != Some(&true) {
        forget_many(tx, &c.tenant, &carts).await?;
        store::forget_exposure(&mut **tx, &c.tenant, &c.data.session).await?;
        store::forget_personalization(tx, &c.tenant, &signals).await?;
    }
    if choices.get("analytics") != Some(&true) {
        store::forget_analytics(&mut **tx, &c.tenant, &signals).await?;
    }
    let changed = ConsentChanged {
        cart_id: &c.id,
        sales_channel_id: &c.data.sales_channel,
        choices: &choices,
    };
    veyndo_core::events::emit(&mut **tx, &c.tenant, &changed).await?;
    Ok(receipt)
}
/// Requires a fresh receipt for the purpose under the current policy.
pub async fn require(a: &State, c: &StoredCart, purpose: &str) -> Result<()> {
    let (s, _) = settings(a, &c.tenant, &c.data.sales_channel).await?;
    let r = store::receipt(&a.db, &c.tenant, &c.id, &c.data.sales_channel).await?;
    let valid = r.is_some_and(|r| admissible(&s.legal, &r, purpose));
    if !valid {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Consent required for this purpose".into(),
        ));
    }
    Ok(())
}

/// Whether a receipt admits processing for the purpose under the current policy.
fn admissible(legal: &Config, r: &store::Receipt, purpose: &str) -> bool {
    verified_kernel::consent_admissible(
        legal.purposes.get(purpose) == Some(&true),
        r.policy_version == version(legal),
        r.fresh,
        view::<ReceiptView>(&r.data).grants(purpose),
    )
}

/// Public disclosure excludes internal legal-review notes.
fn public_config(c: &Config) -> Value {
    let mut v = json!(c);
    v.as_object_mut().unwrap().remove("operatorNotes");
    v
}
/// Shared receipt lock fences processing against concurrent consent withdrawal.
pub async fn require_locked(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    c: &StoredCart,
    purpose: &str,
) -> Result<()> {
    let (s, _) = scoped_locked(tx, &c.tenant, &c.data.sales_channel, &validate_condition).await?;
    let r = store::receipt(&mut **tx, &c.tenant, &c.id, &c.data.sales_channel).await?;
    if !r.is_some_and(|r| admissible(&s.legal, &r, purpose)) {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Consent required for this purpose".into(),
        ));
    }
    Ok(())
}

/// The cart whose receipt this cart continues.
pub async fn receipt_owner(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    c: &StoredCart,
) -> Result<String> {
    Ok(
        store::owner(&mut **tx, &c.tenant, &c.id, &c.data.sales_channel)
            .await?
            .unwrap_or_else(|| c.id.clone()),
    )
}

/// The carts that share one receipt.
pub async fn receipt_carts(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    c: &StoredCart,
    owner: &str,
) -> Result<Vec<String>> {
    store::sharing_carts(&mut **tx, &c.tenant, &c.data.sales_channel, owner).await
}

/// The caller has authenticated the original cart capability through load_cart.
/// Completed storefront carts in the same browser/shop/channel qualify. Company
/// buyers additionally need their current authenticated identity and purchasing grant.
pub async fn continue_consent(a: &State, previous: &StoredCart, next: &StoredCart) -> Result<()> {
    if previous.status != "completed"
        || previous.tenant != next.tenant
        || previous.data.sales_channel != next.data.sales_channel
        || previous.data.session.is_empty()
        || previous.data.session != next.data.session
        || previous.data.customer_id != next.data.customer_id
        || previous.data.pos_actor.is_some()
        || previous.data.admin_actor.is_some()
        || (previous.data.b2b.is_some() && previous.data.customer_id.is_none())
    {
        return Ok(());
    }
    let mut tx = a.db.begin().await?;
    let (settings, _) = scoped_locked(
        &mut tx,
        &next.tenant,
        &next.data.sales_channel,
        &validate_condition,
    )
    .await?;
    // A capability-checked successor points at the original receipt. It never copies
    // a grant: reads, revocation and processing all lock/read the same root row.
    let saved = store::receipt(
        &mut *tx,
        &previous.tenant,
        &previous.id,
        &previous.data.sales_channel,
    )
    .await?;
    if let Some(saved) = saved.filter(|r| r.policy_version == version(&settings.legal) && r.fresh) {
        let source = saved.cart_id;
        // Old binaries fail closed on an alias during a rolling update/rollback.
        if store::alias(
            &mut *tx,
            &next.tenant,
            &next.id,
            &next.data.sales_channel,
            &source,
        )
        .await?
        {
            store::log(
                &mut *tx,
                &next.tenant,
                &next.id,
                &next.data.sales_channel,
                &json!({"continuedFromCartId":previous.id,"receiptCartId":source}),
            )
            .await?;
        }
    }
    tx.commit().await?;
    Ok(())
}
