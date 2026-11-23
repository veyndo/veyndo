//! Revision-bound native MCP App views: which tools and views a caller may reach, and the view document.
use crate::domain::action_admission::authorized;
use crate::domain::action_policy::readonly;
use crate::domain::{approval, native_views};
use serde_json::{Value, json};
use veyndo_app_model::manifest::Manifest;
use veyndo_app_model::surfaces::Surface;
pub use veyndo_app_model::surfaces::assistant_location as location;
use veyndo_app_model::{Action, Entity};
use veyndo_core::{Error, RequestContext, Result, StatusCode, bad};
use veyndo_identity::{merchant, permit};
pub use veyndo_intelligence::domain::tool_trace::{attachment_references, attachments};
use veyndo_kernel::verified_kernel;

/// The media type of an app view document.
pub const MIME: &str = "text/html;profile=mcp-app";
/// The MCP extension a client announces to receive app views.
pub const EXTENSION: &str = "io.modelcontextprotocol/ui";

/// How an MCP caller reaches an app tool: any exposed tool, or only read tools, optionally from one view.
pub struct Mode<'a> {
    read_only: bool,
    resource: Option<&'a str>,
}
impl<'a> Mode<'a> {
    /// A direct tool call; MCP opt-out still applies.
    pub fn tool() -> Self {
        Self {
            read_only: false,
            resource: None,
        }
    }
    /// A read-only call, bound to the view it comes from where one is named.
    pub fn read(resource: Option<&'a str>) -> Self {
        Self {
            read_only: true,
            resource,
        }
    }
}

/// Admits an app tool for MCP: exposed, authorized, read-only where asked and bound to its view where one is named.
pub fn admit_tool(
    h: &RequestContext,
    m: &Manifest,
    action: &Action,
    t: &str,
    mode: Mode<'_>,
) -> Result<()> {
    if h.principal.agent_key.is_some()
        || !verified_kernel::app_tool_admissible(action.mcp != Some(false), authorized(h, action))
        || mode.read_only && !readonly(action)
        || mode
            .resource
            .is_some_and(|uri| !matches(h, m, action, t, uri))
    {
        return Err(Error(
            StatusCode::NOT_FOUND,
            "App MCP tool not exposed".into(),
        ));
    }
    Ok(())
}

/// Whether a surface is an assistant view the caller may see: native, bound to one exposed read tool.
pub fn available(h: &RequestContext, m: &Manifest, s: &Surface) -> bool {
    let private = s.location == "admin.assistant";
    location(&s.location)
        && h.principal.agent_key.is_none()
        && (!private || merchant(h).is_ok())
        && s.permission.as_deref().is_none_or(|p| permit(h, p).is_ok())
        && native_views::is_native(m, s)
        && s.actions.len() == 1
        && m.actions.iter().any(|action| {
            action.name == s.actions[0]
                && action.handler == "list"
                && action.mcp == Some(true)
                && action.public != private
                && authorized(h, action)
        })
}

/// The address of a view; it names the shop, app, surface and the reviewed package digest.
pub fn uri(tenant: &str, m: &Manifest, s: &Surface, digest: &str) -> String {
    format!("ui://veyndo/{tenant}/{}/{}/{digest}", m.id, s.id)
}

/// Whether a tool call belongs to the view it claims to come from.
pub fn matches(
    h: &RequestContext,
    m: &Manifest,
    action: &Action,
    t: &str,
    requested: &str,
) -> bool {
    m.surfaces.iter().any(|s| {
        s.actions.first() == Some(&action.name)
            && available(h, m, s)
            && uri(t, m, s, &approval::canonical_digest(m)) == requested
    })
}

/// The view document and its definition for one available surface, in the caller's language chain.
pub fn resource(
    requested: &str,
    m: &Manifest,
    s: &Surface,
    locale: &str,
    main: &str,
) -> Result<Value> {
    let view = native_views::payload(m, s).ok_or(bad("App view unavailable"))?;
    let locale_chain = [
        locale,
        locale.split('-').next().unwrap_or(locale),
        main,
        main.split('-').next().unwrap_or(main),
    ];
    let definition = json!({"label":s.label,"view":view["view"],"entities":view["entities"],
        "localeChain":locale_chain,"toolName":format!("app.{}.{}",m.id,s.actions[0]),
        "filter": choice_filter(m, s, &view),
        "productReference": (s.location == "storefront.assistant").then(|| {
            view_entity(m, s, &view)?.fields.iter()
                .find(|field| field.core_reference.as_deref() == Some("product"))
                .map(|field| &field.name)
        }).flatten()});
    Ok(
        json!({"uri":requested,"mimeType":MIME,"text":document(&definition),
        "_meta":{"ui":{"csp":{"connectDomains":[],"resourceDomains":[]}},
                 "ai.veyndo/definition":definition}}),
    )
}

fn view_entity<'a>(m: &'a Manifest, s: &Surface, view: &Value) -> Option<&'a Entity> {
    let action = m.actions.iter().find(|a| a.name == s.actions[0])?;
    let entity = m
        .entities
        .iter()
        .find(|e| Some(&e.name) == action.entity.as_ref())?;
    view["entities"]
        .as_array()?
        .iter()
        .any(|e| e["name"] == entity.name)
        .then_some(entity)
}

