//! Stored surface grants: the objects a context may name and the recheck of a presented grant.
use crate::domain::approval;
use crate::domain::surface_grant::actor;
use crate::ports::Platform;
use crate::store::packages::package;
use serde::Deserialize;
use serde_json::Value;
use veyndo_app_model::manifest::Manifest;
use veyndo_app_model::surfaces::Surface;
use veyndo_core::document::{lenient, view};
use veyndo_core::shop_scope::tenant;
use veyndo_core::{Error, RequestContext, Result, StatusCode, bad, hash};
use veyndo_identity::{merchant, permit};
use veyndo_kernel::verified_kernel;

/// Whether a core object of the named reference kind belongs to the shop, read by its owner in one short transaction.
async fn owned(a: &Platform, kind: &str, t: &str, id: &str) -> Result<bool> {
    let mut tx = a.db.begin().await?;
    let owned = crate::store::core_references::owned(&mut tx, kind, t, id).await?;
    tx.commit().await?;
    Ok(owned)
}
/// Checks every object a surface context names: it exists in this shop and the caller may see it.
pub async fn context(a: &Platform, h: &RequestContext, v: &Value) -> Result<()> {
    let obj = v
        .as_object()
        .ok_or(bad("Surface context must be an object"))?;
    if obj.len() > 4 {
        return Err(bad("Surface context field limit"));
    }
    for (key, value) in obj {
        if key == "itemCount" {
            if value.as_u64().is_none_or(|n| n > 10000) {
                return Err(bad("Invalid cart count context"));
            }
            continue;
        }
        let (kind, scope) = match key.as_str() {
            "productId" => ("product", "catalog.read"),
            "customerId" => ("customer", "customers.read"),
            "companyId" => ("b2bCompany", "b2b.read"),
            "unitId" => ("b2bUnit", "b2b.read"),
            "orderId" => ("order", "orders.read"),
            "salesChannelId" | "salesChannel" => ("salesChannel", "catalog.read"),
            _ => return Err(bad("Unknown surface context field")),
        };
        let id = value
            .as_str()
            .filter(|s| !s.is_empty() && s.len() <= 100)
            .ok_or(bad("Invalid surface context reference"))?;
        if h.principal.role.is_none()
            && matches!(
                key.as_str(),
                "customerId" | "unitId" | "productId" | "salesChannelId" | "salesChannel"
            )
        {
            a.actions.visitor_object(h, key, id).await?;
        } else {
            permit(h, scope)?;
        }
        if !owned(a, kind, &tenant(h)?, id).await? {
            return Err(bad("Surface object does not belong to this shop"));
        }
    }
    Ok(())
}
/// The JSON fields `load` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct LoadInput {
    #[serde(deserialize_with = "lenient")]
    grant: Option<String>,
}

/// Recheck current actor, immutable digest, role and object ownership for actions and UI bundle reads.
pub async fn load(
    a: &Platform,
    h: &RequestContext,
    id: &str,
    surface: &str,
    v: &Value,
) -> Result<(Manifest, Surface, Value, Value)> {
    let input: LoadInput = view(v);
    let t = tenant(h)?;
    let token = input
        .grant
        .as_deref()
        .filter(|s| s.len() == 64)
        .ok_or(Error(
            StatusCode::UNAUTHORIZED,
            "Surface grant required".into(),
        ))?;
    let row = sqlx::query!(
        "SELECT package_digest,actions,context FROM app_surface_grants WHERE tenant=$1 AND token_hash=$2 AND app=$3 AND surface=$4 AND actor_hash=$5 AND expires_at>now()",
        t,
        hash(token),
        id,
        surface,
        actor(h)
    )
    .fetch_optional(&a.db)
    .await?
    .ok_or(Error(
        StatusCode::UNAUTHORIZED,
        "Surface grant expired or belongs to another actor".into(),
    ))?;
    let m = package(a, &t, id, true).await?;
    if !verified_kernel::app_surface_admissible(
        row.package_digest == approval::canonical_digest(&m),
        true,
    ) {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Surface package has changed".into(),
        ));
    }
    let s = m
        .surfaces
        .iter()
        .find(|s| s.id == surface)
        .cloned()
        .ok_or(bad("Surface removed"))?;
    if s.location.starts_with("admin.") {
        merchant(h)?;
    }
    if let Some(p) = &s.permission {
        permit(h, p)?;
    }
    let ctx: Value = row.context;
    context(a, h, &ctx).await?;
    Ok((m, s, ctx, row.actions))
}

/// Drops the expired surface grants of an app.
pub async fn expire(conn: &mut sqlx::PgConnection, t: &str, app: &str) -> Result<()> {
    sqlx::query!(
        "DELETE FROM app_surface_grants WHERE tenant=$1 AND app=$2 AND expires_at<=now()",
        t,
        app
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// How many live surface grants one actor holds for an app.
pub async fn actor_count(
    conn: &mut sqlx::PgConnection,
    t: &str,
    app: &str,
    actor: &str,
) -> Result<i64> {
    Ok(sqlx::query_scalar!(
        r#"SELECT count(*) AS "count!" FROM app_surface_grants WHERE tenant=$1 AND app=$2 AND actor_hash=$3"#,
        t,
        app,
        actor
    )
    .fetch_one(conn)
    .await?)
}

/// A surface grant to store for five minutes.
pub struct NewGrant<'a> {
    pub tenant: &'a str,
    pub token_hash: &'a str,
    pub app: &'a str,
    pub surface: &'a str,
    pub actor: &'a str,
    pub package_digest: &'a str,
    pub actions: &'a Value,
    pub context: &'a Value,
}

/// Stores a surface grant that expires after five minutes.
pub async fn grant(conn: &mut sqlx::PgConnection, g: NewGrant<'_>) -> Result<()> {
    sqlx::query!(
        "INSERT INTO app_surface_grants(tenant,token_hash,app,surface,actor_hash,package_digest,actions,context,expires_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,now()+interval '5 minutes')",
        g.tenant,
        g.token_hash,
        g.app,
        g.surface,
        g.actor,
        g.package_digest,
        g.actions,
        g.context
    )
    .execute(conn)
    .await?;
    Ok(())
}
