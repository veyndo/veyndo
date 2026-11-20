//! Typed customer-owned profile updates; price groups, email and merchant roles cannot be self-assigned.
use super::*;
use veyndo_customers::store::accounts;
pub(super) async fn get(State(a): State<App>, h: RequestContext) -> Result<Json<Value>> {
    let (t, email) = identity(&a, &h).await?;
    let mut v = json!(accounts::row(&a, &t, &email).await?);
    v["addresses"] = address_list(&a, &t, &email).await?;
    decorate(&a, &t, &email, &mut v).await?;
    Ok(Json(v))
}
pub(super) async fn save(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let (t, email) = identity(&a, &h).await?;
    let mut p: Contact = serde_json::from_value(v)
        .map_err(|_| bad("Only customer contact fields can be updated"))?;
    p.validate()?;
    if let Some(id) = &p.default_payment_method_id {
        let (s, _) = commerce::config(&a, &t).await?;
        let group = accounts::group(&a, &t, &email).await?;
        if !s
            .payments
            .iter()
            .any(|m| m.id == *id && m.active && (!m.business_only || s.is_business(&group)))
        {
            return Err(bad("Payment method unavailable for customer"));
        }
    }
    if let Some(ad) = &p.address {
        let (settings, _) = commerce::config(&a, &t).await?;
        if !settings.countries.contains(&ad.country) {
            return Err(bad("Address country unavailable"));
        }
    }
    accounts::save_profile(&a, &t, &email, &p).await?;
    Ok(Json(json!({"saved":true})))
}
pub(super) async fn password(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let (t, email) = identity(&a, &h).await?;
    let current = accounts::password_hash(&a, &t, &email).await?;
    let old = v["oldPassword"]
        .as_str()
        .ok_or(bad("Current password required"))?;
    if !auth::verify_password(old.to_string(), current.clone()).await? {
        return Err(Error(
            StatusCode::UNAUTHORIZED,
            "Invalid credentials".into(),
        ));
    }
    let new = auth::hash_password(auth::password(&json!({"password":v["newPassword"]}))?).await?;
    accounts::replace_password(&a, &t, &email, current, new).await?;
    Ok(Json(json!({"customerToken":session(&a,&t,&email).await?})))
}
