//! Store isolation under forced row security: component store functions write under shop A and, on the non-owner runtime role, see nothing of it and change nothing under shop B.
//!
//! Run by the registered `store_isolation` suite (`scripts/store_isolation.py`), which
//! provisions the restricted login and passes `STORE_ISOLATION_RUNTIME_URL`. Fixture rows the
//! store functions only read (a customer, a cart, an order) are written with the owner login.
use serde_json::{Value, json};
use sqlx::PgConnection;
use sqlx::postgres::{PgPool, PgPoolOptions};
use std::sync::Arc;
use veyndo_core::State;
use veyndo_runtime::scoped_pool::ScopedPool;
use veyndo_runtime::tenant_scope::{Scope, scoped};

struct Fixture {
    runtime: ScopedPool,
    state: State,
    owner: PgPool,
    a: String,
    b: String,
    run: String,
    email: String,
    token: String,
    order: String,
}

fn env(key: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| panic!("{key} is set by scripts/store_isolation.py"))
}

async fn fixture() -> Fixture {
    let owner = PgPoolOptions::new()
        .max_connections(2)
        .connect(&env("DATABASE_URL"))
        .await
        .expect("owner login");
    let raw = PgPoolOptions::new()
        .max_connections(2)
        .connect(&env("STORE_ISOLATION_RUNTIME_URL"))
        .await
        .expect("runtime login");
    let (bypass, forced): (bool, bool) = sqlx::query_as(
        "SELECT r.rolsuper OR r.rolbypassrls, (SELECT bool_and(c.relforcerowsecurity) FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname='public' AND c.relkind='r' AND EXISTS(SELECT 1 FROM pg_attribute a WHERE a.attrelid=c.oid AND a.attname='tenant' AND NOT a.attisdropped)) FROM pg_roles r WHERE r.rolname=current_user",
    )
    .fetch_one(&raw)
    .await
    .expect("role audit");
    assert!(!bypass, "the runtime login must not bypass row security");
    assert!(forced, "every tenant table forces row security");
    let runtime = ScopedPool::new(raw, true);
    let state = State {
        db: runtime.clone(),
        http: reqwest::Client::new(),
        admission: Arc::new(veyndo_core::admission::Admission::default()),
        metrics: Arc::new(veyndo_core::channel_metrics::ChannelMetrics::default()),
    };
    let run = uuid::Uuid::new_v4().simple().to_string()[..12].to_string();
    let a = std::env::var("STORE_ISOLATION_SHOP").unwrap_or_else(|_| "atelier".into());
    let b = format!("isolation-{run}");
    let email = format!("buyer-{run}@example.test");
    let token = format!("token-{run}");
    let order = format!("order-{run}");
    let cart = format!("cart-{run}");
    let setup = [
        (
            "INSERT INTO tenants(id,name) VALUES($1,'Isolation shop B')",
            vec![b.clone()],
        ),
        (
            "INSERT INTO customers(tenant,email,password_hash) VALUES($1,$2,'unused')",
            vec![a.clone(), email.clone()],
        ),
        (
            "INSERT INTO carts(id,tenant,token,data) VALUES($1,$2,$3,'{\"items\":[],\"group\":\"consumer\",\"email\":null,\"company\":null,\"session\":\"isolation\",\"buyer\":null,\"order\":null}')",
            vec![cart.clone(), a.clone(), token.clone()],
        ),
        (
            "INSERT INTO orders(id,tenant,cart_id,idempotency_key,fingerprint,data) VALUES($1,$2,$3,$1,$1,jsonb_build_object('id',$1,'state','placed'))",
            vec![order.clone(), a.clone(), cart],
        ),
    ];
    for (statement, binds) in setup {
        let mut query = sqlx::query(sqlx::AssertSqlSafe(statement));
        for value in binds {
            query = query.bind(value);
        }
        query.execute(&owner).await.expect(statement);
    }
    Fixture {
        runtime,
        state,
        owner,
        a,
        b,
        run,
        email,
        token,
        order,
    }
}

/// One transaction of `work` under `scope`, committed only when asked.
async fn within<T>(
    pool: &ScopedPool,
    scope: Scope,
    commit: bool,
    work: impl AsyncFnOnce(&mut PgConnection) -> T,
) -> T {
    scoped(scope, async {
        let mut tx = pool.begin().await.expect("scoped transaction");
        let out = work(&mut tx).await;
        if commit {
            tx.commit().await.expect("commit");
        }
        out
    })
    .await
}

