//! Explicit public legal/brand projection; bank account, domestic tax ID and unlinked uploads remain private.
use super::{company_model, master_data};
use crate::{
    App, Json, RequestContext, Result, State, Value, commerce, header, json, marketing, tenant,
};
pub(super) async fn get(State(a): State<App>, h: RequestContext) -> Result<Json<Value>> {
    let t = tenant(&h)?;
    let channel = marketing::channel_id(&h);
    let (settings, _) = commerce::config(&a, &t).await?;
    let locale = header(&h, "x-commerce-locale").unwrap_or(&settings.main_locale);
    marketing::channel(&a, &h, channel, locale).await?;
    let mut tx = a.db.begin().await?;
    let (data, default_brand) = master_data::public_source(&mut tx, &t, channel).await?;
    tx.commit().await?;
    let public = company_model::public(
        &data,
        &|value| commerce::translated_string(value, locale, &settings.main_locale).to_owned(),
        default_brand,
        &t,
        channel,
    );
    Ok(Json(
        json!({"data":public,"salesChannelId":channel,"mainLocale":settings.main_locale}),
    ))
}
