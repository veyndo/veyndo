//! Release provisioning SQL on the migration-owner connection: role lookups, the grant statements for the runtime group and the connector login, and the privilege coverage that verifies them. Statements are generated server-side with `format('%I')`, so role and table names are always quoted identifiers.
use sqlx::PgPool;
/// Provisioning is an offline command: it reports the database's own error text.
type Result<T> = std::result::Result<T, sqlx::Error>;

/// Effective privileges of one role over the objects it should reach.
pub(crate) struct Coverage {
    pub(crate) tables: i64,
    pub(crate) missing_tables: i64,
    pub(crate) sequences: i64,
    pub(crate) missing_sequences: i64,
    pub(crate) connect: bool,
    pub(crate) schema_usage: bool,
}

/// What the configured database authority leaves to verify after configuration.
pub(crate) struct AuthorityCoverage {
    pub(crate) tenant_tables: i64,
    pub(crate) tenant_missing: i64,
    pub(crate) tenant_temporary: bool,
}

pub(crate) async fn role_exists(db: &PgPool, role: &str) -> Result<bool> {
    sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM pg_roles WHERE rolname=$1) AS "exists!""#,
        role
    )
    .fetch_one(db)
    .await
}

/// Grants for the runtime group: DML on every Core table, reads of views and sequences, and no
/// access to connector tables, which belong to the connector service's own login.
pub(crate) async fn runtime_statements(db: &PgPool, role: &str) -> Result<Vec<String>> {
    sqlx::query_scalar!(
        r#"SELECT format(CASE WHEN c.relkind='S' THEN 'GRANT USAGE,SELECT ON SEQUENCE public.%I TO %I'
                    WHEN left(c.relname,10)='connector_' THEN 'REVOKE ALL ON public.%I FROM %I'
                    WHEN c.relkind IN ('v','m') THEN 'GRANT SELECT ON public.%I TO %I'
                    ELSE 'GRANT SELECT,INSERT,UPDATE,DELETE,REFERENCES ON public.%I TO %I' END,
                c.relname,$1::text) AS "statement!"
           FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace
           WHERE n.nspname='public' AND c.relkind IN ('r','p','v','m','S')
           ORDER BY c.relname"#,
        role
    )
    .fetch_all(db)
    .await
}

/// Grants for the connector login: DML on connector tables, its change sequence and a read of the
/// migration ledger (its readiness check). It never receives Core tables.
pub(crate) async fn connector_statements(db: &PgPool, login: &str) -> Result<Vec<String>> {
    sqlx::query_scalar!(
        r#"SELECT format(CASE WHEN c.relkind='S' THEN 'GRANT USAGE,SELECT,UPDATE ON SEQUENCE public.%I TO %I'
                    ELSE 'GRANT SELECT,INSERT,UPDATE,DELETE ON public.%I TO %I' END,
                c.relname,$1::text) AS "statement!"
           FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace
           WHERE n.nspname='public' AND c.relkind IN ('r','p','S') AND left(c.relname,10)='connector_'
           UNION ALL SELECT format('GRANT SELECT ON public.commerce_migrations TO %I',$1::text)"#,
        login
    )
    .fetch_all(db)
    .await
}