fn shop(t: &str) -> Scope {
    Scope::Tenant(t.into())
}

async fn owner_count(f: &Fixture, statement: &str, key: &str) -> i64 {
    sqlx::query_scalar(sqlx::AssertSqlSafe(statement))
        .bind(&f.a)
        .bind(key)
        .fetch_one(&f.owner)
        .await
        .expect(statement)
}

/// Core outbox and the apps webhook receipts that reference it.
async fn core_and_apps(f: &Fixture) {
    use veyndo_app_platform::store::webhook_receipts::{Receipt, find, record};
    use veyndo_core::store::outbox::publish_id_on;
    let (a, b) = (f.a.as_str(), f.b.as_str());
    let probe = json!({"probe": f.run});
    let event = within(&f.runtime, shop(a), true, async |c: &mut PgConnection| {
        publish_id_on(&mut *c, a, "isolation.probe", &probe)
            .await
            .expect("publish under A")
    })
    .await;
    let foreign = within(&f.runtime, shop(b), false, async |c: &mut PgConnection| {
        publish_id_on(&mut *c, a, "isolation.probe", &probe).await
    })
    .await;
    assert!(foreign.is_err(), "core: B published an event for A");
    assert_eq!(
        owner_count(
            f,
            "SELECT count(*) FROM outbox WHERE tenant=$1 AND data->>'probe'=$2",
            &f.run
        )
        .await,
        1
    );
    let receipt = |digest: &str| Receipt {
        digest: digest.into(),
        event_id: event,
    };
    within(&f.runtime, shop(a), true, async |c: &mut PgConnection| {
        record(c, a, "probe-app", "hook", &f.run, receipt("a"))
            .await
            .expect("record under A")
    })
    .await;
    let seen = within(&f.runtime, shop(b), false, async |c: &mut PgConnection| {
        find(c, a, "probe-app", "hook", &f.run)
            .await
            .expect("find under B")
    })
    .await;
    assert!(seen.is_none(), "apps: B read A's webhook receipt");
    let written = within(&f.runtime, shop(b), false, async |c: &mut PgConnection| {
        record(c, a, "probe-app", "hook", "other", receipt("b")).await
    })
    .await;
    assert!(written.is_err(), "apps: B recorded a receipt for A");
    let own = within(&f.runtime, shop(a), false, async |c: &mut PgConnection| {
        find(c, a, "probe-app", "hook", &f.run)
            .await
            .expect("find under A")
    })
    .await;
    assert_eq!(own.map(|r| r.digest), Some("a".into()));
}

/// Catalog products and the inventory reservations that reference them.
async fn catalog_and_inventory(f: &Fixture) {
    use veyndo_catalog::store::product_edit::{insert, is_main_product, saved, update_content};
    use veyndo_catalog::store::products::exists;
    use veyndo_inventory::InventoryLines;
    use veyndo_inventory::store::reservations::{allocate, release};
    let (a, b) = (f.a.as_str(), f.b.as_str());
    let product = format!("lamp-{}", f.run);
    let inserted = within(&f.runtime, shop(a), true, async |c: &mut PgConnection| {
        insert(c, a, &product, "Lamp", "Kept", None)
            .await
            .expect("insert under A")
    })
    .await;
    assert!(inserted);
    let (main, found, changed) =
        within(&f.runtime, shop(b), false, async |c: &mut PgConnection| {
            (
                is_main_product(&mut *c, a, &product)
                    .await
                    .expect("read under B"),
                exists(&mut *c, a, &product).await.expect("exists under B"),
                update_content(&mut *c, a, &product, json!({}), "Changed", "Changed", 0)
                    .await
                    .expect("update under B"),
            )
        })
        .await;
    assert!(
        !main && !found && !changed,
        "catalog: B saw or changed A's product"
    );
    let foreign = within(&f.runtime, shop(b), false, async |c: &mut PgConnection| {
        insert(c, a, &format!("{product}-b"), "Planted", "", None).await
    })
    .await;
    assert!(foreign.is_err(), "catalog: B inserted a product for A");
    let (product_row, _) = within(&f.runtime, shop(a), false, async |c: &mut PgConnection| {
        saved(c, a, &product).await.expect("saved under A")
    })
    .await;
    assert_eq!(product_row.name, "Lamp", "catalog: A's product changed");

    let lines = || InventoryLines::new([(product.as_str(), 2)].into_iter()).unwrap();
    within(&f.runtime, shop(a), true, async |c: &mut PgConnection| {
        allocate(c, a, &lines(), &f.order)
            .await
            .expect("allocate under A")
    })
    .await;
    within(&f.runtime, shop(b), true, async |c: &mut PgConnection| {
        release(c, a, &f.order)
            .await
            .expect("release under B is a no-op")
    })
    .await;
    let held = "SELECT count(*) FROM order_inventory_reservations WHERE tenant=$1 AND order_id=$2 AND released_at IS NULL";
    assert_eq!(
        owner_count(f, held, &f.order).await,
        1,
        "inventory: B released A's reservation"
    );
    let foreign = within(&f.runtime, shop(b), false, async |c: &mut PgConnection| {
        allocate(c, a, &lines(), &f.order).await
    })
    .await;
    assert!(foreign.is_err(), "inventory: B reserved stock of A");
    within(&f.runtime, shop(a), true, async |c: &mut PgConnection| {
        release(c, a, &f.order).await.expect("release under A")
    })
    .await;
    assert_eq!(
        owner_count(f, held, &f.order).await,
        0,
        "inventory: A could not release"
    );
}

