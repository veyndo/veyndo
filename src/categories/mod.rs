//! Tenant-scoped category tree, localized navigation and product assignment boundaries.
use crate::{App, SecureRoutes};
use axum::Router;
use axum::routing::get;
mod admin;
mod navigation;
pub(crate) use admin::{
    create as create_category, input_schema, list as list_categories, save as save_category,
};
pub(crate) use navigation::admit;
pub(crate) fn router() -> Router<App> {
    Router::new()
        .secure_route(
            "/api/merchant/categories",
            &[("GET", "catalog.read"), ("POST", "catalog.write")],
            get(veyndo_catalog::http::categories::list).post(admin::create),
        )
        .secure_route(
            "/api/merchant/categories/{id}",
            &[("PUT", "catalog.write")],
            axum::routing::put(veyndo_catalog::http::categories::save),
        )
        .route(
            "/store-api/navigation",
            get(veyndo_catalog::http::categories::navigation)
                .post(veyndo_catalog::http::categories::navigation),
        )
}

pub(crate) use veyndo_catalog::store::categories::seed;
