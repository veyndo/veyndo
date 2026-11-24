//! Stored surface grants: the objects a context may name and the recheck of a presented grant.
use crate::domain::approval;
use crate::domain::surface_grant::actor;
use crate::ports::Platform;
use crate::store::packages::package;
use serde::Deserialize;
use serde_json::Value;
use sqlx::Row;
use veyndo_app_model::manifest::Manifest;
use veyndo_app_model::surfaces::Surface;
use veyndo_core::document::{lenient, view};
use veyndo_core::shop_scope::tenant;
use veyndo_core::{Error, RequestContext, Result, StatusCode, bad, hash};
use veyndo_identity::{merchant, permit};
use veyndo_kernel::verified_kernel;

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
        let (sql, scope) = match key.as_str() {
            "productId" => (
                "SELECT EXISTS(SELECT 1 FROM products WHERE tenant=$1 AND id=$2)",
                "catalog.read",
            ),
            "customerId" => (
                "SELECT EXISTS(SELECT 1 FROM customers WHERE tenant=$1 AND id=$2)",
                "customers.read",
            ),
            "companyId" => (
                "SELECT EXISTS(SELECT 1 FROM b2b_companies WHERE tenant=$1 AND id=$2)",
                "b2b.read",
            ),
            "unitId" => (
                "SELECT EXISTS(SELECT 1 FROM b2b_units WHERE tenant=$1 AND id=$2)",
                "b2b.read",
            ),
            "orderId" => (
                "SELECT EXISTS(SELECT 1 FROM orders WHERE tenant=$1 AND id=$2)",
                "orders.read",
            ),
            "salesChannelId" | "salesChannel" => (
                "SELECT EXISTS(SELECT 1 FROM sales_channels WHERE tenant=$1 AND id=$2)",
                "catalog.read",
            ),
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
        if !sqlx::query_scalar::<_, bool>(sql)
            .bind(tenant(h)?)
            .bind(id)
            .fetch_one(&a.db)
            .await?
        {
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
    let row = sqlx::query("SELECT package_digest,actions,context FROM app_surface_grants WHERE tenant=$1 AND token_hash=$2 AND app=$3 AND surface=$4 AND actor_hash=$5 AND expires_at>now()")
        .bind(&t).bind(hash(token)).bind(id).bind(surface).bind(actor(h)).fetch_optional(&a.db).await?.ok_or(Error(StatusCode::UNAUTHORIZED,"Surface grant expired or belongs to another actor".into()))?;
    let m = package(a, &t, id, true).await?;
    if !verified_kernel::app_surface_admissible(
        row.get::<String, _>("package_digest") == approval::canonical_digest(&m),
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
    let ctx: Value = row.get("context");
    context(a, h, &ctx).await?;
    Ok((m, s, ctx, row.get("actions")))
}