/// Applies generated statements in one transaction, so a release never leaves half-granted objects.
pub(crate) async fn execute(db: &PgPool, statements: &[String]) -> Result<()> {
    let mut tx = db.begin().await?;
    for statement in statements {
        // A GRANT is a utility statement and cannot be prepared. The text is safe: PostgreSQL
        // generated it with `format('%I')`, so every role and table name is a quoted identifier.
        sqlx::raw_sql(sqlx::AssertSqlSafe(statement.as_str()))
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}

/// Core coverage of a runtime role or system login: every non-connector table and view, and every sequence.
pub(crate) async fn runtime_coverage(db: &PgPool, role: &str) -> Result<Coverage> {
    let row = sqlx::query!(
        r#"SELECT
            count(*) FILTER (WHERE c.relkind<>'S') AS "tables!",
            count(*) FILTER (WHERE c.relkind IN ('v','m') AND NOT has_table_privilege($1::text,c.oid,'SELECT')
                OR c.relkind IN ('r','p') AND NOT (
                has_table_privilege($1::text,c.oid,'SELECT') AND has_table_privilege($1::text,c.oid,'INSERT')
                AND has_table_privilege($1::text,c.oid,'UPDATE') AND has_table_privilege($1::text,c.oid,'DELETE')
                AND has_table_privilege($1::text,c.oid,'REFERENCES'))) AS "missing_tables!",
            count(*) FILTER (WHERE c.relkind='S') AS "sequences!",
            count(*) FILTER (WHERE c.relkind='S' AND NOT (has_sequence_privilege($1::text,c.oid,'USAGE')
                AND has_sequence_privilege($1::text,c.oid,'SELECT'))) AS "missing_sequences!",
            has_database_privilege($1::text,current_database(),'CONNECT') AS "connect!",
            has_schema_privilege($1::text,'public','USAGE') AS "schema_usage!"
           FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace
           WHERE n.nspname='public' AND c.relkind IN ('r','p','v','m','S')
             AND (c.relkind='S' OR left(c.relname,10)<>'connector_')"#,
        role
    )
    .fetch_one(db)
    .await?;
    Ok(Coverage {
        tables: row.tables,
        missing_tables: row.missing_tables,
        sequences: row.sequences,
        missing_sequences: row.missing_sequences,
        connect: row.connect,
        schema_usage: row.schema_usage,
    })
}

/// Connector coverage: its tables and sequences plus the migration ledger read (counted as a table).
pub(crate) async fn connector_coverage(db: &PgPool, login: &str) -> Result<Coverage> {
    let row = sqlx::query!(
        r#"SELECT
            count(*) FILTER (WHERE c.relkind IN ('r','p')) + 1 AS "tables!",
            count(*) FILTER (WHERE c.relkind IN ('r','p') AND NOT (
                has_table_privilege($1::text,c.oid,'SELECT') AND has_table_privilege($1::text,c.oid,'INSERT')
                AND has_table_privilege($1::text,c.oid,'UPDATE') AND has_table_privilege($1::text,c.oid,'DELETE')))
              + CASE WHEN has_table_privilege($1::text,'public.commerce_migrations','SELECT') THEN 0 ELSE 1 END AS "missing_tables!",
            count(*) FILTER (WHERE c.relkind='S') AS "sequences!",
            count(*) FILTER (WHERE c.relkind='S' AND NOT (has_sequence_privilege($1::text,c.oid,'USAGE')
                AND has_sequence_privilege($1::text,c.oid,'UPDATE'))) AS "missing_sequences!",
            has_database_privilege($1::text,current_database(),'CONNECT') AS "connect!",
            has_schema_privilege($1::text,'public','USAGE') AS "schema_usage!"
           FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace
           WHERE n.nspname='public' AND c.relkind IN ('r','p','S') AND left(c.relname,10)='connector_'"#,
        login
    )
    .fetch_one(db)
    .await?;
    Ok(Coverage {
        tables: row.tables,
        missing_tables: row.missing_tables,
        sequences: row.sequences,
        missing_sequences: row.missing_sequences,
        connect: row.connect,
        schema_usage: row.schema_usage,
    })
}

/// The tenant login must reach every forced-RLS tenant table and must not create temporary objects.
pub(crate) async fn authority_coverage(db: &PgPool, tenant: &str) -> Result<AuthorityCoverage> {
    let row = sqlx::query!(
        r#"SELECT count(*) AS "tables!",
            count(*) FILTER (WHERE NOT (has_table_privilege($1::text,t.oid,'SELECT')
                AND has_table_privilege($1::text,t.oid,'INSERT') AND has_table_privilege($1::text,t.oid,'UPDATE')
                AND has_table_privilege($1::text,t.oid,'DELETE'))) AS "missing!",
            has_database_privilege($1::text,current_database(),'TEMPORARY') AS "temporary!"
           FROM pg_class t JOIN pg_namespace n ON n.oid=t.relnamespace
           WHERE n.nspname='public' AND t.relkind='r' AND t.relrowsecurity AND t.relforcerowsecurity
             AND left(t.relname,10)<>'connector_'
             AND EXISTS(SELECT 1 FROM pg_attribute a WHERE a.attrelid=t.oid AND a.attname='tenant' AND NOT a.attisdropped)"#,
        tenant
    )
    .fetch_one(db)
    .await?;
    Ok(AuthorityCoverage {
        tenant_tables: row.tables,
        tenant_missing: row.missing,
        tenant_temporary: row.temporary,
    })
}
