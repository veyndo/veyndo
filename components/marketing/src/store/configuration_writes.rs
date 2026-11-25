//! Rows of the four configuration kinds (rules, promotions, flows, channels): listing, locked revisions, revisioned upserts and deletion, and the settings and promotion reads their saves check against. Each kind's table has its own checked query.
use serde_json::Value;
use sqlx::{PgConnection, Postgres, Transaction};
use veyndo_core::{Result, State, bad, conflict};

/// One definition with its document and revision.
pub struct ConfigurationRow {
    pub id: String,
    pub data: Value,
    pub revision: i64,
}

/// The first hundred definitions of one kind by id; a rule's document is built from its columns.
pub async fn list(a: &State, tenant: &str, kind: &str) -> Result<Vec<ConfigurationRow>> {
    let rows = match kind {
        "rules" => sqlx::query_as!(ConfigurationRow, r#"SELECT id,jsonb_build_object('name',name,'condition',condition,'active',active) AS "data!",revision FROM commerce_rules WHERE tenant=$1 ORDER BY id LIMIT 100"#, tenant).fetch_all(&a.db).await?,
        "promotions" => sqlx::query_as!(ConfigurationRow, "SELECT id,data,revision FROM commerce_promotions WHERE tenant=$1 ORDER BY id LIMIT 100", tenant).fetch_all(&a.db).await?,
        "flows" => sqlx::query_as!(ConfigurationRow, "SELECT id,data,revision FROM commerce_flows WHERE tenant=$1 ORDER BY id LIMIT 100", tenant).fetch_all(&a.db).await?,
        "channels" => sqlx::query_as!(ConfigurationRow, "SELECT id,data,revision FROM sales_channels WHERE tenant=$1 ORDER BY id LIMIT 100", tenant).fetch_all(&a.db).await?,
        _ => return Err(bad("Unknown configuration type")),
    };
    Ok(rows)
}

/// The revision of one definition, `FOR UPDATE` when `locked`.
pub async fn revision(
    conn: &mut PgConnection,
    tenant: &str,
    kind: &str,
    id: &str,
    locked: bool,
) -> Result<Option<i64>> {
    let revision = match (kind, locked) {
        ("rules", false) => {
            sqlx::query_scalar!(
                "SELECT revision FROM commerce_rules WHERE tenant=$1 AND id=$2",
                tenant,
                id
            )
            .fetch_optional(conn)
            .await?
        }
        ("rules", true) => {
            sqlx::query_scalar!(
                "SELECT revision FROM commerce_rules WHERE tenant=$1 AND id=$2 FOR UPDATE",
                tenant,
                id
            )
            .fetch_optional(conn)
            .await?
        }
        ("promotions", false) => {
            sqlx::query_scalar!(
                "SELECT revision FROM commerce_promotions WHERE tenant=$1 AND id=$2",
                tenant,
                id
            )
            .fetch_optional(conn)
            .await?
        }
        ("promotions", true) => {
            sqlx::query_scalar!(
                "SELECT revision FROM commerce_promotions WHERE tenant=$1 AND id=$2 FOR UPDATE",
                tenant,
                id
            )
            .fetch_optional(conn)
            .await?
        }
        ("flows", false) => {
            sqlx::query_scalar!(
                "SELECT revision FROM commerce_flows WHERE tenant=$1 AND id=$2",
                tenant,
                id
            )
            .fetch_optional(conn)
            .await?
        }
        ("flows", true) => {
            sqlx::query_scalar!(
                "SELECT revision FROM commerce_flows WHERE tenant=$1 AND id=$2 FOR UPDATE",
                tenant,
                id
            )
            .fetch_optional(conn)
            .await?
        }
        ("channels", false) => {
            sqlx::query_scalar!(
                "SELECT revision FROM sales_channels WHERE tenant=$1 AND id=$2",
                tenant,
                id
            )
            .fetch_optional(conn)
            .await?
        }
        ("channels", true) => {
            sqlx::query_scalar!(
                "SELECT revision FROM sales_channels WHERE tenant=$1 AND id=$2 FOR UPDATE",
                tenant,
                id
            )
            .fetch_optional(conn)
            .await?
        }
        _ => return Err(bad("Unknown configuration type")),
    };
    Ok(revision)
}

/// Takes the transaction advisory lock `key` (namespace 17, shared with the shop's automation lock).
pub async fn advisory_lock(conn: &mut PgConnection, key: &str) -> Result<()> {
    sqlx::query!("SELECT pg_advisory_xact_lock(hashtextextended($1,17))", key)
        .execute(conn)
        .await?;
    Ok(())
}

/// The shop settings document, `FOR UPDATE` when `locked`.
pub async fn settings(conn: &mut PgConnection, tenant: &str, locked: bool) -> Result<Value> {
    Ok(if locked {
        sqlx::query_scalar!(
            "SELECT data FROM commerce_settings WHERE tenant=$1 FOR UPDATE",
            tenant
        )
        .fetch_one(conn)
        .await?
    } else {
        sqlx::query_scalar!("SELECT data FROM commerce_settings WHERE tenant=$1", tenant)
            .fetch_one(conn)
            .await?
    })
}

/// Locks the shop settings row so a channel save and checkout see the same settings.
pub async fn lock_settings(conn: &mut PgConnection, tenant: &str) -> Result<()> {
    sqlx::query_scalar!(
        "SELECT revision FROM commerce_settings WHERE tenant=$1 FOR UPDATE",
        tenant
    )
    .fetch_one(conn)
    .await?;
    Ok(())
}

/// Whether PostgreSQL reads `date` as a timestamp.
pub async fn timestamp(a: &State, date: &str) -> Result<bool> {
    Ok(sqlx::query_scalar!(
        r#"SELECT $1::text::timestamptz IS NOT NULL AS "valid!""#,
        date
    )
    .fetch_one(&a.db)
    .await?)
}

/// Whether `start` is not after `end`, both read as timestamps.
pub async fn ordered(a: &State, start: Option<&str>, end: Option<&str>) -> Result<bool> {
    Ok(sqlx::query_scalar!(
        r#"SELECT $1::text::timestamptz <= $2::text::timestamptz AS "ordered!""#,
        start,
        end
    )
    .fetch_one(&a.db)
    .await?)
}

/// The stored document of one promotion.
pub async fn promotion<'e>(
    conn: impl sqlx::PgExecutor<'e>,
    tenant: &str,
    id: &str,
) -> Result<Option<Value>> {
    Ok(sqlx::query_scalar!(
        "SELECT data FROM commerce_promotions WHERE tenant=$1 AND id=$2",
        tenant,
        id
    )
    .fetch_optional(conn)
    .await?)
}

