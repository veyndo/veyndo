//! The merchant product editor: revision-bound multilingual content, identity, commerce fields, channels and categories, checked against the shop settings, compliance rules, automation metadata and app extensions.
use crate::domain::views::Visibility;
use crate::store::products as studio_products;
use axum::Json;
use axum::extract::{Path, State as Shared};
use serde_json::{Value, json};
use veyndo_app_model::Object;
use veyndo_app_model::declarations::identifier;
use veyndo_app_platform::store::extension_projection::attach;
use veyndo_catalog::domain::product_admission::{category_assignment, category_edit};
use veyndo_catalog::service::{product_fields, product_identity};
use veyndo_catalog::store::product_edit as store;
use veyndo_catalog::store::{categories, channels, products};
use veyndo_catalog::types::product_edit::Edit;
use veyndo_core::document::{merge, view};
use veyndo_core::store::history_context::context;
use veyndo_core::{Error, RequestContext, Result, State, StatusCode, bad, conflict};
use veyndo_identity::{merchant, permit};
use veyndo_intelligence::knowledge::sync_product;
use veyndo_marketing::domain::metadata::validate as validate_metadata;
use veyndo_pricing::service::conversion::validate_prices;
use veyndo_shop::domain::defaults::decode;
use veyndo_shop::domain::legal_product::validate_product;
use veyndo_shop::service::read_cache::config;

/// The language key of a locale in the shop's settings.
fn language_key(locale: &str, s: &veyndo_shop::Settings) -> String {
    s.languages().key(locale)
}
/// Saves an existing product at its revision.
pub async fn edit_product(
    Shared(a): Shared<State>,
    h: RequestContext,
    Path(id): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let v = expand_category_edit(&a, &h, &id, v).await?;
    save_product(a, h, id, v, false).await
}
/// The shared revision-checked save of product content, identity, fields and translations.
pub async fn save_product(
    a: State,
    h: RequestContext,
    id: String,
    v: Value,
    create: bool,
) -> Result<Json<Value>> {
    permit(&h, "catalog")?;
    let t = merchant(&h)?;
    let mut edit: Edit = serde_json::from_value(v).map_err(|_| bad("Invalid product metadata"))?;
    let mut tx = a.db.begin().await?;
    context(&mut tx, &h, "merchant").await?;
    let (settings_data, settings_revision) = studio_products::share_settings(&mut tx, &t).await?;
    if edit
        .settings_revision
        .is_some_and(|revision| revision != settings_revision)
    {
        return Err(conflict("Catalogue settings changed; prepare fresh drafts"));
    }
    let settings = decode(settings_data)?;
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
    validate_prices(&edit.extra.currency_prices, &settings.currencies)?;
    let languages = settings.languages();
    edit.validate_translations(languages)?;
    validate_product(&edit.extra.compliance, &settings)?;
    let (main_name, main_description) = edit.main_text(languages)?;
    edit.validate_metadata(languages, |id| {
        settings.taxes.iter().any(|tax| tax.id == id)
    })?;
    validate_metadata("products", &edit.extra.automation)?;
    edit.validate_fields(languages)?;
    if let Some(c) = &edit.catalog {
        // A missing automation document becomes an object, as before.
        merge(
            &mut edit.extra.automation,
            json!({ "categoryIds": c.category_ids }),
        )?;
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
        if !products::exists(&mut tx, &t, product).await? || product == &id {
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
    sync_product(&mut tx, &t, &json!(saved)).await?;
    veyndo_core::store::outbox::publish(
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
/// The product as the merchant editor shows it, with languages, channels, categories and app extensions.
pub async fn product_editor(
    Shared(a): Shared<State>,
    h: RequestContext,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    permit(&h, "catalog.read")?;
    let t = merchant(&h)?;
    let r = studio_products::editor_row(&a, &t, &id)
        .await?
        .ok_or(Error(StatusCode::NOT_FOUND, "Product unavailable".into()))?;
    let (settings, _) = config(&a, &t).await?;
    let rows = studio_products::translations(&a, &t, &id, &settings.locales).await?;
    let mut translations = json!({});
    for (locale, name, description) in rows {
        let key = language_key(&locale, &settings);
        translations[&key] = json!({"name":name,"description":description});
    }
    // Old catalogs may have only the canonical source columns. Seed only the main
    // field in the editable response; other languages remain NULL (inherit).
    let main = language_key(&settings.main_locale, &settings);
    if translations[&main]["name"].is_null() {
        translations[&main]["name"] = json!(r.name);
    }
    if translations[&main]["description"].is_null() {
        translations[&main]["description"] = json!(r.description);
    }
    let channels = product_channels(&a, &t, &id).await?;
    let selected_channels = channels
        .as_array()
        .unwrap()
        .iter()
        .map(view::<Visibility>)
        .filter(|c| c.visible == true)
        .map(|c| c.id)
        .collect::<Vec<_>>();
    let category_ids = categories::of_product(&a, &t, &id).await?;
    let mut value = json!({"id":id,"mainLocale":settings.main_locale,"availableLocales":settings.locales,"channels":channels,"catalog":{"salesChannelIds":selected_channels,"active":r.active,"productNumber":r.product_number.clone().unwrap_or_else(||id.clone()),"categoryIds":category_ids,"parentId":r.parent_id,"options":r.options},"revision":r.revision,"contentRevision":r.content_revision,"translations":translations,"extra":({let mut extra=r.extra.clone();if extra.get("priceCurrency").is_none_or(Value::is_null){merge(&mut extra,json!({"priceCurrency":settings.currencies.pricing_currency}))?;}extra}),"commerce":r.commerce()});
    attach(&a, &h, Object::Product, std::slice::from_mut(&mut value)).await?;
    Ok(Json(value))
}
/// The channel visibility of a product for the editor, at most 100 channels.
async fn product_channels(a: &State, t: &str, id: &str) -> Result<Value> {
    let channels = channels::visibility(a, t, id).await?;
    if channels.len() > 100 {
        return Err(bad(
            "Interactive product editor supports at most 100 channels",
        ));
    }
    Ok(json!(channels))
}
/// Expands an explicit category-only change through the same revisioned editor.
async fn expand_category_edit(a: &State, h: &RequestContext, id: &str, v: Value) -> Result<Value> {
    if v.get("categoryIds").is_none() {
        return Ok(v);
    }
    permit(h, "catalog")?;
    let edit = category_edit(&v)?;
    let current = product_editor(Shared(a.clone()), h.clone(), Path(id.to_owned()))
        .await?
        .0;
    category_assignment(&edit, &current)
}
/// Creates a product with a generated or validated stable ID.
pub async fn create_product(
    Shared(a): Shared<State>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let id = match v.get("id") {
        None | Some(Value::Null) => uuid::Uuid::new_v4().to_string(),
        Some(Value::String(id)) if identifier(id) || uuid::Uuid::parse_str(id).is_ok() => {
            id.clone()
        }
        _ => return Err(bad("Invalid product ID")),
    };
    save_product(a, h, id, v, true).await
}
