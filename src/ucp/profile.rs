//! Discover the versioned B2B extension and intersect opt-in platform profiles through bounded public HTTPS.
use super::document::{UcpCapability, UcpMeta, UcpService};
use super::views::ShopLinks;
use super::{UCP_VERSION, cart, consent};
use crate::apps;
use axum::Json;
use axum::http::StatusCode;
use serde::Deserialize;
use serde_json::Value;
use serde_json::json;
use std::collections::BTreeMap;
use std::collections::HashMap;
use veyndo_core::Error;
use veyndo_core::Result;
use veyndo_core::bad;
use veyndo_core::conflict;
use veyndo_core::document::{lenient, view};
use veyndo_core::request_context::RequestContext;
use veyndo_core::shop_scope::header;
use veyndo_core::shop_scope::tenant;
pub(super) const B2B: &str = "ai.veyndo.app.shopping.b2b_negotiation";
pub(super) const VERSION: &str = "2026-10-13";
pub(super) const CHECKOUT: &str = "dev.ucp.shopping.checkout";
const SCHEMA: &str = "https://app.veyndo.ai/ucp/2026-10-13/b2b-negotiation.json";
pub(crate) fn ucp_meta() -> UcpMeta {
    UcpMeta {
        version: UCP_VERSION,
        capabilities: BTreeMap::from([(CHECKOUT, UcpCapability::version(UCP_VERSION))]),
        services: None,
        payment_handlers: None,
    }
}
pub(crate) async fn schema() -> Json<Value> {
    Json(serde_json::from_str(include_str!("b2b-negotiation.json")).expect("Versioned UCP schema"))
}
/// The discovery profile's `ucp` block for a shop's endpoint.
fn discovery_meta(endpoint: &str) -> UcpMeta {
    let mut meta = ucp_meta();
    meta.services = Some(BTreeMap::from([(
        "dev.ucp.shopping",
        vec![UcpService {
            version: UCP_VERSION,
            spec: "https://ucp.dev/2026-08-25/specification/overview/",
            transport: "rest",
            schema: "https://ucp.dev/2026-08-25/services/shopping/rest.openapi.json",
            endpoint: endpoint.to_owned(),
        }],
    )]));
    meta.capabilities.insert(
        CHECKOUT,
        vec![UcpCapability {
            version: UCP_VERSION,
            spec: Some("https://ucp.dev/2026-08-25/specification/shopping/checkout/"),
            schema: Some("https://ucp.dev/2026-08-25/schemas/shopping/checkout.json"),
            extends: None,
        }],
    );
    meta.capabilities.insert(
        B2B,
        vec![UcpCapability {
            version: VERSION,
            extends: Some(vec![CHECKOUT]),
            schema: Some(SCHEMA),
            spec: Some("https://www.veyndo.ai/docs/b2b-ucp-negotiation.html"),
        }],
    );
    meta.capabilities.insert(
        cart::CAPABILITY,
        vec![UcpCapability {
            version: UCP_VERSION,
            spec: Some("https://ucp.dev/2026-08-25/specification/shopping/cart/"),
            schema: Some("https://ucp.dev/2026-08-25/schemas/shopping/cart.json"),
            extends: None,
        }],
    );
    meta.capabilities.insert(
        consent::CAPABILITY,
        vec![UcpCapability {
            version: UCP_VERSION,
            extends: Some(vec![CHECKOUT, cart::CAPABILITY]),
            schema: Some("https://ucp.dev/2026-08-25/schemas/shopping/buyer_consent.json"),
            spec: Some("https://ucp.dev/2026-08-25/specification/buyer-consent/"),
        }],
    );
    meta.payment_handlers = Some(BTreeMap::new());
    meta
}
pub(crate) async fn ucp_profile(h: RequestContext) -> Result<Json<Value>> {
    let mut url = reqwest::Url::parse(ShopLinks::of(&tenant(&h)?).studio())
        .map_err(|_| bad("Invalid Studio origin"))?;
    url.set_path("/ucp/v1");
    url.set_fragment(None);
    // Retain tenant query in discovery; callers also send x-tenant and channel headers.
    Ok(Json(json!({"ucp":discovery_meta(url.as_str())})))
}
/// An advertised profile grants protocol compatibility only, never customer or merchant authority.
#[derive(Clone, Copy)]
pub(super) struct Selection {
    pub b2b: bool,
    pub consent: bool,
}
pub(super) async fn select(h: &RequestContext, cart_only: bool) -> Result<Selection> {
    let inline = header(h, "x-veyndo-ucp-b2b-version");
    let agent = header(h, "ucp-agent");
    let consent = header(h, "x-veyndo-ucp-consent-version");
    if consent.is_some_and(|v| v != UCP_VERSION) {
        return Err(conflict(
            "capabilities_incompatible: unsupported buyer consent version",
        ));
    }
    if (inline.is_some() || consent.is_some()) && agent.is_some() {
        return Err(bad(
            "Choose UCP-Agent profile or pre-negotiated Veyndo extension version",
        ));
    }
    if let Some(v) = inline {
        return if v == VERSION {
            Ok(Selection {
                b2b: true,
                consent: consent.is_some(),
            })
        } else {
            Err(conflict(
                "capabilities_incompatible: unsupported Veyndo B2B version",
            ))
        };
    }
    let Some(agent) = agent else {
        return Ok(Selection {
            b2b: false,
            consent: consent.is_some(),
        });
    };
    let url = profile_url(agent)?;
    let client = apps::public_https_client(&url).await?;
    // No inherited Authorization, tenant, cart or customer headers. Redirects/proxies/private DNS are denied.
    let mut response = client.get(url).send().await.map_err(|_| {
        Error(
            StatusCode::BAD_GATEWAY,
            "UCP platform profile could not be fetched".into(),
        )
    })?;
    if !response.status().is_success() || response.content_length().is_some_and(|n| n > 32768) {
        return Err(bad(
            "UCP platform profile must be a successful bounded JSON response",
        ));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| bad("UCP platform profile read failed"))?
    {
        if bytes.len() + chunk.len() > 32768 {
            return Err(bad("UCP platform profile exceeds 32 KiB"));
        }
        bytes.extend_from_slice(&chunk);
    }
    let profile: Value =
        serde_json::from_slice(&bytes).map_err(|_| bad("UCP platform profile is not JSON"))?;
    intersect(&profile, cart_only)
}
fn profile_url(value: &str) -> Result<reqwest::Url> {
    let raw = value
        .strip_prefix("profile=\"")
        .and_then(|s| s.strip_suffix('"'))
        .filter(|s| s.len() <= 2048)
        .ok_or(bad("UCP-Agent requires profile=\"https://…\""))?;
    let url = reqwest::Url::parse(raw).map_err(|_| bad("Invalid UCP profile URL"))?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(bad("UCP profiles require credential-free public HTTPS"));
    }
    Ok(url)
}
/// The fields of a platform's UCP profile the intersection reads.
#[derive(Deserialize, Default)]
#[serde(default)]
struct PlatformProfile {
    #[serde(deserialize_with = "lenient")]
    ucp: PlatformUcp,
}
#[derive(Deserialize, Default)]
#[serde(default)]
struct PlatformUcp {
    version: Value,
    #[serde(deserialize_with = "lenient")]
    capabilities: HashMap<String, Value>,
}
/// One capability version a platform advertises.
#[derive(Deserialize, Default)]
#[serde(default)]
struct PlatformCapability {
    version: Value,
    schema: Value,
    extends: Value,
}
impl PlatformUcp {
    /// The advertised versions of one capability; anything but a list reads as none.
    fn list(&self, name: &str) -> Vec<PlatformCapability> {
        self.capabilities
            .get(name)
            .and_then(Value::as_array)
            .map(|entries| entries.iter().map(view).collect())
            .unwrap_or_default()
    }
    fn supports(&self, name: &str, version: &str) -> bool {
        self.list(name).iter().any(|e| e.version == version)
    }
}
fn intersect(p: &Value, cart_only: bool) -> Result<Selection> {
    let p: PlatformProfile = view(p);
    let ucp = &p.ucp;
    let base = if cart_only {
        cart::CAPABILITY
    } else {
        CHECKOUT
    };
    if ucp.version != UCP_VERSION || !ucp.supports(base, UCP_VERSION) {
        return Err(conflict(
            "capabilities_incompatible: requires the requested UCP 2026-08-25 base capability",
        ));
    }
    let selected = ucp.supports(B2B, VERSION);
    if selected
        && !ucp
            .list(B2B)
            .iter()
            .any(|e| e.version == VERSION && e.schema == SCHEMA && e.extends == json!([CHECKOUT]))
    {
        return Err(bad(
            "Veyndo B2B schema authority or parent capability mismatch",
        ));
    }
    Ok(Selection {
        b2b: selected && !cart_only,
        consent: ucp.supports(consent::CAPABILITY, UCP_VERSION),
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn discovery_profile_matches_the_indexed_shape() {
        let endpoint = "https://shop.example/ucp/v1?shop=s";
        let mut meta =
            json!({"version":UCP_VERSION,"capabilities":{CHECKOUT:[{"version":UCP_VERSION}]}});
        assert_eq!(serde_json::to_value(ucp_meta()).unwrap(), meta);
        meta["services"] = json!({"dev.ucp.shopping":[{"version":UCP_VERSION,"spec":"https://ucp.dev/2026-08-25/specification/overview/","transport":"rest","schema":"https://ucp.dev/2026-08-25/services/shopping/rest.openapi.json","endpoint":endpoint}]});
        meta["capabilities"][CHECKOUT][0]["spec"] =
            json!("https://ucp.dev/2026-08-25/specification/shopping/checkout/");
        meta["capabilities"][CHECKOUT][0]["schema"] =
            json!("https://ucp.dev/2026-08-25/schemas/shopping/checkout.json");
        meta["capabilities"][B2B] = json!([{"version":VERSION,"extends":[CHECKOUT],"schema":SCHEMA,"spec":"https://www.veyndo.ai/docs/b2b-ucp-negotiation.html"}]);
        meta["capabilities"][cart::CAPABILITY] = json!([{"version":UCP_VERSION,"spec":"https://ucp.dev/2026-08-25/specification/shopping/cart/","schema":"https://ucp.dev/2026-08-25/schemas/shopping/cart.json"}]);
        meta["capabilities"][consent::CAPABILITY] = json!([{"version":UCP_VERSION,"extends":[CHECKOUT,cart::CAPABILITY],"schema":"https://ucp.dev/2026-08-25/schemas/shopping/buyer_consent.json","spec":"https://ucp.dev/2026-08-25/specification/buyer-consent/"}]);
        meta["payment_handlers"] = json!({});
        assert_eq!(json!({"ucp":discovery_meta(endpoint)}), json!({"ucp":meta}));
    }
    #[test]
    fn malformed_platform_profiles_read_like_indexing() {
        for p in [
            json!(null),
            json!({"ucp":"x"}),
            json!({"ucp":{"version":UCP_VERSION,"capabilities":[]}}),
            json!({"ucp":{"version":UCP_VERSION,"capabilities":{CHECKOUT:{"version":UCP_VERSION}}}}),
            json!({"ucp":{"version":UCP_VERSION,"capabilities":{CHECKOUT:[7,{"version":UCP_VERSION}],B2B:[1,{"version":VERSION,"schema":SCHEMA,"extends":[CHECKOUT]}]}}}),
        ] {
            let typed: PlatformProfile = view(&p);
            for (name, version) in [(CHECKOUT, UCP_VERSION), (B2B, VERSION)] {
                let indexed = p["ucp"]["capabilities"][name]
                    .as_array()
                    .is_some_and(|entries| entries.iter().any(|e| e["version"] == version));
                assert_eq!(typed.ucp.supports(name, version), indexed);
            }
            assert_eq!(typed.ucp.version, p["ucp"]["version"]);
        }
    }
    #[test]
    fn intersection_requires_exact_parent_version_and_vendor_authority() {
        let mut p = json!({"ucp":{"version":UCP_VERSION,"capabilities":{CHECKOUT:[{"version":UCP_VERSION}],B2B:[{"version":VERSION,"schema":SCHEMA,"extends":[CHECKOUT]}]}}});
        assert!(intersect(&p, false).unwrap().b2b);
        p["ucp"]["capabilities"][B2B][0]["schema"] = json!("https://evil.example/schema.json");
        assert!(intersect(&p, false).is_err());
        p["ucp"]["capabilities"]
            .as_object_mut()
            .unwrap()
            .remove(B2B);
        assert!(!intersect(&p, false).unwrap().b2b);
        p["ucp"]["capabilities"]
            .as_object_mut()
            .unwrap()
            .remove(CHECKOUT);
        assert!(intersect(&p, false).is_err());
    }
    #[test]
    fn cart_only_platform_and_optional_consent_intersect_independently() {
        let mut p = json!({"ucp":{"version":UCP_VERSION,"capabilities":{cart::CAPABILITY:[{"version":UCP_VERSION}]}}});
        assert!(!intersect(&p, true).unwrap().consent);
        assert!(intersect(&p, false).is_err());
        p["ucp"]["capabilities"][consent::CAPABILITY] = json!([{"version":UCP_VERSION}]);
        assert!(intersect(&p, true).unwrap().consent);
    }
    #[test]
    fn profiles_cannot_smuggle_credentials_or_alternate_schemes() {
        for v in [
            "profile=\"http://example.test/x\"",
            "profile=\"https://user:secret@example.test\"",
            "profile=\"https://example.test/#secret\"",
            "profile=\"https://example.test/\"; role=merchant",
        ] {
            assert!(profile_url(v).is_err());
        }
        assert!(profile_url("profile=\"https://agent.example/profile.json\"").is_ok());
    }
}