/// Pricing jobs and shop limits/settings.
async fn pricing_and_shop(f: &Fixture) {
    use veyndo_pricing::store::price_jobs::{insert, queued};
    use veyndo_shop::store::operator_control::{limit_revision, upsert_limit};
    use veyndo_shop::store::settings::shared;
    let (a, b) = (f.a.as_str(), f.b.as_str());
    let job = format!("job-{}", f.run);
    within(&f.runtime, shop(a), true, async |c: &mut PgConnection| {
        insert(c, a, &job, json!({}), "")
            .await
            .expect("queue under A")
    })
    .await;
    let foreign_count = within(&f.runtime, shop(b), false, async |c: &mut PgConnection| {
        queued(c, a).await.expect("count under B")
    })
    .await;
    assert_eq!(foreign_count, 0, "pricing: B counted A's price jobs");
    let planted = within(&f.runtime, shop(b), false, async |c: &mut PgConnection| {
        insert(c, a, &format!("{job}-b"), json!({}), "").await
    })
    .await;
    assert!(planted.is_err(), "pricing: B queued a job for A");
    let own = within(&f.runtime, shop(a), false, async |c: &mut PgConnection| {
        queued(c, a).await.expect("count under A")
    })
    .await;
    assert!(own >= 1);

    // Operator limits are written with system authority only; a shop scope can never write them.
    within(
        &f.runtime,
        Scope::System,
        true,
        async |c: &mut PgConnection| upsert_limit(c, a, 7, 1).await.expect("operator limit"),
    )
    .await;
    let tenant_write = within(&f.runtime, shop(a), false, async |c: &mut PgConnection| {
        upsert_limit(c, a, 9, 1).await
    })
    .await;
    assert!(
        tenant_write.is_err(),
        "shop: a shop scope wrote its own operator limit"
    );
    let (limit, settings) = within(&f.runtime, shop(b), false, async |c: &mut PgConnection| {
        (
            limit_revision(&mut *c, a).await.expect("limit under B"),
            shared(&mut *c, a).await.is_ok(),
        )
    })
    .await;
    assert!(
        limit.is_none() && !settings,
        "shop: B read A's limit or settings"
    );
    let (limit, settings) = within(&f.runtime, shop(a), false, async |c: &mut PgConnection| {
        (
            limit_revision(&mut *c, a).await.expect("limit under A"),
            shared(&mut *c, a).await.is_ok(),
        )
    })
    .await;
    assert!(limit.is_some() && settings);
}

