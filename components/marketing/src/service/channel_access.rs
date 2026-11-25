//! Single admission boundary for Store API, UCP/MCP and hosted storefronts; previews cannot purchase.
use crate::service::channel_preview::{cookie, valid};
use crate::store::channels as channel_rows;
use veyndo_core::host_names::public_path;
use veyndo_core::shop_scope::{channel, header, tenant};
use veyndo_core::{Error, RequestContext, Result, State, StatusCode};
use veyndo_identity::permit;
use veyndo_kernel::verified_kernel;
fn shopper_path(path: &str) -> bool {
    path.starts_with("/store-api/")
        || path.starts_with("/ucp/")
        || path.starts_with("/products/")
        || path == "/mcp"
        || path == "/api/experience"
        || path == "/api/concierge"
}
/// Whether a request changes state beyond the bounded cart simulations a merchant preview may run.
pub fn preview_mutation(path: &str, method: &str) -> bool {
    if method == "GET" || method == "HEAD" {
        return false;
    }
    // Cart edits are simulations. Orders, customer/account writes, reviews, payments and AI are not.
    !matches!(
        path,
        "/store-api/product"
            | "/store-api/navigation"
            | "/store-api/checkout/cart"
            | "/store-api/checkout/cart/line-item"
            | "/store-api/checkout/context"
            | "/store-api/checkout/coupons"
    ) && !(path.starts_with("/experience-api/")
        && [
            "/commerce/product",
            "/commerce/checkout/cart",
            "/commerce/checkout/cart/line-item",
            "/commerce/checkout/context",
            "/commerce/checkout/coupons",
        ]
        .iter()
        .any(|end| path.ends_with(end)))
}
/// Admits a shopper request to its sales channel; returns whether it runs as a merchant preview.
pub async fn admit(
    a: &State,
    h: &RequestContext,
    path: &str,
    method: &str,
    host: Option<&str>,
) -> Result<bool> {
    if path == "/mcp" && permit(h, "settings.read").is_ok() {
        return Ok(false);
    }
    if path.starts_with("/channel-preview/") {
        return Ok(false);
    }
    let hosted = host.is_some() && public_path(path);
    if !shopper_path(path) && !hosted {
        return Ok(false);
    }
    let t = tenant(h)?;
    let id = channel(h);
    let snapshot = if let Some(access) = h
        .access
        .as_ref()
        .filter(|s| tenant(h).is_ok_and(|t| t == s.tenant) && s.channel_id == id)
    {
        access.channel.clone()
    } else {
        channel_rows::snapshot(a, &t, id).await?
    }
    .ok_or(Error(
        StatusCode::NOT_FOUND,
        "Sales channel unavailable".into(),
    ))?;
    let data = snapshot.data;
    if data["kind"] == "marketplace" {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Marketplace channels require authenticated connector access".into(),
        ));
    }
    if data["kind"] == "pos" {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "POS channels require the authenticated POS API".into(),
        ));
    }
    let active = data["active"] == true;
    let private = data["visibility"] == "private";
    let supplied = header(h, "x-channel-preview")
        .map(str::to_owned)
        .or_else(|| cookie(h));
    let preview = if let Some(token) = supplied {
        let valid = valid(a, (&t, id, snapshot.revision), &token, host).await?;
        if !valid {
            return Err(Error(
                StatusCode::FORBIDDEN,
                "Preview expired, revoked or belongs to another channel".into(),
            ));
        }
        true
    } else {
        false
    };
    let merchant = permit(h, "settings.read").is_ok();
    if !verified_kernel::channel_access_admissible(
        active,
        private,
        merchant,
        preview,
        preview_mutation(path, method),
    ) {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Sales channel is paused or private; open an authorized merchant preview".into(),
        ));
    }
    // Possessing a preview never grants a write path, even while the channel is public.
    if preview && preview_mutation(path, method) {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Merchant preview cannot place orders or change customer data".into(),
        ));
    }
    Ok(preview)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preview_only_admits_bounded_simulations() {
        assert!(!preview_mutation("/store-api/product", "POST"));
        assert!(!preview_mutation("/store-api/navigation", "POST"));
        assert!(!preview_mutation(
            "/experience-api/shops/own/commerce/checkout/cart",
            "PUT"
        ));
        for path in [
            "/store-api/checkout/order",
            "/store-api/account/register",
            "/store-api/account/profile",
            "/store-api/payment/finalize",
            "/mcp",
            "/ucp/v1/checkout-sessions",
            "/experience-api/shops/own/ask",
        ] {
            assert!(preview_mutation(path, "POST"), "{path}");
        }
    }
}
