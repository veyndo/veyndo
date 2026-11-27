//! Schema setup, separate from serving: applies the component baselines and then the forward
//! migrations in their global order under one advisory lock, and records each one with its checksum.
//! An applied file that changed, or a pending migration that sorts before an applied one, is refused.
//! A forward migration runs in one transaction unless its first line is [`NO_TRANSACTION`]; then each
//! [`STEP`]-separated part commits on its own and the migration is recorded after the last part.
//! A database that the pre-baseline release set up with incremental migrations is refused.
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

/// The first migration every database of the pre-baseline incremental release recorded.
const PRE_BASELINE: &str = "001";
/// First line of a migration whose steps must commit separately (batched backfills, concurrent indexes).
const NO_TRANSACTION: &str = "-- veyndo:no-transaction";
/// A line that ends one separately committed step of a [`NO_TRANSACTION`] migration.
const STEP: &str = "-- veyndo:step";

/// The separately committed steps of a migration, or `None` when it runs in one transaction.
fn steps(source: &'static str) -> Option<Vec<&'static str>> {
    let body = source.strip_prefix(NO_TRANSACTION)?;
    Some(
        body.split(&format!("\n{STEP}\n"))
            .map(str::trim)
            .filter(|step| !step.is_empty())
            .collect(),
    )
}

/// The migrations still to apply, in order; refused when an applied file changed or when a pending
/// migration sorts before an applied one (its number must be raised above the applied migrations).
fn pending(applied: &[Option<String>]) -> Vec<(&'static str, &'static str)> {
    let last_applied = applied.iter().rposition(Option::is_some);
    let mut out = Vec::new();
    for (index, ((version, source), recorded)) in schema::MIGRATIONS.iter().zip(applied).enumerate()
    {
        match recorded {
            Some(recorded) => assert_eq!(
                recorded,
                &hash(source),
                "Migration {version} changed after it was applied; restore it and add a new migration instead"
            ),
            None => {
                if let Some(last) = last_applied.filter(|last| *last > index) {
                    panic!(
                        "Migration {version} sorts before the applied {}; give it a number above every applied migration",
                        schema::MIGRATIONS[last].0
                    );
                }
                out.push((*version, *source));
            }
        }
    }
    out
}

pub(crate) async fn apply(pool: &PgPool) {
    store::enter_trusted(pool)
        .await
        .expect("trusted migration context");
    // The dedicated setup pool has exactly one connection, retaining this
    // session lock through schema setup and the bounded demo seed.
    store::lock(pool).await.expect("migration lock");
    store::ensure_ledger(pool).await.expect("migration ledger");
    let earlier = store::applied_checksum(pool, PRE_BASELINE)
        .await
        .expect("migration status");
    assert!(
        earlier.is_none(),
        "This database was set up by the pre-baseline release; there is no upgrade path from it. Set up a new database"
    );
    let mut applied = Vec::with_capacity(schema::MIGRATIONS.len());
    for (version, _) in schema::MIGRATIONS {
        applied.push(
            store::applied_checksum(pool, version)
                .await
                .expect("migration status"),
        );
    }
    for (version, source) in pending(&applied) {
        let checksum = hash(source);
        if let Some(steps) = steps(source) {
            // The one setup connection (holding the lock) runs every step, then records the migration.
            let mut conn = pool.acquire().await.expect("migration connection");
            for step in steps {
                store::run_step(&mut conn, step)
                    .await
                    .unwrap_or_else(|e| panic!("Migration {version}: {}", e.1));
            }
            drop(conn);
            let mut tx = pool.begin().await.expect("migration transaction");
            store::record(&mut tx, version, &checksum)
                .await
                .expect("record migration");
            tx.commit().await.expect("commit migration");
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

/// Every migration this binary ships must be recorded with the checksum of the shipped file. Migrations
/// a newer release recorded are not this binary's concern: expand/contract keeps them compatible.
pub(crate) async fn ready(db: &veyndo::scoped_pool::ScopedPool) {
    for (version, source) in schema::MIGRATIONS {
        let current = store::applied_checksum(db, version)
            .await
            .expect("Run BOOTSTRAP_MODE=migrate before serving");
        assert!(
            current.as_deref() == Some(hash(source).as_str()),
            "Schema not ready: {version}; run BOOTSTRAP_MODE=migrate"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_marked_migration_splits_into_its_steps() {
        let source = "-- veyndo:no-transaction\nALTER TABLE a ADD b int;\n-- veyndo:step\nDO $$ BEGIN COMMIT; END $$;\n";
        assert_eq!(
            steps(source).unwrap(),
            ["ALTER TABLE a ADD b int;", "DO $$ BEGIN COMMIT; END $$;"]
        );
        assert!(steps("ALTER TABLE a ADD b int;").is_none());
    }

    #[test]
    fn every_shipped_migration_is_pending_on_an_empty_database() {
        let none = vec![None; schema::MIGRATIONS.len()];
        assert_eq!(pending(&none).len(), schema::MIGRATIONS.len());
        let all: Vec<_> = schema::MIGRATIONS
            .iter()
            .map(|(_, s)| Some(hash(s)))
            .collect();
        assert!(pending(&all).is_empty());
    }

    #[test]
    #[should_panic(expected = "changed after it was applied")]
    fn a_changed_applied_file_is_refused() {
        let mut all: Vec<_> = schema::MIGRATIONS
            .iter()
            .map(|(_, s)| Some(hash(s)))
            .collect();
        all[0] = Some("edited".into());
        pending(&all);
    }

    #[test]
    #[should_panic(expected = "give it a number above every applied migration")]
    fn a_pending_migration_before_an_applied_one_is_refused() {
        let mut all: Vec<_> = schema::MIGRATIONS
            .iter()
            .map(|(_, s)| Some(hash(s)))
            .collect();
        all[1] = None;
        pending(&all);
    }
}
