//! The merchant product list lives in the studio component; the server registers it with product creation.
use crate::auth::SecureRoutes;
use crate::foundation::App;
use axum::Router;
use axum::routing::get;

pub(crate) use veyndo_studio::http::products::list_products;
pub(crate) fn product_admin_router() -> Router<App> {
    Router::new().secure_route(
        "/api/merchant/products",
        &[("GET", "catalog.read"), ("POST", "catalog.write")],
        get(list_products).post(super::create_product),
    )
}
