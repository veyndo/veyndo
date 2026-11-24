//! API-key app hosting reuses encrypted connector settings, durable jobs and live configuration fences.
use super::*;
mod settings;
#[cfg(test)]
pub use contentful::sources as content_sources;
use veyndo_provider_algolia::provider as algolia;
use veyndo_provider_contentful::provider as contentful;
use veyndo_provider_hubspot::provider as hubspot;
use veyndo_provider_klaviyo::provider as klaviyo;
use veyndo_provider_vercel::provider as vercel;
pub const APPS: [&str; 5] = ["vercel", "klaviyo", "hubspot", "contentful", "algolia"];
pub fn fields(a: &str) -> &'static [&'static str] {
    match a {
        "algolia" => &["applicationId", "index"],
        "contentful" => &["space", "environment", "locale", "region"],
        _ => &[],
    }
}
pub fn prepare(a: &str, s: &Value, key: &str, v: &Value) -> Result<Call> {
    match a {
        "vercel" => vercel::prepare(s, key, v),
        "klaviyo" => klaviyo::prepare(s, key, v),
        "hubspot" => hubspot::prepare(s, key, v),
        "contentful" => contentful::prepare(s, key, v),
        "algolia" => algolia::prepare(s, key, v),
        _ => Err(Error::Invalid("Unknown integration")),
    }
}
pub fn receipt(a: &str, code: u16, v: &Value) -> Result<Value> {
    match a {
        "vercel" => vercel::receipt(code, v),
        "klaviyo" => klaviyo::receipt(code, v),
        "hubspot" => hubspot::receipt(code, v),
        "contentful" => contentful::receipt(code, v),
        "algolia" => algolia::receipt(code, v),
        _ => Err(Error::Invalid("Unknown integration")),
    }
}
pub async fn action(store: &Store, t: &str, a: &str, name: &str, v: &Value) -> Result<Value> {
    if name == "status" {
        return settings::public(store, t, a).await;
    }
    if name == "configure" {
        return settings::configure(store, t, a, v).await;
    }
    checked(name == "run", "Unknown integration action")?;
    let state = store.get(t, a).await?;
    checked(
        state["settings"]["enabled"] == true,
        "Enable and configure this integration first",
    )?;
    let input = v.get("input").cloned().unwrap_or(json!({}));
    let _ = prepare(
        a,
        &state["settings"],
        &text(&state["credentials"], "apiKey"),
        &input,
    )?;
    store
        .enqueue(
            t,
            a,
            &text(v, "requestKey"),
            &json!({"input":input,"revision":state["revision"]}),
        )
        .await
}
pub async fn perform(store: &Store, j: &queue::Job) -> Result<Value> {
    let mut tx = store.tx(&j.tenant).await?;
    store.lock(&mut tx, &j.tenant, &j.app).await?;
    let state = store.get_tx(&mut tx, &j.tenant, &j.app).await?;
    checked(
        state["revision"] == j.payload["revision"] && state["settings"]["enabled"] == true,
        "Integration changed or disabled; create a new request",
    )?;
    let call = prepare(
        &j.app,
        &state["settings"],
        &text(&state["credentials"], "apiKey"),
        &j.payload["input"],
    )?;
    let (code, value) = network::integration_request(call)
        .await
        .map_err(|e| match e {
            Error::Invalid(_) if j.app != "contentful" => Error::Uncertain,
            _ => e,
        })?;
    let mut result = receipt(&j.app, code, &value).map_err(|e| {
        if j.app == "contentful" {
            e
        } else {
            Error::Uncertain
        }
    })?;
    if j.app == "contentful" {
        checked(
            value["skip"].as_u64() == Some(j.payload["input"]["offset"].as_u64().unwrap_or(0)),
            "Contentful returned a different page offset",
        )?;
    }
    if j.app == "algolia" {
        let expected = j.payload["input"]["records"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["objectID"].clone())
            .collect::<Vec<_>>();
        if value["objectIDs"] != json!(expected) {
            return Err(Error::Uncertain);
        }
    }
    tx.commit().await?;
    if j.app == "contentful" {
        let records = contentful::sources(&value, &state["settings"])?;
        store
            .put_sources(
                &j.tenant,
                &j.app,
                &records,
                &state["settings"],
                &state["revision"],
                None,
            )
            .await?;
        result["sourceCount"] = json!(records.len());
    }
    Ok(result)
}