/// Whether another promotion of the shop already uses `code`.
pub async fn code_taken(
    conn: &mut PgConnection,
    tenant: &str,
    id: &str,
    code: Option<&str>,
) -> Result<bool> {
    Ok(sqlx::query_scalar!(r#"SELECT EXISTS(SELECT 1 FROM commerce_promotions WHERE tenant=$1 AND id<>$2 AND data->>'code'=$3) AS "taken!""#, tenant, id, code)
        .fetch_one(conn)
        .await?)
}

/// The columns of a rule.
pub struct RuleColumns<'a> {
    pub name: &'a Value,
    pub condition: &'a Value,
    pub active: Option<bool>,
}

/// Inserts or revises a rule from its columns.
pub async fn write_rule(
    tx: &mut Transaction<'_, Postgres>,
    tenant: &str,
    id: &str,
    rule: RuleColumns<'_>,
) -> Result<()> {
    sqlx::query!("INSERT INTO commerce_rules(tenant,id,name,condition,active) VALUES($1,$2,$3,$4,$5) ON CONFLICT(tenant,id) DO UPDATE SET name=EXCLUDED.name,condition=EXCLUDED.condition,active=EXCLUDED.active,revision=commerce_rules.revision+1",
        tenant, id, rule.name, rule.condition, rule.active as _)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Inserts or revises a promotion, flow or channel document.
pub async fn write_document(
    tx: &mut Transaction<'_, Postgres>,
    tenant: &str,
    kind: &str,
    id: &str,
    data: &Value,
) -> Result<()> {
    match kind {
        "promotions" => sqlx::query!("INSERT INTO commerce_promotions(tenant,id,data) VALUES($1,$2,$3) ON CONFLICT(tenant,id) DO UPDATE SET data=EXCLUDED.data,revision=commerce_promotions.revision+1", tenant, id, data).execute(&mut **tx).await?,
        "flows" => sqlx::query!("INSERT INTO commerce_flows(tenant,id,data) VALUES($1,$2,$3) ON CONFLICT(tenant,id) DO UPDATE SET data=EXCLUDED.data,revision=commerce_flows.revision+1", tenant, id, data).execute(&mut **tx).await?,
        "channels" => sqlx::query!("INSERT INTO sales_channels(tenant,id,data) VALUES($1,$2,$3) ON CONFLICT(tenant,id) DO UPDATE SET data=EXCLUDED.data,revision=sales_channels.revision+1", tenant, id, data).execute(&mut **tx).await?,
        _ => return Err(bad("Unknown configuration type")),
    };
    Ok(())
}

/// Deletes a definition at `revision`; a reference that appeared meanwhile turns the foreign-key failure into a conflict.
pub async fn delete(
    tx: &mut Transaction<'_, Postgres>,
    tenant: &str,
    kind: &str,
    id: &str,
    revision: i64,
) -> Result<()> {
    let deleted = match kind {
        "rules" => {
            sqlx::query!(
                "DELETE FROM commerce_rules WHERE tenant=$1 AND id=$2 AND revision=$3",
                tenant,
                id,
                revision
            )
            .execute(&mut **tx)
            .await
        }
        "promotions" => {
            sqlx::query!(
                "DELETE FROM commerce_promotions WHERE tenant=$1 AND id=$2 AND revision=$3",
                tenant,
                id,
                revision
            )
            .execute(&mut **tx)
            .await
        }
        "flows" => {
            sqlx::query!(
                "DELETE FROM commerce_flows WHERE tenant=$1 AND id=$2 AND revision=$3",
                tenant,
                id,
                revision
            )
            .execute(&mut **tx)
            .await
        }
        "channels" => {
            sqlx::query!(
                "DELETE FROM sales_channels WHERE tenant=$1 AND id=$2 AND revision=$3",
                tenant,
                id,
                revision
            )
            .execute(&mut **tx)
            .await
        }
        _ => return Err(bad("Unknown configuration type")),
    };
    deleted.map_err(|e| match &e {
        sqlx::Error::Database(db) if db.code().as_deref() == Some("23503") => {
            conflict("Configuration gained a dependency; reload and deactivate it instead")
        }
        _ => e.into(),
    })?;
    Ok(())
}
