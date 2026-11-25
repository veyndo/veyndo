//! Reads of the outbox for its consumers: events after a cursor, events and kinds by ID, one event's data and recent submission counts.
use crate::Result;
use serde_json::Value;
use sqlx::{Executor, PgConnection, Postgres};

/// One outbox event.
pub struct Event {
    pub id: i64,
    pub kind: String,
    pub data: Value,
}

/// The shop's outbox events after a cursor, oldest first.
pub async fn after(conn: &mut PgConnection, t: &str, after: i64, limit: i64) -> Result<Vec<Event>> {
    Ok(sqlx::query_as!(
        Event,
        "SELECT id,kind,data FROM outbox WHERE tenant=$1 AND id>$2 ORDER BY id LIMIT $3",
        t,
        after,
        limit
    )
    .fetch_all(conn)
    .await?)
}

/// The kind of each of the shop's events among `ids`.
pub struct Kind {
    pub id: i64,
    pub kind: String,
}

/// The kinds of the shop's events among `ids`.
pub async fn kinds<'e>(
    db: impl Executor<'e, Database = Postgres>,
    t: &str,
    ids: &[i64],
) -> Result<Vec<Kind>> {
    Ok(sqlx::query_as!(
        Kind,
        "SELECT id,kind FROM outbox WHERE tenant=$1 AND id=ANY($2)",
        t,
        ids
    )
    .fetch_all(db)
    .await?)
}

/// The data of one event of the shop.
pub async fn data(conn: &mut PgConnection, t: &str, id: i64) -> Result<Value> {
    Ok(
        sqlx::query_scalar!("SELECT data FROM outbox WHERE tenant=$1 AND id=$2", t, id)
            .fetch_one(conn)
            .await?,
    )
}

/// An event with the trace that produced it.
pub struct Traced {
    pub id: i64,
    pub kind: String,
    pub data: Value,
    pub trace_id: String,
}

/// The shop's events among `ids` with their traces, oldest first.
pub async fn traced(conn: &mut PgConnection, t: &str, ids: &[i64]) -> Result<Vec<Traced>> {
    Ok(sqlx::query_as!(
        Traced,
        "SELECT id,kind,data,trace_id FROM outbox WHERE tenant=$1 AND id=ANY($2) ORDER BY id",
        t,
        ids
    )
    .fetch_all(conn)
    .await?)
}

/// How many events of a kind the shop recorded in the last minute.
pub async fn last_minute(conn: &mut PgConnection, t: &str, kind: &str) -> Result<i64> {
    Ok(sqlx::query_scalar!(
        r#"SELECT count(*) AS "count!" FROM outbox WHERE tenant=$1 AND kind=$2 AND created_at>now()-interval '1 minute'"#,
        t,
        kind
    )
    .fetch_one(conn)
    .await?)
}

/// How many events of a kind with this intake key the shop recorded in the last minute.
pub async fn last_minute_for_intake(
    conn: &mut PgConnection,
    t: &str,
    kind: &str,
    intake_key: &str,
) -> Result<i64> {
    Ok(sqlx::query_scalar!(
        r#"SELECT count(*) AS "count!" FROM outbox WHERE tenant=$1 AND kind=$2 AND data->>'intakeKey'=$3 AND created_at>now()-interval '1 minute'"#,
        t,
        kind,
        intake_key
    )
    .fetch_one(conn)
    .await?)
}
