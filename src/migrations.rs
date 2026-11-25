//! Fresh-installation setup, separate from serving: applies the component schema baselines in their
//! global order and records each one in the ledger. There is no upgrade path from a database that an
//! earlier release set up with incremental migrations; such a database is refused.
use crate::foundation::App;
use crate::seed::seed;
use crate::store::migrations as store;
use crate::store::shops;
use serde_json::Value;
use serde_json::json;
use sqlx::PgPool;
use veyndo_core::hash;
use veyndo_intelligence::knowledge;

mod schema;

/// The first migration every database of an earlier release recorded.
const EARLIER_RELEASE: &str = "001";

pub(crate) async fn apply(pool: &PgPool) {
    store::enter_trusted(pool)
        .await
        .expect("trusted migration context");
    // The dedicated setup pool has exactly one connection, retaining this
    // session lock through schema setup and the bounded demo seed.
    store::lock(pool).await.expect("migration lock");
    store::ensure_ledger(pool).await.expect("migration ledger");
    let earlier = store::applied_checksum(pool, EARLIER_RELEASE)
        .await
        .expect("migration status");
    assert!(
        earlier.is_none(),
        "This database was set up by an earlier release; there is no upgrade path. Set up a new database"
    );
    for (version, source) in schema::BASELINES {
        let checksum = hash(source);
        let applied = store::applied_checksum(pool, version)
            .await
            .expect("migration status");
        if let Some(applied) = applied {
            assert_eq!(
                applied, checksum,
                "Baseline {version} changed after this database was set up; set up a new database"
            );
            continue;
        }
        let mut tx = pool.begin().await.expect("migration transaction");
        store::run_script(&mut tx, source)
            .await
            .unwrap_or_else(|e| panic!("Migration {version}: {}", e.1));
        store::record(&mut tx, version, &checksum)
            .await
            .expect("record migration");
        tx.commit().await.expect("commit migration");
    }
}

pub(crate) async fn seed_demo(a: &App, setup: &PgPool) {
    let ready = store::demo_seeded(setup).await.expect("demo seed status");
    if ready {
        return;
    }
    for t in ["atelier", "workshop"] {
        shops::ensure_settings(
            &a.db,
            t,
            serde_json::from_str::<Value>(include_str!("../fixtures/demo-settings.json")).unwrap(),
        )
        .await
        .expect("demo settings basis");
        shops::seed_automation(&a.db, t)
            .await
            .expect("demo channel before customer creation");
    }
    seed(a).await.expect("demo seed");
    for source in [
        include_str!("../fixtures/demo/01-merchandising.sql"),
        include_str!("../fixtures/demo/02-context.sql"),
        include_str!("../fixtures/demo/03-commerce.sql"),
        include_str!("../fixtures/demo/04-variant-properties.sql"),
    ] {
        store::run_fixture(&a.db, source)
            .await
            .expect("demo metadata");
    }
    let ids = ["chair", "desk", "lamp", "mug", "notebook", "shelf"].map(String::from);
    for t in ["atelier", "workshop"] {
        let default = include_str!("../extensions/company-limit.wat");
        shops::insert_extension(&a.db, t, default, &hash(default), true)
            .await
            .expect("default extension");
        let products = veyndo_catalog::store::products::main_by_ids(&a.core, t, &ids)
            .await
            .expect("bounded demo products");
        for p in products {
            knowledge::sync_product_scoped(&a.db, t, &json!(p))
                .await
                .expect("demo graph product");
        }
        knowledge::seed_relations(&a.db, t)
            .await
            .expect("demo graph relations");
    }
    store::record_demo_seed(setup)
        .await
        .expect("record completed seed");
}

/// Every baseline must be recorded with the checksum of the shipped file.
pub(crate) async fn ready(db: &veyndo::scoped_pool::ScopedPool) {
    for (version, source) in schema::BASELINES {
        let current = store::applied_checksum(db, version)
            .await
            .expect("Run BOOTSTRAP_MODE=migrate before serving");
        assert!(
            current.as_deref() == Some(hash(source).as_str()),
            "Schema not ready: {version}; run BOOTSTRAP_MODE=migrate"
        );
    }
}
