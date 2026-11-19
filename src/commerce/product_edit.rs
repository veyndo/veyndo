//! Revision-bound multilingual product metadata: specifications, SEO, cross-selling and free shipping.
use super::*;
use veyndo_catalog::service::{product_fields, product_identity};
use veyndo_catalog::store::product_edit as store;
use veyndo_catalog::types::product_edit::Edit;
pub(crate) async fn edit_product(
    State(a): State<App>,
    h: RequestContext,
    Path(id): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let v = super::product_input::expand_category_edit(&a, &h, &id, v).await?;
    save_product(a, h, id, v, false).await
}
pub(super) async fn save_product(
    a: App,
    h: RequestContext,
    id: String,
    v: Value,
    create: bool,
) -> Result<Json<Value>> {
    auth::permit(&h, "catalog")?;
    let t = merchant(&a, &h)?;
    let mut edit: Edit = serde_json::from_value(v).map_err(|_| bad("Invalid product metadata"))?;
    let mut tx = a.db.begin().await?;
    history::context(&mut tx, &h, "merchant").await?;
    let settings_row =
        sqlx::query("SELECT data, revision FROM commerce_settings WHERE tenant=$1 FOR SHARE")
            .bind(&t)
            .fetch_one(&mut *tx)
            .await?;
    if edit
        .settings_revision
        .is_some_and(|revision| revision != settings_row.get::<i64, _>("revision"))
    {
        return Err(conflict("Catalogue settings changed; prepare fresh drafts"));
    }
    let settings = decode_config(settings_row.get("data"))?;
    if create && edit.extra.price_currency.is_none() {
        edit.extra.price_currency = Some(settings.currencies.base_currency.clone());
    }
    if edit
        .extra
        .price_currency
        .as_ref()
        .is_some_and(|c| !settings.currencies.definitions.iter().any(|d| &d.code == c))
    {
        return Err(bad("Product price currency is not configured"));
    }
    currencies::validate_prices(&edit.extra.currency_prices, &settings.currencies)?;
    let languages = settings.languages();
    edit.validate_translations(languages)?;
    crate::legal::validate_product(&edit.extra.compliance, &settings)?;
    let (main_name, main_description) = edit.main_text(languages)?;
    edit.validate_metadata(languages, |id| {
        settings.taxes.iter().any(|tax| tax.id == id)
    })?;
    marketing::validate_metadata("products", &edit.extra.automation)?;
    edit.validate_fields(languages)?;
    if let Some(c) = &edit.catalog {
        if edit.extra.automation.is_null() {
            edit.extra.automation = json!({});
        }
        edit.extra.automation["categoryIds"] = json!(c.category_ids);
    }
    store::lock_numbers(&mut tx, &t).await?;
    if create {
        let fields = edit
            .catalog
            .as_ref()
            .ok_or(bad("New product catalog fields required"))?;
        if edit.revision != 0 {
            return Err(bad("New product revision must be zero"));
        }
        if let Some(parent) = &fields.parent_id
            && (!store::is_main_product(&mut tx, &t, parent).await? || fields.options.is_empty())
        {
            return Err(bad("Variant parent and options required"));
        }
        if !store::insert(
            &mut tx,
            &t,
            &id,
            &main_name,
            &main_description,
            fields.parent_id.as_deref(),
        )
        .await?
        {
            return Err(conflict("Product already exists"));
        }
    }
    if !create {
        let clocks = store::lock_clocks(&mut tx, &t, &id)
            .await?
            .ok_or(Error(StatusCode::NOT_FOUND, "Product not found".into()))?;
        // Content-only requests cannot carry an absolute stock value. Legacy clients
        // and explicit stock edits retain aggregate CAS, including checkout changes.
        let check = edit.expected(clocks.revision, clocks.content_revision);
        if check.current != check.expected {
            return Err(Error::revision_at(
                check.current,
                check.pointer,
                "Product changed",
            ));
        }
        edit.revision = clocks.revision;
        if edit
            .catalog
            .as_ref()
            .is_some_and(|c| c.parent_id != clocks.parent_id)
        {
            return Err(bad("Variant parent cannot be changed"));
        }
    }
    if let Some(c) = &edit.catalog {
        product_identity::save(&mut tx, &t, &id, c).await?;
    }
    for product in &edit.extra.cross_selling {
        if !veyndo_catalog::store::products::exists(&mut tx, &t, product).await? || product == &id {
            return Err(bad("Invalid cross-selling product"));
        }
    }
    if !store::update_content(
        &mut tx,
        &t,
        &id,
        json!(edit.extra),
        &main_name,
        &main_description,
        edit.revision,
    )
    .await?
    {
        return Err(conflict("Product changed"));
    }
    if let Some(fields) = &edit.commerce {
        product_fields::save(&mut tx, &t, &id, fields).await?;
    }
    for (lang, tr) in edit.translations {
        let locale = languages.locale(&lang).ok_or(bad("Unsupported locale"))?;
        store::upsert_translation(&mut tx, &t, &id, locale, tr.name, tr.description).await?;
    }
    let (saved, content_revision) = store::saved(&mut tx, &t, &id).await?;
    knowledge::sync_product(&mut tx, &t, &json!(saved)).await?;
    veyndo_core::outbox::publish(
        &mut tx,
        &t,
        if create {
            "product.created"
        } else {
            "product.updated"
        },
        json!({"productId":id,"revision":edit.revision+1}),
    )
    .await?;
    let result = json!({"id":id,"saved":true,"revision":saved.revision,"contentRevision":content_revision,"stock":saved.stock});
    tx.commit().await?;
    Ok(Json(result))
}
pub(crate) async fn product_editor(
    State(a): State<App>,
    h: RequestContext,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    auth::permit(&h, "catalog.read")?;
    let t = merchant(&a, &h)?;
    let r = sqlx::query("SELECT * FROM products WHERE tenant=$1 AND id=$2")
        .bind(&t)
        .bind(&id)
        .fetch_optional(&a.db)
        .await?
        .ok_or(Error(StatusCode::NOT_FOUND, "Product unavailable".into()))?;
    let (settings, _) = config(&a, &t).await?;
    let rows=sqlx::query("SELECT l.locale,p.name,p.description FROM languages l LEFT JOIN product_translations p ON p.language_id=l.id AND p.tenant=$1 AND p.product_id=$2 WHERE l.locale=ANY($3)").bind(&t).bind(&id).bind(&settings.locales).fetch_all(&a.db).await?;
    let mut translations = json!({});
    for tr in rows {
        let locale: String = tr.get("locale");
        let key = super::product_languages::key(&locale, &settings);
        translations[&key] = json!({"name":tr.get::<Option<String>,_>("name"),"description":tr.get::<Option<String>,_>("description")});
    }
    // Old catalogs may have only the canonical source columns. Seed only the main
    // field in the editable response; other languages remain NULL (inherit).
    let main = super::product_languages::key(&settings.main_locale, &settings);
    if translations[&main]["name"].is_null() {
        translations[&main]["name"] = json!(r.get::<String, _>("name"));
    }
    if translations[&main]["description"].is_null() {
        translations[&main]["description"] = json!(r.get::<String, _>("description"));
    }
    let channels = product_channels(&a, &t, &id).await?;
    let selected_channels = channels
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["visible"] == true)
        .map(|c| c["id"].clone())
        .collect::<Vec<_>>();
    let category_ids = veyndo_catalog::store::categories::of_product(&a.core, &t, &id).await?;
    let mut value = json!({"id":id,"mainLocale":settings.main_locale,"availableLocales":settings.locales,"channels":channels,"catalog":{"salesChannelIds":selected_channels,"active":r.get::<bool,_>("active"),"productNumber":r.get::<Option<String>,_>("product_number").unwrap_or_else(||id.clone()),"categoryIds":category_ids,"parentId":r.get::<Option<String>,_>("parent_id"),"options":r.get::<Value,_>("options")},"revision":r.get::<i64,_>("revision"),"contentRevision":r.get::<i64,_>("content_revision"),"translations":translations,"extra":({let mut extra=r.get::<Value,_>("extra");if extra["priceCurrency"].is_null(){extra["priceCurrency"]=json!(settings.currencies.pricing_currency);}extra}),"commerce":editable_fields(&r)});
    apps::core_extensions::attach(
        &a,
        &h,
        apps::core_extensions::Object::Product,
        std::slice::from_mut(&mut value),
    )
    .await?;
    Ok(Json(value))
}
