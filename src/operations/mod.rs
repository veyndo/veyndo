//! Merchant CRM and fulfillment APIs, shared verbatim with MCP operations capabilities.
use crate::*;
mod addresses;
mod company_model;
mod company_public;
mod creation_schema;
pub(crate) use creation_schema::CAPABILITIES as CREATION_CAPABILITIES;
mod customer_create;
pub(crate) mod imports;
pub(crate) use creation_schema::schema as creation_schema;
mod customer_lookup;
mod manual_orders;
pub(crate) use master_data::save as restore_company;
pub(crate) use veyndo_studio::http::customers::restore_customer;
use veyndo_studio::http::{customers, orders, workflow};
mod master_data;
pub(crate) use master_data::lock as lock_company;
mod redacted_reads;
pub(crate) use redacted_reads::redacted_read;
mod delivery_booking;
mod order_documents;
mod receipt_pdf;
mod receipt_text;
pub(crate) use delivery_booking::booking_intent;
mod receipts;
pub(crate) use order_documents::{allocation_digest, document_response, import_document};
pub(crate) use receipts::{receipt_response, receipt_snapshot};
mod routes;
pub(crate) use routes::router;

pub(crate) use veyndo_orders::types::list_criteria::Criteria;
pub(crate) mod grid_schema;
pub(crate) use grid_schema::fields as grid_fields;
mod grid_plan;
pub(crate) use grid_plan::visible as grid_visible;
pub(crate) use veyndo_studio::domain::merchant_operations::{
    creation_visible, permission, requires_pii,
};
/// The record a merchant operation names.
fn record_id(v: &Value) -> Result<String> {
    v["id"]
        .as_str()
        .map(str::to_string)
        .ok_or(bad("id required"))
}
/// Products, categories and product files through their native handlers.
async fn catalog_operation(a: &App, h: &RequestContext, name: &str, v: &Value) -> Result<Value> {
    let id = || record_id(v);
    let Json(result) = match name {
        "merchant.products" => {
            commerce::list_products(
                State(axum::extract::FromRef::from_ref(a)),
                h.clone(),
                axum::extract::Query(
                    serde_json::from_value(v.clone())
                        .map_err(|_| bad("Invalid catalog criteria"))?,
                ),
            )
            .await?
        }
        "merchant.categories" => {
            crate::categories::list_categories(State(a.clone()), h.clone()).await?
        }
        "merchant.product.create" => {
            commerce::create_product(State(a.core.clone()), h.clone(), Json(v["product"].clone()))
                .await?
        }
        "merchant.category.create" => {
            crate::categories::create_category(
                State(a.clone()),
                h.clone(),
                Json(v["category"].clone()),
            )
            .await?
        }
        "merchant.category.save" => {
            crate::categories::save_category(
                State(a.clone()),
                h.clone(),
                Path(id()?),
                Json(v["category"].clone()),
            )
            .await?
        }
        "merchant.product.content" => {
            commerce::product_editor(State(a.core.clone()), h.clone(), Path(id()?)).await?
        }
        "merchant.product.save" => {
            commerce::edit_product(
                State(a.core.clone()),
                h.clone(),
                Path(id()?),
                Json(v["product"].clone()),
            )
            .await?
        }
        "merchant.product.assets" => {
            crate::assets::list(State(a.clone()), h.clone(), Path(id()?)).await?
        }
        "merchant.asset.publish" => {
            crate::assets::publish(State(a.clone()), h.clone(), Path(id()?), Json(v.clone()))
                .await?
        }
        _ => return Err(bad("Unknown operation")),
    };
    Ok(result)
}
pub(crate) async fn invoke(a: &App, h: &RequestContext, name: &str, v: &Value) -> Result<Value> {
    merchant(a, h)?;
    auth::permit(
        h,
        permission(name).ok_or(bad("Unknown merchant operation"))?,
    )?;
    if requires_pii(name) {
        auth::permit(h, "customers.pii")?;
    }
    let id = || record_id(v);
    let criteria = || serde_json::from_value(v.clone()).map_err(|_| bad("Invalid search criteria"));
    let Json(result) = match name {
        "merchant.import.record" => Json(imports::invoke(a, h, v).await?),
        "merchant.import.inspect" => Json(imports::inspect(a, h, v).await?),
        "merchant.customer.creation" => {
            customer_create::context(State(a.core.clone()), h.clone()).await?
        }
        "merchant.customer.create" => {
            customer_create::create(State(a.core.clone()), h.clone(), Json(v.clone())).await?
        }
        "merchant.order.context" | "merchant.order.review" | "merchant.order.complete" => Json(
            Box::pin(manual_orders::invoke(
                a,
                h,
                name.rsplit('.').next().unwrap(),
                v,
            ))
            .await?,
        ),
        "merchant.grid.plan" => {
            grid_plan::plan(State(a.clone()), h.clone(), Json(v.clone())).await?
        }
        "merchant.customer.lookup" => {
            customer_lookup::lookup(
                State(a.clone()),
                h.clone(),
                axum::extract::Query(criteria()?),
            )
            .await?
        }
        "merchant.customer.addresses" => {
            addresses::list(State(a.core.clone()), h.clone(), Path(id()?)).await?
        }
        "merchant.customer.address.save" => Json(
            accounts::address_save(a, &merchant(a, h)?, &id()?, v["addressId"].as_str(), v, h)
                .await?,
        ),
        "merchant.customer.address.delete" => Json(
            accounts::address_delete(
                a,
                &merchant(a, h)?,
                &id()?,
                v["addressId"].as_str().ok_or(bad("addressId required"))?,
                v["revision"].as_i64().ok_or(bad("revision required"))?,
                h,
            )
            .await?,
        ),
        "merchant.company" => master_data::read(a, h, v["channelId"].as_str()).await?,
        "merchant.company.save" => {
            master_data::save(a, h, v.clone(), v["channelId"].as_str()).await?
        }
        "merchant.workflow" => workflow::get(State(a.core.clone()), h.clone()).await?,
        "merchant.workflow.save" => {
            workflow::save(State(a.core.clone()), h.clone(), Json(v.clone())).await?
        }
        "merchant.products"
        | "merchant.categories"
        | "merchant.product.create"
        | "merchant.category.create"
        | "merchant.category.save"
        | "merchant.product.content"
        | "merchant.product.save"
        | "merchant.product.assets"
        | "merchant.asset.publish" => Json(Box::pin(catalog_operation(a, h, name, v)).await?),
        "merchant.orders" => {
            orders::list(
                State(a.core.clone()),
                h.clone(),
                axum::extract::Query(criteria()?),
            )
            .await?
        }
        "merchant.order" => orders::detail(State(a.core.clone()), h.clone(), Path(id()?)).await?,
        "merchant.delivery.split" => {
            commerce::split_delivery(
                State(a.core.clone()),
                h.clone(),
                Path(id()?),
                Json(v.clone()),
            )
            .await?
        }
        "merchant.customers" => {
            customers::list(
                State(a.core.clone()),
                h.clone(),
                axum::extract::Query(criteria()?),
            )
            .await?
        }
        "merchant.customer" => {
            customers::detail(State(a.core.clone()), h.clone(), Path(id()?)).await?
        }
        "merchant.customer.save" => {
            customers::save(
                State(a.core.clone()),
                h.clone(),
                Path(id()?),
                Json(v["customer"].clone()),
            )
            .await?
        }
        "merchant.order.transition" => {
            commerce::transition_order(
                State(a.core.clone()),
                h.clone(),
                Path(id()?),
                Json(v.clone()),
            )
            .await?
        }
        "merchant.order.note" => {
            orders::note(
                State(a.core.clone()),
                h.clone(),
                Path(id()?),
                Json(v.clone()),
            )
            .await?
        }
        "merchant.receipts" => receipts::list(State(a.clone()), h.clone(), Path(id()?)).await?,
        "merchant.receipt.create" => {
            receipts::create(State(a.clone()), h.clone(), Path(id()?), Json(v.clone())).await?
        }
        "merchant.payment.instructions"
        | "merchant.payment.details"
        | "merchant.payment.providers"
        | "merchant.payment.onboarding"
        | "merchant.payment" => Json(payments::merchant_operation(a, h, name, v).await?),
        _ => return Err(bad("Unknown operation")),
    };
    Ok(result)
}
