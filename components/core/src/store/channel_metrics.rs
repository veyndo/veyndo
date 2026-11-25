//! The persisted channel diagnostics: one bounded bulk add per flush interval, and the traffic read shared by operator overview, shop dossiers and infrastructure.
use crate::Result;
use serde_json::{Value, json};
use sqlx::Row;
use veyndo_runtime::scoped_pool::ScopedPool;

/// One flush interval's counters, one entry per (tenant, channel).
pub struct Totals {
    pub tenants: Vec<String>,
    pub channels: Vec<&'static str>,
    pub calls: Vec<i64>,
    pub failures: Vec<i64>,
    pub total_ms: Vec<i64>,
    pub max_ms: Vec<i64>,
    pub responses: Vec<Value>,
}

/// Adds a flush interval's counters to the persisted totals.
pub async fn add(db: &ScopedPool, t: Totals) -> Result<()> {
    sqlx::query("INSERT INTO channel_metrics(tenant,channel,calls,failures,total_ms,max_ms,response_counts,timed_calls) SELECT *,c FROM unnest($1::text[],$2::text[],$3::bigint[],$4::bigint[],$5::bigint[],$6::bigint[],$7::jsonb[]) AS v(t,ch,c,f,ms,mx,r) ON CONFLICT(tenant,channel) DO UPDATE SET calls=channel_metrics.calls+EXCLUDED.calls,failures=channel_metrics.failures+EXCLUDED.failures,timed_calls=channel_metrics.timed_calls+EXCLUDED.timed_calls,total_ms=channel_metrics.total_ms+EXCLUDED.total_ms,max_ms=greatest(channel_metrics.max_ms,EXCLUDED.max_ms),response_counts=veyndo_merge_http_counts(channel_metrics.response_counts,EXCLUDED.response_counts),last_seen=now()")
        .bind(t.tenants).bind(t.channels).bind(t.calls).bind(t.failures).bind(t.total_ms).bind(t.max_ms).bind(t.responses).execute(db).await?;
    Ok(())
}

pub async fn traffic(
    db: &ScopedPool,
    tenant: Option<&str>,
    exclude_staging: bool,
) -> Result<Vec<Value>> {
    // Status codes are a fixed, bounded vocabulary. Legacy totals are deliberately not guessed.
    let rows = sqlx::query("WITH scoped AS (SELECT * FROM channel_metrics m WHERE ($1::text IS NULL OR m.tenant=$1) AND (NOT $2 OR NOT EXISTS(SELECT 1 FROM shop_environments e WHERE e.tenant=m.tenant))), statuses AS (SELECT channel,key,sum(value::bigint) AS n FROM scoped CROSS JOIN LATERAL jsonb_each_text(response_counts) GROUP BY channel,key), codes AS (SELECT channel,jsonb_object_agg(key,n) AS responses FROM statuses GROUP BY channel) SELECT s.channel,sum(calls)::bigint AS calls,sum(failures)::bigint AS failures,sum(total_ms)::bigint AS total_ms,max(max_ms) AS max_ms,sum(timed_calls)::bigint AS timed_calls,max(last_seen)::text AS seen,coalesce(c.responses,'{}'::jsonb) AS responses FROM scoped s LEFT JOIN codes c USING(channel) GROUP BY s.channel,c.responses ORDER BY s.channel")
        .bind(tenant).bind(exclude_staging).fetch_all(db).await?;
    Ok(rows
        .iter()
        .map(|r| {
            json!({
                "channel":r.get::<String,_>("channel"), "calls":r.get::<i64,_>("calls"),
                "failures":r.get::<i64,_>("failures"), "timedCalls":r.get::<i64,_>("timed_calls"),
                "totalMs":r.get::<i64,_>("total_ms"), "maxMs":r.get::<i64,_>("max_ms"),
                "lastSeen":r.get::<String,_>("seen"), "responses":r.get::<Value,_>("responses")
            })
        })
        .collect())
}