fn choice_filter(m: &Manifest, s: &Surface, view: &Value) -> Option<Value> {
    let action = m.actions.iter().find(|a| a.name == s.actions[0])?;
    let schema = &action.input_schema["properties"]["filter"];
    if schema["type"] != "object" {
        return None;
    }
    let entity = view_entity(m, s, view)?;
    let field = entity.fields.iter().find(|f| {
        f.indexed
            && f.kind == "string"
            && !f.translatable
            && !f.choices.is_empty()
            && f.references.is_none()
            && f.core_reference.is_none()
            && (schema["additionalProperties"] != false
                || schema["properties"][&f.name]["type"] == "string")
    })?;
    Some(json!({"name":field.name,"label":field.label,"choices":field.choices}))
}

/// The static view document with its definition embedded as inert data.
pub fn document(definition: &Value) -> String {
    // JSON inside an inert script element. Do not let app data terminate the element.
    let data = definition
        .to_string()
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('&', "\\u0026");
    include_str!("mcp_view.html").replace("__APP_VIEW_DEFINITION__", &data)
}

#[cfg(test)]
mod tests {
    use super::{Mode, admit_tool, available, document, matches, resource, uri};
    use crate::domain::approval::canonical_digest;
    use serde_json::json;
    use veyndo_app_model::manifest::Manifest;
    use veyndo_core::RequestContext;

    fn manifest() -> Manifest {
        serde_json::from_str(include_str!(
            "../../../../extensions/apps/assistant-examples/mcp-app.json"
        ))
        .unwrap()
    }
    fn merchant() -> RequestContext {
        let mut h = RequestContext::new();
        h.insert("x-rac-role", "admin".parse().unwrap());
        h.insert("x-rac-tenant", "demo".parse().unwrap());
        h.insert("x-tenant", "demo".parse().unwrap());
        h
    }

    #[test]
    fn a_visitor_sees_the_public_view_and_only_a_merchant_the_private_one() {
        let m = manifest();
        let visitor = RequestContext::new();
        let (shopper, private) = (&m.surfaces[1], &m.surfaces[2]);
        assert!(available(&visitor, &m, shopper));
        assert!(!available(&visitor, &m, private));
        assert!(available(&merchant(), &m, private));
        assert!(!available(&merchant(), &m, &m.surfaces[0]));
        let mut agent = RequestContext::new();
        agent.principal.agent_key = Some("key".into());
        assert!(!available(&agent, &m, shopper));
    }

    #[test]
    fn a_view_address_binds_shop_app_surface_and_digest() {
        let m = manifest();
        let digest = canonical_digest(&m);
        let address = uri("demo", &m, &m.surfaces[1], &digest);
        assert_eq!(
            address,
            format!("ui://veyndo/demo/{}/shopper/{digest}", m.id)
        );
        let visitor = RequestContext::new();
        assert!(matches(&visitor, &m, &m.actions[0], "demo", &address));
        assert!(!matches(&visitor, &m, &m.actions[0], "other", &address));
        assert!(!matches(&visitor, &m, &m.actions[2], "demo", &address));
    }

    #[test]
    fn mcp_admits_exposed_read_tools_and_hides_the_rest() {
        let m = manifest();
        let visitor = RequestContext::new();
        let address = uri("demo", &m, &m.surfaces[1], &canonical_digest(&m));
        assert!(admit_tool(&visitor, &m, &m.actions[0], "demo", Mode::tool()).is_ok());
        assert!(
            admit_tool(
                &visitor,
                &m,
                &m.actions[0],
                "demo",
                Mode::read(Some(&address))
            )
            .is_ok()
        );
        assert!(
            admit_tool(
                &visitor,
                &m,
                &m.actions[0],
                "demo",
                Mode::read(Some("ui://veyndo/x"))
            )
            .is_err()
        );
        // The save action opted out of MCP, also for a merchant who may call it over HTTP.
        assert!(admit_tool(&merchant(), &m, &m.actions[1], "demo", Mode::tool()).is_err());
        assert!(admit_tool(&visitor, &m, &m.actions[2], "demo", Mode::tool()).is_err());
        let mut agent = RequestContext::new();
        agent.principal.agent_key = Some("key".into());
        assert!(admit_tool(&agent, &m, &m.actions[0], "demo", Mode::tool()).is_err());
    }

    #[test]
    fn a_view_document_carries_its_definition_as_inert_data() {
        let mut m = manifest();
        m.surfaces[1]
            .label
            .insert("en".into(), "</script><img src=x onerror=alert(1)>".into());
        let s = &m.surfaces[1];
        let address = uri("demo", &m, s, &canonical_digest(&m));
        let view = resource(&address, &m, s, "de-DE", "en-GB").unwrap();
        assert_eq!(view["uri"], address);
        assert_eq!(view["mimeType"], super::MIME);
        let definition = &view["_meta"]["ai.veyndo/definition"];
        assert_eq!(
            definition["localeChain"],
            json!(["de-DE", "de", "en-GB", "en"])
        );
        assert_eq!(definition["toolName"], format!("app.{}.list_entries", m.id));
        assert_eq!(definition["filter"]["name"], "segment");
        assert_eq!(definition["productReference"], "product_id");
        let text = view["text"].as_str().unwrap();
        assert!(!text.contains("</script><img"));
        assert!(text.contains("\\u003c/script\\u003e"));
        assert!(!document(&json!({})).contains("__APP_VIEW_DEFINITION__"));
        // The private view has no product reference for shoppers and still resolves for its entity.
        let private = resource("ui://x", &m, &m.surfaces[2], "en-GB", "en-GB").unwrap();
        assert!(private["_meta"]["ai.veyndo/definition"]["productReference"].is_null());
    }
}
