//! What quotes and procurement lists need from the server: the compiled app lifecycle of a new quote, app configuration admission, the configured cart edit and the records of other components (carts, customers, orders, products) procurement reads inside its own transactions.
use crate::store::lists::Sku;
use crate::store::units::{Receipt, UnitOrder};
use serde_json::Value;
use sqlx::PgConnection;
use std::sync::OnceLock;
use veyndo_core::{PortFuture, RequestContext, State};
use veyndo_shop::domain::configuration_selection::Selection;
use veyndo_shop::types::cart::Item;
use veyndo_shop::types::cart::StoredCart;

/// The app lifecycle of quotes; the app platform compiles it from the active packages.
pub trait QuoteLifecycle: Send + Sync {
    /// The lifecycle snapshot a new quote carries, or null when no app declares one; held until the quote commits.
    fn capture<'a>(
        &'a self,
        conn: &'a mut sqlx::PgConnection,
        tenant: &'a str,
    ) -> PortFuture<'a, Value>;
}

/// What applying and saving a procurement list needs from the server: app configuration admission and the configured cart edit.
pub trait ListCarts: Send + Sync {
    /// Checks that a saved selection still resolves against the shop's current packages.
    fn check<'a>(&'a self, tenant: &'a str, selection: &'a Selection) -> PortFuture<'a, ()>;
    /// Replaces the request cart's lines and configurations at the expected revision and returns the priced cart.
    fn apply<'a>(
        &'a self,
        h: &'a RequestContext,
        items: Vec<Item>,
        expected: i64,
        selections: &'a [Selection],
    ) -> PortFuture<'a, Value>;
}

/// What the company command gateway needs from components that depend on B2B: carts, lists, the quote lifecycle, customer sessions, order projection and state labels, the cashier guard, scoped settings, the settings writer and app extension fields.
pub trait Procurement: Send + Sync {
    /// The guarded request cart, its priced view and line replacement.
    fn carts(&self) -> &dyn veyndo_shop::ports::Carts;
    /// App configuration admission and the configured cart edit of procurement lists.
    fn lists(&self) -> &dyn ListCarts;
    /// The compiled app lifecycle of a new quote.
    fn lifecycle(&self) -> &dyn QuoteLifecycle;
    /// Refuses a request without a current customer session.
    fn customer<'a>(&'a self, h: &'a RequestContext) -> PortFuture<'a, ()>;
    /// The customer-safe projection of one stored order.
    fn order(&self, raw: &Value, created: &str) -> Value;
    /// The configured order state labels of the shop, by state ID.
    fn labels<'a>(&'a self, tenant: &'a str) -> PortFuture<'a, Value>;
    /// Refuses a cart that belongs to another cashier or a POS register.
    fn cashier(
        &self,
        h: &RequestContext,
        cart: &veyndo_shop::types::cart::StoredCart,
    ) -> veyndo_core::Result<()>;
    /// The settings of a sales channel under the basis row lock, with native rule conditions checked.
    fn locked<'a, 'c: 'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        tenant: &'a str,
        channel: &'a str,
    ) -> PortFuture<'a, (veyndo_shop::Settings, i64)>;
    /// The revisioned settings writer with its audit and outbox.
    fn save<'a>(&'a self, h: &'a RequestContext, v: Value) -> PortFuture<'a, Value>;
    /// Attaches the app extension fields of companies to the rows.
    fn companies<'a>(&'a self, h: &'a RequestContext, rows: &'a mut [Value]) -> PortFuture<'a, ()>;
}

/// A requested product with its parent and whether one of its variants is among the listed ids.
pub struct CatalogProduct {
    pub id: String,
    pub parent_id: Option<String>,
    pub variant_listed: bool,
}

/// The contact fields a company screen shows of a member.
pub struct Contact {
    pub id: String,
    pub email: String,
    pub profile: Value,
}

/// Rows of other components that procurement reads or changes inside its own statements and transactions: carts (checkout), customer accounts and sessions (customers), unit orders and receipts (orders) and product identity (catalog). The server implements it over the owners' stores and installs it once at startup.
pub trait Records: Send + Sync {
    /// The cart behind `token`, unlocked.
    fn cart_by_token<'a>(
        &'a self,
        a: &'a State,
        tenant: &'a str,
        token: &'a str,
    ) -> PortFuture<'a, Option<StoredCart>>;
    /// The cart behind `token`, locked for this transaction.
    fn lock_cart_by_token<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        tenant: &'a str,
        token: &'a str,
    ) -> PortFuture<'a, Option<StoredCart>>;
    /// Cart `id`, locked for this transaction.
    fn lock_cart<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        tenant: &'a str,
        id: &'a str,
    ) -> PortFuture<'a, Option<StoredCart>>;
    /// The revision of open cart `id`, locked for update, or share-locked when `shared`.
    fn open_cart_revision<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        tenant: &'a str,
        id: &'a str,
        shared: bool,
    ) -> PortFuture<'a, Option<i64>>;
    /// Stores the cart document at `revision`.
    fn save_cart<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        cart: (&'a str, &'a str),
        data: &'a Value,
        revision: i64,
    ) -> PortFuture<'a, ()>;
    /// The active customer behind the live session `digest`, share-locking the session and the account.
    fn session_customer<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        tenant: &'a str,
        digest: &'a str,
    ) -> PortFuture<'a, Option<String>>;
    /// Whether customer `id` exists.
    fn customer_exists<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        tenant: &'a str,
        id: &'a str,
    ) -> PortFuture<'a, bool>;
    /// The contact fields of the customers among `ids`, in no particular order.
    fn contacts<'a>(
        &'a self,
        a: &'a State,
        tenant: &'a str,
        ids: &'a [String],
    ) -> PortFuture<'a, Vec<Contact>>;
    /// The unit's orders, or only order `id`, newest first (at most 100).
    fn unit_orders<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        tenant: &'a str,
        unit: &'a str,
        id: Option<&'a str>,
    ) -> PortFuture<'a, Vec<UnitOrder>>;
    /// The receipts issued for order `id`, newest first (at most 100).
    fn receipts<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        tenant: &'a str,
        id: &'a str,
    ) -> PortFuture<'a, Vec<Receipt>>;
    /// The products among `ids` that exist, each with its parent and whether a variant of it is in `listed`.
    fn catalog_products<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        tenant: &'a str,
        ids: &'a [String],
        listed: &'a [String],
    ) -> PortFuture<'a, Vec<CatalogProduct>>;
    /// How many of `ids` are products of the shop.
    fn products_found<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        tenant: &'a str,
        ids: &'a [String],
    ) -> PortFuture<'a, i64>;
    /// The products whose numbers are among `skus`.
    fn products_by_sku<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        tenant: &'a str,
        skus: &'a [String],
    ) -> PortFuture<'a, Vec<Sku>>;
}

static RECORDS: OnceLock<Box<dyn Records>> = OnceLock::new();

/// Installs the server's records port at startup; a second installation keeps the first.
pub fn install(records: Box<dyn Records>) {
    let _ = RECORDS.set(records);
}

/// The installed records port; refused while the server has not installed it.
pub fn records() -> veyndo_core::Result<&'static dyn Records> {
    RECORDS.get().map(Box::as_ref).ok_or(veyndo_core::Error(
        veyndo_core::StatusCode::INTERNAL_SERVER_ERROR,
        "B2B records port is not installed".into(),
    ))
}
