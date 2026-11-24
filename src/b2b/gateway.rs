//! Company command routes and the server side of the procurement port; the B2B component owns the command contract.
use crate::foundation::App;
use crate::foundation::merchant;
use crate::{apps, auth};
use axum::Json;
use axum::extract::FromRef;
use axum::extract::Path;
use axum::extract::State;
use axum::http::StatusCode;
use serde_json::Value;
use std::collections::HashMap;
use veyndo_b2b::domain::contract::permission;
use veyndo_core::Error;
use veyndo_core::PortFuture;
use veyndo_core::Result;
use veyndo_core::bad;
use veyndo_core::request_context::RequestContext;
use veyndo_pricing::domain::cart::Item;
use veyndo_shop::types::cart::StoredCart;

pub(super) async fn admin(
    State(a): State<App>,
    h: RequestContext,
    Path(op): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let name = format!("b2b.{op}");
    let permission = permission(&name).ok_or(bad(
        "Use the authenticated customer API for employee operations",
    ))?;
    merchant(&a, &h)?;
    auth::permit(&h, permission)?;
    Ok(Json(invoke(&a, &h, &name, &v).await?))
}
pub(super) async fn customer(
    State(a): State<App>,
    h: RequestContext,
    Path(op): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let name = format!("b2b.{op}");
    if permission(&name).is_some() {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Merchant operation requires Studio authorization".into(),
        ));
    }
    Ok(Json(invoke(&a, &h, &name, &v).await?))
}
pub(crate) async fn invoke(a: &App, h: &RequestContext, name: &str, v: &Value) -> Result<Value> {
    veyndo_b2b::service::commands::invoke(&Procurement::new(a), a, h, name, v).await
}

/// The cart, order, settings and app owners the company commands reach.
pub(super) struct Procurement<'s> {
    app: &'s App,
    carts: crate::cart_storage::Carts<'s>,
    lists: ListCarts<'s>,
}
impl<'s> Procurement<'s> {
    pub(super) fn new(app: &'s App) -> Self {
        Self {
            app,
            carts: crate::cart_storage::Carts(app),
            lists: ListCarts(app),
        }
    }
}
impl veyndo_b2b::ports::Procurement for Procurement<'_> {
    fn carts(&self) -> &dyn veyndo_shop::ports::Carts {
        &self.carts
    }
    fn lists(&self) -> &dyn veyndo_b2b::ports::ListCarts {
        &self.lists
    }
    fn lifecycle(&self) -> &dyn veyndo_b2b::ports::QuoteLifecycle {
        &QuoteLifecycle
    }
    fn customer<'a>(&'a self, h: &'a RequestContext) -> PortFuture<'a, ()> {
        Box::pin(async move {
            crate::accounts::identity(self.app, h).await?;
            Ok(())
        })
    }
    fn order(&self, raw: &Value, created: &str) -> Value {
        crate::accounts::project_order(raw, created)
    }
    fn labels<'a>(&'a self, tenant: &'a str) -> PortFuture<'a, Value> {
        Box::pin(async move {
            let (machine, _) = crate::commerce::machine(&self.app.db, tenant).await?;
            Ok(serde_json::json!(
                machine
                    .states
                    .iter()
                    .map(|s| (&s.id, &s.label))
                    .collect::<HashMap<_, _>>()
            ))
        })
    }
    fn cashier(&self, h: &RequestContext, cart: &StoredCart) -> Result<()> {
        crate::pos::guard_cart(h, cart)
    }
    fn locked<'a, 'c: 'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        tenant: &'a str,
        channel: &'a str,
    ) -> PortFuture<'a, (veyndo_shop::Settings, i64)> {
        Box::pin(crate::commerce::scoped_locked(tx, tenant, channel))
    }
    fn save<'a>(&'a self, h: &'a RequestContext, v: Value) -> PortFuture<'a, Value> {
        Box::pin(async move {
            Ok(
                crate::commerce::save_config(
                    State(FromRef::from_ref(self.app)),
                    h.clone(),
                    Json(v),
                )
                .await?
                .0,
            )
        })
    }
    fn companies<'a>(&'a self, h: &'a RequestContext, rows: &'a mut [Value]) -> PortFuture<'a, ()> {
        Box::pin(apps::core_extensions::attach(
            self.app,
            h,
            apps::core_extensions::Object::Company,
            rows,
        ))
    }
}

/// The app platform compiles the lifecycle a new quote is accepted under.
struct QuoteLifecycle;
impl veyndo_b2b::ports::QuoteLifecycle for QuoteLifecycle {
    fn capture<'a>(
        &'a self,
        conn: &'a mut sqlx::PgConnection,
        tenant: &'a str,
    ) -> PortFuture<'a, Value> {
        Box::pin(apps::native_lifecycle::capture(conn, tenant, "quote"))
    }
}

/// App configuration admission and the configured cart edit of procurement lists.
struct ListCarts<'a>(&'a App);
impl veyndo_b2b::ports::ListCarts for ListCarts<'_> {
    fn check<'a>(
        &'a self,
        tenant: &'a str,
        selection: &'a apps::ConfigurationSelection,
    ) -> PortFuture<'a, ()> {
        Box::pin(async move {
            apps::configuration_selection_for_list(self.0, tenant, selection).await?;
            Ok(())
        })
    }
    fn apply<'a>(
        &'a self,
        h: &'a RequestContext,
        items: Vec<Item>,
        expected: i64,
        selections: &'a [apps::ConfigurationSelection],
    ) -> PortFuture<'a, Value> {
        Box::pin(async move {
            let c = crate::cart_mutation::set_configured_cart(
                self.0,
                h,
                items,
                Some(expected),
                None,
                Some(selections),
            )
            .await?;
            crate::cart_price::cart_json(self.0, &c).await
        })
    }
}
