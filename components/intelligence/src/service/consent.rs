//! Cart- and channel-bound affirmative consent choices; stale or expired policy receipts never authorize processing, and withdrawal deletes the personalization and analytics data at once. Successive carts share one revocable receipt, never copied grants.
use crate::store::preferences::forget_many;
use axum::Json;
use axum::extract::State as Shared;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use std::collections::HashMap;
use veyndo_checkout::store::cart_load::load as load_cart;
pub use veyndo_checkout::store::legal_snapshot::RECEIPT_SQL;
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
    let r = sqlx::query(RECEIPT_SQL)
        .bind(&c.tenant)
        .bind(&c.id)
        .bind(&c.data.sales_channel)
        .fetch_optional(&a.db)
        .await?;
    Ok(Json(
        r.filter(|r| {
            r.get::<String, _>("policy_version") == version(&s.legal) && r.get::<bool, _>("fresh")
        })
        .map(|r| r.get::<Value, _>("data"))
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
    sqlx::query("SELECT revision FROM carts WHERE tenant=$1 AND id=$2 FOR UPDATE")
        .bind(&c.tenant)
        .bind(&c.id)
        .fetch_one(&mut *tx)
        .await?;
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
    let count:i64=sqlx::query_scalar("SELECT count(*) FROM privacy_consent_log WHERE tenant=$1 AND cart_id=$2 AND created_at>now()-interval '1 day'").bind(&c.tenant).bind(&owner).fetch_one(&mut **tx).await?;
    let (s, _) = scoped_locked(tx, &c.tenant, &c.data.sales_channel, &validate_condition).await?;
    if version(&s.legal) != policy {
        return Err(conflict("Privacy policy changed; review your choices"));
    }
    if incoming.keys().any(|p| !PURPOSES.contains(&p.as_str())) {
        return Err(bad("Unknown consent purpose"));
    }
    if delta
        && let Some(saved) = sqlx::query("SELECT data FROM privacy_consents WHERE tenant=$1 AND cart_id=$2 AND channel_id=$3 AND policy_version=$4 AND expires_at>now() FOR UPDATE")
            .bind(&c.tenant).bind(&owner).bind(&c.data.sales_channel).bind(policy).fetch_optional(&mut **tx).await? {
            let saved: Value = saved.get("data");
            for purpose in PURPOSES {
                incoming.entry((*purpose).into()).or_insert(saved["choices"][*purpose] == true);
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
    if let Some(r) = sqlx::query("SELECT data FROM privacy_consents WHERE tenant=$1 AND cart_id=$2 AND channel_id=$3 AND policy_version=$4 AND expires_at>now() FOR UPDATE")
      .bind(&c.tenant).bind(&owner).bind(&c.data.sales_channel).bind(policy).fetch_optional(&mut **tx).await? {
        let saved:Value=r.get("data");
        if saved["choices"]==json!(choices) { return Ok(saved); }
    }
    if count >= 100 && choices.values().any(|v| *v) {
        return Err(Error(
            StatusCode::TOO_MANY_REQUESTS,
            "Consent change limit reached".into(),
        ));
    }
    let receipt = json!({"choices":choices,"policyVersion":policy,"decided":true,"behaviorAnalyticsEnabled":s.legal.behavior_analytics.enabled,"recordedAt":chrono::Utc::now().to_rfc3339(),"expiresAt":(chrono::Utc::now()+chrono::Duration::days(s.legal.consent_days.into())).to_rfc3339()});
    sqlx::query("INSERT INTO privacy_consents(tenant,cart_id,channel_id,policy_version,data,expires_at) VALUES($1,$2,$3,$4,$5,now()+make_interval(days=>$6)) ON CONFLICT(tenant,cart_id,channel_id) DO UPDATE SET policy_version=EXCLUDED.policy_version,data=EXCLUDED.data,expires_at=EXCLUDED.expires_at")
      .bind(&c.tenant).bind(&owner).bind(&c.data.sales_channel).bind(policy).bind(&receipt).bind(s.legal.consent_days as i32).execute(&mut **tx).await?;
    sqlx::query(
        "INSERT INTO privacy_consent_log(tenant,cart_id,channel_id,data) VALUES($1,$2,$3,$4)",
    )
    .bind(&c.tenant)
    .bind(&owner)
    .bind(&c.data.sales_channel)
    .bind(&receipt)
    .execute(&mut **tx)
    .await?;
    let carts = receipt_carts(tx, c, &owner).await?;
    let signals = carts.iter().map(|id| hash(id)).collect::<Vec<_>>();
    if choices.get("personalization") != Some(&true) {
        forget_many(tx, &c.tenant, &carts).await?;
        sqlx::query("DELETE FROM exposures WHERE tenant=$1 AND session=$2")
            .bind(&c.tenant)
            .bind(&c.data.session)
            .execute(&mut **tx)
            .await?;
        sqlx::query("DELETE FROM session_signals WHERE tenant=$1 AND session=ANY($2)")
            .bind(&c.tenant)
            .bind(&signals)
            .execute(&mut **tx)
            .await?;
        sqlx::query("DELETE FROM session_signal_events WHERE tenant=$1 AND session=ANY($2) AND purpose='personalization'")
            .bind(&c.tenant)
            .bind(&signals)
            .execute(&mut **tx)
            .await?;
    }
    if choices.get("analytics") != Some(&true) {
        sqlx::query("DELETE FROM session_signal_events WHERE tenant=$1 AND session=ANY($2) AND purpose='analytics'").bind(&c.tenant).bind(&signals).execute(&mut **tx).await?;
    }
    veyndo_core::outbox::publish_on(
        &mut **tx,
        &c.tenant,
        "privacy.consent_changed",
        &json!({"cartId":c.id,"salesChannelId":c.data.sales_channel,"choices":choices}),
    )
    .await?;
    Ok(receipt)
}
/// Requires a fresh receipt for the purpose under the current policy.
pub async fn require(a: &State, c: &StoredCart, purpose: &str) -> Result<()> {
    let (s, _) = settings(a, &c.tenant, &c.data.sales_channel).await?;
    let r = sqlx::query(RECEIPT_SQL)
        .bind(&c.tenant)
        .bind(&c.id)
        .bind(&c.data.sales_channel)
        .fetch_optional(&a.db)
        .await?;
    let valid = r.is_some_and(|r| {
        verified_kernel::consent_admissible(
            s.legal.purposes.get(purpose) == Some(&true),
            r.get::<String, _>("policy_version") == version(&s.legal),
            r.get::<bool, _>("fresh"),
            r.get::<Value, _>("data")["choices"][purpose] == true,
        )
    });
    if !valid {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Consent required for this purpose".into(),
        ));
    }
    Ok(())
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
    let r = sqlx::query(RECEIPT_SQL)
        .bind(&c.tenant)
        .bind(&c.id)
        .bind(&c.data.sales_channel)
        .fetch_optional(&mut **tx)
        .await?;
    if !r.is_some_and(|r| {
        verified_kernel::consent_admissible(
            s.legal.purposes.get(purpose) == Some(&true),
            r.get::<String, _>("policy_version") == version(&s.legal),
            r.get::<bool, _>("fresh"),
            r.get::<Value, _>("data")["choices"][purpose] == true,
        )
    }) {
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
    Ok(sqlx::query_scalar("SELECT coalesce(source_cart_id,cart_id) FROM privacy_consents WHERE tenant=$1 AND cart_id=$2 AND channel_id=$3")
        .bind(&c.tenant).bind(&c.id).bind(&c.data.sales_channel)
        .fetch_optional(&mut **tx).await?.unwrap_or_else(|| c.id.clone()))
}

/// The carts that share one receipt.
pub async fn receipt_carts(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    c: &StoredCart,
    owner: &str,
) -> Result<Vec<String>> {
    Ok(sqlx::query_scalar("SELECT cart_id FROM privacy_consents WHERE tenant=$1 AND channel_id=$2 AND (cart_id=$3 OR source_cart_id=$3) ORDER BY cart_id")
        .bind(&c.tenant).bind(&c.data.sales_channel).bind(owner).fetch_all(&mut **tx).await?)
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
    let saved = sqlx::query(RECEIPT_SQL)
        .bind(&previous.tenant)
        .bind(&previous.id)
        .bind(&previous.data.sales_channel)
        .fetch_optional(&mut *tx)
        .await?;
    if let Some(saved) = saved.filter(|r| {
        r.get::<String, _>("policy_version") == version(&settings.legal)
            && r.get::<bool, _>("fresh")
    }) {
        let source: String = saved.get("cart_id");
        // Old binaries fail closed on an alias during a rolling update/rollback.
        let inserted = sqlx::query("INSERT INTO privacy_consents(tenant,cart_id,channel_id,policy_version,data,expires_at,source_cart_id) VALUES($1,$2,$3,'','{\"choices\":{},\"decided\":false}',now(),$4) ON CONFLICT(tenant,cart_id,channel_id) DO NOTHING")
            .bind(&next.tenant).bind(&next.id).bind(&next.data.sales_channel)
            .bind(&source).execute(&mut *tx).await?.rows_affected();
        if inserted == 1 {
            sqlx::query("INSERT INTO privacy_consent_log(tenant,cart_id,channel_id,data) VALUES($1,$2,$3,$4)")
                .bind(&next.tenant).bind(&next.id).bind(&next.data.sales_channel)
                .bind(json!({"continuedFromCartId":previous.id,"receiptCartId":source}))
                .execute(&mut *tx).await?;
        }
    }
    tx.commit().await?;
    Ok(())
}
