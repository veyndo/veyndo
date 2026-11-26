//! Which assistant views of the installed packages a caller may list and open.
use crate::domain::approval;
use crate::domain::mcp_views::{MIME, available, uri};
use crate::store::packages::{active_manifests, package};
use serde_json::{Value, json};
use veyndo_app_model::manifest::Manifest;
use veyndo_app_model::surfaces::Surface;
use veyndo_core::shop_scope::tenant;
use veyndo_core::{RequestContext, Result, State, StatusCode};

/// The view resources a caller may read.
pub async fn list(a: &State, h: &RequestContext) -> Result<Vec<Value>> {
    let mut resources = Vec::new();
    if h.principal.agent_key.is_some() {
        return Ok(resources);
    }
    let Ok(t) = tenant(h) else {
        return Ok(resources);
    };
    for m in active_manifests(a, h).await? {
        let mut surfaces = m.surfaces.iter().filter(|s| available(h, &m, s)).peekable();
        if surfaces.peek().is_some() {
            let digest = approval::canonical_digest(&m);
            resources.extend(surfaces.map(|s| {
                json!({"uri":uri(&t,&m,s,&digest),"name":format!("{}.{}",m.id,s.id),
                       "mimeType":MIME})
            }));
        }
    }
    Ok(resources)
}

/// The installed package and surface a view address names, if the caller may see it and the digest is current.
pub async fn find(
    a: &State,
    h: &RequestContext,
    requested: &str,
) -> Result<Option<(Manifest, Surface)>> {
    let Some(path) = requested.strip_prefix("ui://veyndo/") else {
        return Ok(None);
    };
    let mut parts = path.split('/');
    let (Some(shop), Some(app), Some(id), Some(digest), None) = (
        parts.next(),
        parts.next(),
        parts.next(),
        parts.next(),
        parts.next(),
    ) else {
        return Ok(None);
    };
    let Ok(t) = tenant(h) else { return Ok(None) };
    if shop != t || digest.len() != 64 || h.principal.agent_key.is_some() {
        return Ok(None);
    }
    let m = match package(a, &t, app, true).await {
        Ok(m) => m,
        Err(error) if matches!(error.0, StatusCode::NOT_FOUND | StatusCode::CONFLICT) => {
            return Ok(None);
        }
        Err(error) => return Err(error),
    };
    let Some(s) = m
        .surfaces
        .iter()
        .find(|s| s.id == id && available(h, &m, s))
        .cloned()
    else {
        return Ok(None);
    };
    if approval::canonical_digest(&m) != digest {
        return Ok(None);
    }
    Ok(Some((m, s)))
}

/// One app view document in the caller's language, if the caller may reach it.
pub async fn read(a: &State, h: &RequestContext, requested: &str) -> Result<Option<Value>> {
    let Some((m, s)) = find(a, h, requested).await? else {
        return Ok(None);
    };
    let (locale, _) = veyndo_shop::service::language::language_context(a, h).await?;
    let (settings, _) = veyndo_shop::service::read_cache::config(a, &tenant(h)?).await?;
    crate::domain::mcp_views::resource(requested, &m, &s, &locale, &settings.main_locale).map(Some)
}
