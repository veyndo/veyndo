//! Customer-owned order reads and receipts share one tenant/identity predicate and a public projection.
use super::*;
use veyndo_orders::store::account_orders;

// An account ID snapshot owns modern orders; matching guest email never grants access.
async fn owned(
    a: &App,
    t: &str,
    email: &str,
    id: Option<&str>,
    after: Option<&str>,
) -> Result<Vec<Value>> {
    Ok(account_orders::owned(a, t, email, id, after)
        .await?
        .iter()
        .map(|(data, time)| project(data, time))
        .collect())
}
pub(crate) fn project(raw: &Value, created: &str) -> Value {
    let mut v = json!({});
    for key in [
        "id",
        "orderNumber",
        "revision",
        "state",
        "orderDateTime",
        "billingAddress",
        "shippingAddress",
        "billingAddressId",
        "shippingAddressId",
        "deliveries",
        "legal",
    ] {
        v[key] = raw[key].clone();
    }
    v["cart"] = json!({"id":raw["cart"]["id"], "price":raw["cart"]["price"], "shippingCosts":raw["cart"]["shippingCosts"], "lineItems":raw["cart"]["lineItems"]});
    v["payment"] = json!({"state":raw["payment"]["state"], "provider":raw["payment"]["provider"], "method":raw["payment"]["method"]});
    v["createdAt"] = json!(created);
    commerce::order_fields(&mut v);
    apps::native_lifecycle::project_labels(raw, &mut v);
    v
}
#[derive(Default, Deserialize)]
pub(super) struct Page {
    after: Option<String>,
}
pub(super) async fn list(
    State(a): State<App>,
    h: RequestContext,
    axum::extract::Query(page): axum::extract::Query<Page>,
) -> Result<Json<Value>> {
    let (t, email) = identity(&a, &h).await?;
    if let Some(after) = &page.after
        && (after.len() > 128 || owned(&a, &t, &email, Some(after), None).await?.is_empty())
    {
        return Err(Error(StatusCode::NOT_FOUND, "Order not found".into()));
    }
    let mut elements = owned(&a, &t, &email, None, page.after.as_deref()).await?;
    let has_more = elements.len() > 100;
    elements.truncate(100);
    let next = if has_more {
        elements.last().map(|v| v["id"].clone())
    } else {
        None
    };
    let (machine, _) = commerce::machine(&a.db, &t).await?;
    let labels = json!(
        machine
            .states
            .iter()
            .map(|s| (&s.id, &s.label))
            .collect::<HashMap<_, _>>()
    );
    for v in &mut elements {
        if !v["stateLabels"].is_object() {
            v["stateLabels"] = labels.clone();
        }
    }
    Ok(Json(json!({"elements":elements,"nextCursor":next})))
}
pub(super) async fn detail(
    State(a): State<App>,
    h: RequestContext,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    let (t, email) = identity(&a, &h).await?;
    let mut v = owned(&a, &t, &email, Some(&id), None)
        .await?
        .pop()
        .ok_or(Error(StatusCode::NOT_FOUND, "Order not found".into()))?;
    v["receipts"] = json!(account_orders::receipts(&a, &t, &id).await?);
    let (machine, _) = commerce::machine(&a.db, &t).await?;
    if !v["stateLabels"].is_object() {
        v["stateLabels"] = json!(
            machine
                .states
                .iter()
                .map(|s| (&s.id, &s.label))
                .collect::<HashMap<_, _>>()
        );
    }
    Ok(Json(v))
}
pub(super) async fn receipt(
    State(a): State<App>,
    h: RequestContext,
    Path((order, id)): Path<(String, String)>,
) -> Result<Response> {
    let (t, email) = identity(&a, &h).await?;
    if owned(&a, &t, &email, Some(&order), None).await?.is_empty() {
        return Err(Error(StatusCode::NOT_FOUND, "Order not found".into()));
    }
    let (snapshot, content) = account_orders::receipt(&a, &t, &order, &id).await?;
    Ok(crate::operations::document_response(&snapshot, content))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn projection_does_not_leak_operational_or_session_data() {
        let raw = json!({"id":"own", "automation":{"notes":["Internal"]}, "activity":[{"actor":"private"}], "cart":{"token":"secret", "checkout":{"customerToken":"secret"}, "price":{"totalPrice":12}}, "payment":{"state":"paid","provider":"manual","attemptId":"private"}});
        let v = project(&raw, "2026-10-06");
        assert_eq!(v["id"], "own");
        assert_eq!(v["amountTotal"], 12);
        assert!(v.get("automation").is_none());
        assert!(v.get("activity").is_none());
        assert!(v["cart"].get("token").is_none());
        assert!(v["cart"].get("checkout").is_none());
        assert!(v["payment"].get("attemptId").is_none());
    }
}
