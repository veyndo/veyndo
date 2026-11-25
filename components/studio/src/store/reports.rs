//! Shared native commerce reports for Studio, workspace, API and agent reads; no second analytics ledger.
use crate::domain::views::ReportScale;
use crate::domain::{report_config, report_period};
use serde_json::{Value, json};
use sqlx::{Postgres, Transaction};
use veyndo_core::document::view;
use veyndo_core::{Error, RequestContext, Result, State, StatusCode, bad};
use veyndo_identity::allowed;
use veyndo_pricing::types::currency::scale;
use veyndo_shop::domain::patch;
use veyndo_shop::types::legal::version;
use veyndo_shop::types::settings::Settings;

/// What the server resolves for a report: the shop's settings, the reader's language and the consent filter of behaviour events.
pub struct Context<'a> {
    pub settings: &'a Settings,
    pub locale: &'a str,
    pub chain: &'a [String],
    pub eligible: &'a str,
    pub conditions: &'a (dyn Fn(&Value) -> Result<()> + Sync),
}

/// Runs one report on a read snapshot; products and traffic appear only with the matching rights.
pub async fn run(
    a: &State,
    h: &RequestContext,
    t: &str,
    v: &Value,
    x: Context<'_>,
) -> Result<Value> {
    let q: report_config::Query =
        serde_json::from_value(v.clone()).map_err(|_| bad("Invalid report filters"))?;
    q.validate()?;
    let settings = x.settings;
    let currency = q
        .currency
        .as_deref()
        .unwrap_or(&settings.currencies.base_currency);
    let tz = q
        .timezone
        .parse::<chrono_tz::Tz>()
        .map_err(|_| bad("Invalid report timezone"))?;
    let earliest: Option<String> = sqlx::query_scalar!(
        "SELECT (min(created_at) AT TIME ZONE $2)::date::text FROM orders WHERE tenant=$1",
        t,
        &q.timezone
    )
    .fetch_one(&a.db)
    .await?;
    let earliest = earliest.and_then(|d| chrono::NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok());
    let p = report_period::resolve(
        &q,
        chrono::Utc::now().with_timezone(&tz).date_naive(),
        earliest,
    )?;
    let mut tx = a.db.begin_read_snapshot().await?;
    let sql = format!(
        "{}{}",
        include_str!("reports/source.sql"),
        include_str!("reports/sales.sql")
    );
    // dynamic SQL: the shared report source CTE and the sales fragment are two bundled files composed at runtime.
    let mut data: Value = sqlx::query_scalar(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(t)
        .bind(if q.compare { &p.previous } else { &p.from })
        .bind(&p.from)
        .bind(&p.to)
        .bind(&p.interval)
        .bind(&q.timezone)
        .bind(currency)
        .bind(&q.channel)
        .bind(&q.channel_type)
        .fetch_one(&mut *tx)
        .await?;
    let rows=sqlx::query_as!(ChannelRow, r#"SELECT s.id,s.data->'name' AS name,s.data->>'kind' AS kind,o.data AS "overrides?" FROM sales_channels s LEFT JOIN commerce_overrides o ON o.tenant=s.tenant AND o.channel_id=s.id WHERE s.tenant=$1 ORDER BY s.id LIMIT 501"#, t).fetch_all(&mut *tx).await?;
    if rows.len() > 500 {
        return Err(bad(
            "Report channel limit exceeded; select a smaller shop scope",
        ));
    }
    if q.channel
        .as_ref()
        .is_some_and(|id| !rows.iter().any(|r| r.id == *id))
    {
        return Err(Error(
            StatusCode::NOT_FOUND,
            "Report channel not found".into(),
        ));
    }
    let (locale, chain) = (x.locale, x.chain);
    let channels=rows.iter().map(|r| {
        let id=r.id.clone();let names=r.name.clone().unwrap_or_default();
        let name=chain.iter().find_map(|l|names[l].as_str()).or_else(||names[&locale].as_str()).or_else(||names[&settings.main_locale].as_str()).unwrap_or(&id);
        json!({"id":id,"name":if allowed(h,"channels.read"){name}else{&id},"kind":r.kind.clone().unwrap_or_else(||"storefront".into())})
    }).collect::<Vec<_>>();
    let mut currencies=sqlx::query_scalar!(r#"SELECT DISTINCT coalesce(data#>>'{money,currency,code}',data#>>'{cart,price,currency}','EUR') AS "currency!" FROM orders WHERE tenant=$1 ORDER BY 1 LIMIT 100"#, t).fetch_all(&mut *tx).await?;
    if !currencies.iter().any(|c| c == currency) {
        currencies.push(currency.to_string());
    }
    if !allowed(h, "catalog.read") {
        data["products"] = json!([]);
    }
    let traffic = if allowed(h, "knowledge.read") {
        traffic(&mut tx, t, &q, &p, currency, &rows, &x).await?
    } else {
        Value::Null
    };
    tx.commit().await?;
    data["traffic"] = traffic;
    let scale = view::<ReportScale>(&data)
        .currency_scale
        .unwrap_or(scale(currency).unwrap_or(2) as i64);
    data["filters"] = json!({"from":p.from,"toExclusive":p.to,"previousFrom":p.previous,"interval":p.interval,"timezone":q.timezone,"currency":currency,"currencyScale":scale,"channels":channels,"currencies":currencies});
    data["permissions"] =
        json!({"traffic":allowed(h,"knowledge.read"),"products":allowed(h,"catalog.read")});
    let live: Option<String> = sqlx::query_scalar!(
        "SELECT live_tenant FROM shop_environments WHERE tenant=$1",
        t
    )
    .fetch_optional(&a.db)
    .await?;
    data["sandbox"] = json!(live.is_some());
    Ok(data)
}

/// A sales channel with its name, kind and settings overrides.
struct ChannelRow {
    id: String,
    name: Option<Value>,
    kind: Option<String>,
    overrides: Option<Value>,
}

/// Traffic reuses the behaviour owner's receipt and expiry predicate under each channel's inherited policy; no new tracking store.
async fn traffic(
    tx: &mut Transaction<'_, Postgres>,
    t: &str,
    q: &report_config::Query,
    p: &report_period::Period,
    currency: &str,
    rows: &[ChannelRow],
    x: &Context<'_>,
) -> Result<Value> {
    let mut policies = vec![];
    for row in rows {
        let channel = row.id.clone();
        let kind = row.kind.clone();
        if q.channel.as_ref().is_some_and(|id| id != &channel)
            || q.channel_type != "all" && kind.as_deref() != Some(&q.channel_type)
        {
            continue;
        }
        let settings = patch::resolve(
            x.settings,
            row.overrides.clone().unwrap_or(json!([])),
            x.conditions,
        )?;
        let policy = &settings.legal.behavior_analytics;
        let enabled = policy.enabled && settings.legal.purposes.get("analytics") == Some(&true);
        if enabled {
            policies.push(json!({"channel":channel,"days":policy.retention_days,"version":version(&settings.legal)}));
        }
    }
    // Only bundled SQL fragments are composed. The underlying native consent filter stays a single source.
    let eligible = x
        .eligible
        .replace(
            "FROM session_signal_events e",
            "FROM session_signal_events e JOIN policies p ON p.channel=e.channel_id",
        )
        .replace("e.channel_id=$2", "e.channel_id=p.channel")
        .replace("days=>$3", "days=>p.days")
        .replace("c.policy_version=$4", "c.policy_version=p.version");
    let sql = format!(
        "{}, policies AS (SELECT * FROM jsonb_to_recordset($10::jsonb) AS p(channel text,days int,version text)), events AS ({eligible}) {}",
        include_str!("reports/source.sql"),
        include_str!("reports/traffic.sql")
    );
    // dynamic SQL: bundled fragments around the behaviour owner's consent filter, rewritten per channel policy.
    let value: Value = sqlx::query_scalar(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(t)
        .bind(if q.compare { &p.previous } else { &p.from })
        .bind(&p.from)
        .bind(&p.to)
        .bind(&p.interval)
        .bind(&q.timezone)
        .bind(currency)
        .bind(&q.channel)
        .bind(&q.channel_type)
        .bind(json!(policies))
        .fetch_one(&mut **tx)
        .await?;
    Ok(value)
}
