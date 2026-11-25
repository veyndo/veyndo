//! Restore snapshots by replaying validated entity edits; financial effects and publication are never copied from historical state.
use super::product;
use crate::foundation::App;
use crate::foundation::merchant;
use crate::{auth, b2b, categories, commerce, documents, marketing, operations};
use axum::Json;
use axum::extract::Path;
use axum::extract::State;
use serde::Deserialize;
use serde_json::Value;
use serde_json::json;
use veyndo_core::Result;
use veyndo_core::bad;
use veyndo_core::document::view;
use veyndo_core::request_context::RequestContext;
/// The audited row columns a restoration replays; each passes on exactly as stored (absent reads as null).
#[derive(Deserialize, Default)]
#[serde(default)]
struct Snapshot {
    data: Value,
    company_id: Value,
    parent_id: Value,
    position: Value,
    name: Value,
    condition: Value,
    active: Value,
    title: Value,
    content: Value,
    kind: Value,
    locale: Value,
    product_id: Value,
    translations: Value,
}

pub(super) async fn apply(
    a: App,
    h: RequestContext,
    entity: &str,
    id: String,
    state: Value,
    revision: i64,
) -> Result<Json<Value>> {
    let snapshot: Snapshot = view(&state);
    let data = snapshot.data.clone();
    match entity {
 "role"=>Ok(Json(auth::roles::invoke(&a,&h,"team.role.save",&json!({"id":id,"revision":revision,"data":data})).await?)),
 "b2bCompany" | "b2bUnit" => {
  // Replay configuration only. Current employees/catalog, quotes and budget allocations are never rewound.
  let args=if entity=="b2bUnit" {json!({"id":id,"revision":revision,"data":data,"companyId":snapshot.company_id,"parentId":snapshot.parent_id})} else {json!({"id":id,"revision":revision,"data":data})};
  Ok(Json(Box::pin(b2b::restore_configuration(&a,&h,if entity=="b2bCompany" {"company.save"} else {"unit.save"},&args)).await?))
 },
 "product"=>product::restore(a,h,id,state,revision).await,
 "customer"=>operations::restore_customer(a.core.clone(),h,id,state,revision).await,
 "category"=>categories::save_category(State(a),h,Path(id),Json(json!({"revision":revision,"parentId":snapshot.parent_id,"position":snapshot.position,"data":data}))).await,
 "settings"=>commerce::save_config(State(axum::extract::FromRef::from_ref(&a)),h,Json(json!({"revision":revision,"data":data}))).await,
 "company"=>operations::restore_company(&a,&h,json!({"revision":revision,"data":data}),None).await,
 "companyChannel"=>{
  let t=merchant(&a,&h)?;let base=veyndo_orders::store::receipts::settings_revision(&a,&t).await?;
  operations::restore_company(&a,&h,json!({"revision":revision,"data":data,"baseRevision":base}),Some(&id)).await
 },
 "checkoutChannel"=>{
  let t=merchant(&a,&h)?;let (base,base_revision)=commerce::config(&a,&t).await?;
  let effective=commerce::restore_checkout_data(&base,data)?;
  commerce::save_scope(State(axum::extract::FromRef::from_ref(&a)),h,Path(id),Json(json!({"revision":revision,"baseRevision":base_revision,"data":effective}))).await
 },
 "rule"|"flow"|"promotion"|"channel"=>{
  let kind=match entity{"rule"=>"rules","flow"=>"flows","promotion"=>"promotions",_=>"channels"};
  let data=if entity=="rule"{json!({"name":snapshot.name,"condition":snapshot.condition,"active":snapshot.active})}else{data};
  marketing::restore_config(State(a),h,Path((kind.into(),id)),Json(json!({"revision":revision,"data":data}))).await
 },
 "source"=>documents::restore_edit(State(a),h,Path(id),Json(json!({"revision":revision,"title":snapshot.title,"content":snapshot.content,"kind":snapshot.kind,"locale":snapshot.locale,"productId":snapshot.product_id,"translations":snapshot.translations}))).await,
 _=>Err(bad("History restoration unavailable"))
 }
}
