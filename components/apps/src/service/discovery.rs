//! Searches the installed app capabilities a caller may use; data reads run through the same admitted action gateway.
use crate::domain::discovery::{admitted, relevant, schema};
use crate::domain::input_schema::validate_native;
use crate::ports::Platform;
use crate::service::gateway;
use crate::store::packages::{self, package};
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_app_model::manifest::Manifest;
use veyndo_core::document::{lenient, view};
use veyndo_core::{RequestContext, Result, bad, conflict};
use veyndo_identity::{merchant, permit};

/// The handlers a discovery read may reach: managed lists and saved configurations.
const READS: [&str; 2] = ["list", "configurations"];

/// One page of ranked apps after the app named by `after`; a vanished cursor asks the caller to search again.
pub fn page(
    mut apps: Vec<(usize, String, Value)>,
    after: Option<&str>,
    limit: usize,
) -> Result<Value> {
    apps.sort_by(|x, y| y.0.cmp(&x.0).then(x.1.cmp(&y.1)));
    let offset = match after {
        Some(id) => apps
            .iter()
            .position(|x| x.1 == id)
            .map(|n| n + 1)
            .ok_or(conflict("App discovery changed; search again"))?,
        None => 0,
    };
    let has_more = offset + limit < apps.len();
    let page = apps
        .into_iter()
        .skip(offset)
        .take(limit)
        .collect::<Vec<_>>();
    // The listed app ID is the ID the entry was ranked under.
    let next = page.last().filter(|_| has_more).map(|x| x.1.clone());
    let page = page.into_iter().map(|x| x.2).collect::<Vec<_>>();
    Ok(json!({"nextAfter":next,"hasMore":has_more,"apps":page}))
}

/// An app discovery request: a managed read with its arguments, or a search page.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct DiscoveryRequest {
    #[serde(deserialize_with = "lenient")]
    app: Option<String>,
    #[serde(deserialize_with = "lenient")]
    action: Option<String>,
    arguments: Value,
    #[serde(deserialize_with = "lenient")]
    query: Option<String>,
    #[serde(deserialize_with = "lenient")]
    after: Option<String>,
    #[serde(deserialize_with = "lenient")]
    limit: Option<u64>,
}
/// Runs `merchant.apps.search` or `merchant.apps.read` with the rights of the caller.
pub async fn invoke(a: &Platform, h: &RequestContext, name: &str, v: &Value) -> Result<Value> {
    let t = merchant(h)?;
    permit(h, "apps.read")?;
    validate_native(
        &schema(name).ok_or(bad("Unknown app discovery operation"))?,
        v,
    )?;
    let input: DiscoveryRequest = view(v);
    if name == "merchant.apps.read" {
        let app = input.app.as_deref().ok_or(bad("App required"))?;
        let m = package(a, &t, app, true).await?;
        let action = m
            .actions
            .iter()
            .find(|act| {
                Some(&act.name) == input.action.as_ref()
                    && admitted(h, &m, act)
                    && READS.contains(&act.handler.as_str())
            })
            .ok_or(bad("App action is not an admitted managed read"))?;
        return Box::pin(gateway::invoke(
            a,
            h,
            &m.id,
            &action.name,
            &input.arguments,
            None,
        ))
        .await;
    }
    let rows = packages::active_manifest_values(a, &t).await?;
    let query = input.query.as_deref().unwrap_or("");
    let mut apps = vec![];
    for row in rows {
        let m: Manifest = serde_json::from_value(row).map_err(|_| bad("Invalid installed app"))?;
        let actions=m.actions.iter().filter(|act|admitted(h,&m,act) && READS.contains(&act.handler.as_str()))
            .map(|act|json!({"action":act.name,"description":act.description,"inputSchema":act.input_schema})).take(12).collect::<Vec<_>>();
        let proposals=m.actions.iter().filter(|act|admitted(h,&m,act) && act.handler=="save").map(|act|json!({"name":format!("app.{}.{}",m.id,act.name),"description":act.description,"inputSchema":act.input_schema,"reviewRequired":true})).take(12).collect::<Vec<_>>();
        if actions.is_empty() && proposals.is_empty() {
            continue;
        }
        let score = relevant(&m, query);
        if !query.trim().is_empty() && score == 0 {
            continue;
        }
        apps.push((
            score,
            m.id.clone(),
            json!({"app":m.id,"version":m.version,"name":m.name,"tools":actions,"proposals":proposals}),
        ));
    }
    page(
        apps,
        input.after.as_deref(),
        input.limit.unwrap_or(8) as usize,
    )
}

#[cfg(test)]
mod tests {
    use super::page;
    use serde_json::json;
    use veyndo_core::StatusCode;

    #[test]
    fn pages_rank_by_relevance_then_id_and_continue_after_a_known_app() {
        let apps = || {
            vec![
                (1, "notes".to_string(), json!({"app":"notes"})),
                (3, "reviews".to_string(), json!({"app":"reviews"})),
                (1, "advisor".to_string(), json!({"app":"advisor"})),
            ]
        };
        let first = page(apps(), None, 2).unwrap();
        assert_eq!(first["apps"], json!([{"app":"reviews"},{"app":"advisor"}]));
        assert_eq!(first["hasMore"], true);
        assert_eq!(first["nextAfter"], "advisor");
        let rest = page(apps(), Some("advisor"), 2).unwrap();
        assert_eq!(rest["apps"], json!([{"app":"notes"}]));
        assert_eq!(rest["hasMore"], false);
        assert!(rest["nextAfter"].is_null());
        assert_eq!(
            page(apps(), Some("removed"), 2).err().map(|e| e.0),
            Some(StatusCode::CONFLICT)
        );
    }
}
