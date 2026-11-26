//! The server side of the B2B records port: carts of checkout, customer accounts, unit orders and product identity, each read or changed by its owner's store on procurement's connection.
use serde_json::Value;
use sqlx::PgConnection;
use veyndo_b2b::ports::{CatalogProduct, Contact, Records};
use veyndo_b2b::store::lists::Sku;
use veyndo_b2b::store::units::{Receipt, UnitOrder};
use veyndo_catalog::store::unit_scope;
use veyndo_checkout::store::carts::{self, CartRow};
use veyndo_checkout::store::procurement_carts;
use veyndo_core::{PortFuture, State};
use veyndo_customers::store::procurement_accounts;
use veyndo_orders::store::unit_orders;
use veyndo_shop::types::cart::StoredCart;

/// The owners' stores behind the B2B records port.
pub(crate) struct ProcurementRecords;

impl Records for ProcurementRecords {
    fn cart_by_token<'a>(
        &'a self,
        a: &'a State,
        tenant: &'a str,
        token: &'a str,
    ) -> PortFuture<'a, Option<StoredCart>> {
        Box::pin(async move {
            procurement_carts::by_token(a, tenant, token)
                .await?
                .map(CartRow::into_cart)
                .transpose()
        })
    }
    fn lock_cart_by_token<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        tenant: &'a str,
        token: &'a str,
    ) -> PortFuture<'a, Option<StoredCart>> {
        Box::pin(async move {
            carts::lock_by_token(conn, tenant, token)
                .await?
                .map(CartRow::into_cart)
                .transpose()
        })
    }
    fn lock_cart<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        tenant: &'a str,
        id: &'a str,
    ) -> PortFuture<'a, Option<StoredCart>> {
        Box::pin(async move {
            procurement_carts::lock_by_id(conn, tenant, id)
                .await?
                .map(CartRow::into_cart)
                .transpose()
        })
    }
    fn open_cart_revision<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        tenant: &'a str,
        id: &'a str,
        shared: bool,
    ) -> PortFuture<'a, Option<i64>> {
        Box::pin(async move {
            if shared {
                procurement_carts::open_revision_shared(conn, tenant, id).await
            } else {
                procurement_carts::open_revision(conn, tenant, id).await
            }
        })
    }
    fn save_cart<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        (tenant, id): (&'a str, &'a str),
        data: &'a Value,
        revision: i64,
    ) -> PortFuture<'a, ()> {
        Box::pin(procurement_carts::save(conn, tenant, id, data, revision))
    }
    fn session_customer<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        tenant: &'a str,
        digest: &'a str,
    ) -> PortFuture<'a, Option<String>> {
        Box::pin(procurement_accounts::session_customer(conn, tenant, digest))
    }
    fn customer_exists<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        tenant: &'a str,
        id: &'a str,
    ) -> PortFuture<'a, bool> {
        Box::pin(procurement_accounts::exists(conn, tenant, id))
    }
    fn contacts<'a>(
        &'a self,
        a: &'a State,
        tenant: &'a str,
        ids: &'a [String],
    ) -> PortFuture<'a, Vec<Contact>> {
        Box::pin(async move {
            Ok(procurement_accounts::contacts(a, tenant, ids)
                .await?
                .into_iter()
                .map(|c| Contact {
                    id: c.id,
                    email: c.email,
                    profile: c.profile,
                })
                .collect())
        })
    }
    fn unit_orders<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        tenant: &'a str,
        unit: &'a str,
        id: Option<&'a str>,
    ) -> PortFuture<'a, Vec<UnitOrder>> {
        Box::pin(async move {
            Ok(unit_orders::orders(conn, tenant, unit, id)
                .await?
                .into_iter()
                .map(|o| UnitOrder {
                    data: o.data,
                    b2b: o.b2b,
                    created: o.created,
                })
                .collect())
        })
    }
    fn receipts<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        tenant: &'a str,
        id: &'a str,
    ) -> PortFuture<'a, Vec<Receipt>> {
        Box::pin(async move {
            Ok(unit_orders::receipts(conn, tenant, id)
                .await?
                .into_iter()
                .map(|r| Receipt {
                    id: r.id,
                    kind: r.kind,
                    number: r.number,
                    locale: r.locale,
                    created: r.created,
                })
                .collect())
        })
    }
    fn catalog_products<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        tenant: &'a str,
        ids: &'a [String],
        listed: &'a [String],
    ) -> PortFuture<'a, Vec<CatalogProduct>> {
        Box::pin(async move {
            Ok(unit_scope::scope(conn, tenant, ids, listed)
                .await?
                .into_iter()
                .map(|p| CatalogProduct {
                    id: p.id,
                    parent_id: p.parent_id,
                    variant_listed: p.variant_listed,
                })
                .collect())
        })
    }
    fn products_found<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        tenant: &'a str,
        ids: &'a [String],
    ) -> PortFuture<'a, i64> {
        Box::pin(unit_scope::count(conn, tenant, ids))
    }
    fn products_by_sku<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        tenant: &'a str,
        skus: &'a [String],
    ) -> PortFuture<'a, Vec<Sku>> {
        Box::pin(async move {
            Ok(unit_scope::by_sku(conn, tenant, skus)
                .await?
                .into_iter()
                .map(|s| Sku {
                    id: s.id,
                    product_number: s.product_number,
                })
                .collect())
        })
    }
}
