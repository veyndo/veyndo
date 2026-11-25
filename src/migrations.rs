//! Versioned setup is separate from serving; no catalog-wide startup repair.
use crate::apps;
use crate::foundation::App;
use crate::seed::seed;
use crate::store::migrations as store;
use crate::store::shops;
use serde_json::Value;
use serde_json::json;
use sqlx::PgPool;
use veyndo_core::hash;
use veyndo_intelligence::knowledge;

mod identity_checksums;
mod recent_schema;
mod schema;
fn sources() -> impl Iterator<Item = &'static (&'static str, &'static str)> {
    schema::SCHEMA.iter().chain(recent_schema::SCHEMA.iter())
}

pub(crate) async fn apply(pool: &PgPool) {
    store::enter_trusted(pool)
        .await
        .expect("trusted migration context");
    // The dedicated setup pool has exactly one connection, retaining this
    // session lock through schema setup and the bounded demo seed.
    store::lock(pool).await.expect("migration lock");
    store::ensure_ledger(pool).await.expect("migration ledger");
    // Historical AGE databases keep their original checksum contract; fresh installs use managed variants.
    for (version, source) in [
        ("002", include_str!("../migrations/002.sql")),
        (
            "011-documents",
            include_str!("../migrations/011-documents.sql"),
        ),
    ] {
        let applied = store::applied_checksum(pool, version)
            .await
            .expect("legacy migration ledger");
        if let Some(applied) = applied {
            assert_eq!(applied, hash(source), "Legacy migration drift: {version}");
        }
    }
    for (version, source) in sources() {
        let checksum = hash(source);
        let applied = store::applied_checksum(pool, version)
            .await
            .expect("migration status");
        if let Some(applied) = applied {
            assert!(
                identity_checksums::matches(version, source, &applied),
                "Applied migration changed: {version}"
            );
            continue;
        }
        let mut tx = pool.begin().await.expect("migration transaction");
        store::run_script(&mut tx, source)
            .await
            .unwrap_or_else(|e| panic!("Migration {version}: {}", e.1));
        if *version == "102-consolidate-app-tables" {
            apps::hosted::backfill(&mut tx)
                .await
                .expect("hosted app migration");
        }
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
        include_str!("../migrations/003-seed.sql"),
        include_str!("../migrations/005-context-seed.sql"),
        include_str!("../migrations/007-commerce-seed.sql"),
        include_str!("../migrations/009-variant-properties.sql"),
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

/// Every required schema must match, including domain schemas introduced after bounded reads.
pub(crate) async fn ready(db: &veyndo::scoped_pool::ScopedPool) {
    for (version, source) in sources() {
        let current = store::applied_checksum(db, version)
            .await
            .expect("Run BOOTSTRAP_MODE=migrate before serving");
        assert!(
            current
                .as_deref()
                .is_some_and(|applied| identity_checksums::matches(version, source, applied)),
            "Schema not ready: {version}; run BOOTSTRAP_MODE=migrate"
        );
    }
}
