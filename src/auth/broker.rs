//! Optional trusted identity exchange: signatures bind route, audience, expiry and one-use nonce.
use super::{email, hash_password, provision_shop};
use crate::{
    App, Arc, Json, Result, State, Value, bad, conflict, json, knowledge, prototype_products, uid,
    validate_tenant,
};

pub(crate) use veyndo_identity::store::broker::assertion;

pub(crate) async fn exchange(State(a): State<App>, Json(v): Json<Value>) -> Result<Json<Value>> {
    let c = assertion(&a, "/api/identity/exchange", &v).await?;
    let issuer = c["iss"].as_str().unwrap();
    let subject = c["sub"].as_str().unwrap();
    let email = email(&c)?;
    let name = c["name"]
        .as_str()
        .filter(|n| !n.trim().is_empty() && n.len() <= 100)
        .ok_or(bad("Merchant name required"))?;
    let shop = c["workspaceId"].as_str().ok_or(bad("Shop ID required"))?;
    validate_tenant(shop)?;
    let shop_name = c["workspaceName"]
        .as_str()
        .filter(|n| !n.trim().is_empty() && n.len() <= 100)
        .ok_or(bad("Shop name required"))?;
    let random_password = hash_password(format!("{}{}", uid(), uid())).await?;
    let mut tx = a.db.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,42))")
        .bind(&email)
        .execute(&mut *tx)
        .await?;
    // Only the configured broker can assert verified ownership of this email; arbitrary clients cannot link accounts.
    let user: String=sqlx::query_scalar("INSERT INTO merchant_users(id,email,name,password_hash,password_initialized,email_verified_at) VALUES($1,$2,$3,$4,false,now()) ON CONFLICT(email) DO UPDATE SET email=EXCLUDED.email,email_verified_at=COALESCE(merchant_users.email_verified_at,now()) RETURNING id")
        .bind(uid()).bind(&email).bind(name).bind(random_password).fetch_one(&mut *tx).await?;
    let linked: Option<String> = sqlx::query_scalar(
        "SELECT user_id FROM merchant_identities WHERE issuer=$1 AND subject=$2",
    )
    .bind(issuer)
    .bind(subject)
    .fetch_optional(&mut *tx)
    .await?;
    if linked.as_ref().is_some_and(|u| u != &user) {
        return Err(conflict(
            "Identity email changed; explicit account recovery required",
        ));
    }
    sqlx::query("INSERT INTO merchant_identities(issuer,subject,user_id) VALUES($1,$2,$3) ON CONFLICT DO NOTHING").bind(issuer).bind(subject).bind(&user).execute(&mut *tx).await?;
    let first: Option<String> = sqlx::query_scalar(
        "SELECT tenant FROM membership_grants WHERE user_id=$1 AND active ORDER BY tenant LIMIT 1",
    )
    .bind(&user)
    .fetch_optional(&mut *tx)
    .await?;
    if !super::mfa::trusted_session_tx(&mut tx, &user, c["sessionToken"].as_str()).await?
        && let Some(first) = first
        && let Some(challenge) = super::mfa::challenge_tx(&mut tx, &user, &first).await?
    {
        tx.commit().await?;
        return Ok(Json(challenge));
    }
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM tenants WHERE id=$1)")
        .bind(shop)
        .fetch_one(&mut *tx)
        .await?;
    let mut sandbox = None;
    let mut seeded = false;
    if exists {
        let owned:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM membership_grants WHERE tenant=$1 AND user_id=$2 AND role='owner' AND active)").bind(shop).bind(&user).fetch_one(&mut *tx).await?;
        if !owned {
            return Err(conflict("Shop address already exists"));
        }
    } else {
        seeded = c["demoCatalog"] == true;
        sandbox = Some(
            provision_shop(
                &mut tx,
                &user,
                shop,
                shop_name,
                String::new(),
                c["demoCatalog"] == true,
                false,
            )
            .await?,
        );
    }
    tx.commit().await?;
    if let Some(sandbox) = sandbox {
        a.sandboxes.insert(shop.into(), Arc::new(sandbox));
    }
    if seeded {
        for p in prototype_products(&a, shop).await? {
            knowledge::sync_product_scoped(&a.db, shop, &json!(p)).await?;
        }
        knowledge::seed_relations(&a.db, shop).await?;
    }
    Ok(Json(
        super::sessions::issue_session_with_proof(&a, &user, shop, c["sessionToken"].as_str())
            .await?,
    ))
}