/// Customers and their sessions, read by customers and payments.
async fn customers_and_payments(f: &Fixture) {
    use veyndo_customers::store::addresses::lock_customer;
    use veyndo_customers::store::sessions::{delete, insert, valid};
    use veyndo_payments::store::funding::session_customer;
    let (a, b) = (f.a.as_str(), f.b.as_str());
    let digest = format!("digest-{}", f.run);
    scoped(shop(a), insert(&f.state, &digest, a, &f.email))
        .await
        .expect("session under A");
    let planted = scoped(
        shop(b),
        insert(&f.state, &format!("{digest}-b"), a, &f.email),
    )
    .await;
    assert!(
        planted.is_err(),
        "customers: B opened a session for A's customer"
    );
    assert!(
        !scoped(shop(b), valid(&f.state, a, &digest))
            .await
            .expect("valid under B"),
        "customers: B saw A's session"
    );
    scoped(shop(b), delete(&f.state, a, &digest))
        .await
        .expect("delete under B is a no-op");
    let (customer, paying) = within(&f.runtime, shop(b), false, async |c: &mut PgConnection| {
        (
            lock_customer(&mut *c, a, &f.email)
                .await
                .expect("customer under B"),
            session_customer(&mut *c, a, &digest)
                .await
                .expect("payer under B"),
        )
    })
    .await;
    assert!(
        !customer && paying.is_none(),
        "customers/payments: B saw A's customer"
    );
    assert!(
        scoped(shop(a), valid(&f.state, a, &digest))
            .await
            .expect("valid under A"),
        "customers: B ended A's session"
    );
    let paying = within(&f.runtime, shop(a), false, async |c: &mut PgConnection| {
        session_customer(c, a, &digest)
            .await
            .expect("payer under A")
    })
    .await;
    assert!(paying.is_some());
}

/// Carts as checkout, B2B, POS and marketing lock them; orders, roles, settings and exposures.
async fn carts_orders_and_reads(f: &Fixture) {
    let (a, b) = (f.a.as_str(), f.b.as_str());
    for (scope, visible) in [(shop(b), false), (shop(a), true)] {
        let seen = within(&f.runtime, scope, false, async |c: &mut PgConnection| {
            [
                veyndo_checkout::store::carts::lock_by_token(&mut *c, a, &f.token)
                    .await
                    .expect("checkout cart")
                    .is_some(),
                veyndo_b2b::store::carts::lock_by_token(&mut *c, a, &f.token)
                    .await
                    .expect("b2b cart")
                    .is_some(),
                veyndo_pos::store::carts::lock(&mut *c, a, &f.token)
                    .await
                    .expect("pos cart")
                    .is_some(),
                veyndo_marketing::store::carts::lock(&mut *c, a, &f.token)
                    .await
                    .is_ok(),
            ]
        })
        .await;
        assert_eq!(
            seen, [visible; 4],
            "checkout/b2b/pos/marketing carts: visible to A only"
        );
        let reads = within(
            &f.runtime,
            shop(if visible { a } else { b }),
            false,
            async |c: &mut PgConnection| {
                (
                    veyndo_identity::store::roles::count(&mut *c, a)
                        .await
                        .expect("roles"),
                    veyndo_pos::store::register::main_locale(&mut *c, a)
                        .await
                        .is_ok(),
                    veyndo_marketing::store::configuration_writes::settings(&mut *c, a, false)
                        .await
                        .is_ok(),
                )
            },
        )
        .await;
        assert_eq!(
            (reads.0 > 0, reads.1, reads.2),
            (visible, visible, visible),
            "identity/pos/marketing reads: visible to A only"
        );
        let tenant = if visible { a } else { b };
        let orders = scoped(
            shop(tenant),
            veyndo_orders::store::order_reads::recent(&f.state, a),
        )
        .await
        .expect("orders");
        assert_eq!(
            orders.iter().any(|o: &Value| o["id"] == json!(f.order)),
            visible,
            "orders: visible to A only"
        );
    }
    let session = format!("session-{}", f.run);
    let planted = within(&f.runtime, shop(b), false, async |c: &mut PgConnection| {
        veyndo_intelligence::store::personalization::expose(c, a, &session, "control", 0.5).await
    })
    .await;
    assert!(
        planted.is_err(),
        "intelligence: B recorded an exposure for A"
    );
}

#[tokio::test]
#[ignore = "needs the store_isolation suite database; run through scripts/store_isolation.py"]
async fn component_stores_keep_shops_apart() {
    let f = fixture().await;
    core_and_apps(&f).await;
    println!("PASS core, apps: outbox events and webhook receipts");
    catalog_and_inventory(&f).await;
    println!("PASS catalog, inventory: products and reservations");
    pricing_and_shop(&f).await;
    println!("PASS pricing, shop: price jobs, operator limits and settings");
    customers_and_payments(&f).await;
    println!("PASS customers, payments: sessions and customers");
    carts_orders_and_reads(&f).await;
    println!(
        "PASS checkout, b2b, pos, marketing, orders, identity, intelligence: carts, orders, roles, settings and exposures"
    );
}
