//! Consent-bound behaviour observations: the bounded write, a shopper's export and erasure, the channel report with its derived graph and the private journey.
use serde_json::{Value, json};
use sqlx::{Postgres, Row, Transaction};
use veyndo_core::{Error, Result, State, StatusCode, conflict, hash};

/// Observations that are still covered by the current consent receipt, retention and channel.
pub const ELIGIBLE: &str = include_str!("behavior_eligible.sql");

/// One admitted observation and the limits of its sales channel.
pub struct Observation<'a> {
    pub tenant: &'a str,
    pub cart: &'a str,
    pub channel: &'a str,
    pub id: &'a str,
    pub kind: &'a str,
    pub product: Option<&'a str>,
    pub customer: Option<String>,
    pub dimensions: Value,
    pub events_per_hour: i64,
    pub retention_days: i32,
}

/// Stores the observation once; a repeated identifier must describe the same observation.
pub async fn record(tx: &mut Transaction<'_, Postgres>, e: Observation<'_>) -> Result<Value> {
    let dimensions = &e.dimensions;
    let session = hash(e.cart);
    if let Some(old) = sqlx::query!("SELECT session,purpose,kind,product_id,dimensions FROM session_signal_events WHERE tenant=$1 AND id=$2",
        e.tenant, e.id).fetch_optional(&mut **tx).await? {
        if old.session != session || old.purpose != "analytics" || old.kind != e.kind || old.product_id.as_deref() != e.product || old.dimensions != *dimensions {
            return Err(conflict("Behavioral event ID already belongs to a different observation"));
        }
        return Ok(json!({"stored":false,"duplicate":true}));
    }
    let count = sqlx::query_scalar!(r#"SELECT count(*) AS "count!" FROM session_signal_events WHERE tenant=$1 AND session=$2 AND purpose='analytics' AND created_at>now()-interval '1 hour'"#,
        e.tenant, session).fetch_one(&mut **tx).await?;
    if count >= e.events_per_hour {
        return Err(Error(
            StatusCode::TOO_MANY_REQUESTS,
            "Behavioral limit reached; retry after the hourly window".into(),
        ));
    }
    sqlx::query!("INSERT INTO session_signal_events(tenant,id,session,purpose,kind,cart_id,channel_id,product_id,customer_id,dimensions,expires_at) VALUES($1,$2,$3,'analytics',$4,$5,$6,$7,$8,$9,now()+make_interval(days=>$10))",
        e.tenant, e.id, session, e.kind, e.cart, e.channel, e.product, e.customer, dimensions, e.retention_days).execute(&mut **tx).await?;
    Ok(json!({"stored":true,"retentionDays":e.retention_days,"purpose":"analytics"}))
}

/// Erases the observations of one cart; serialized with capture through the cart lock.
pub async fn forget(a: &State, tenant: &str, cart: &str) -> Result<u64> {
    let mut tx = a.db.begin().await?;
    sqlx::query_scalar!(
        "SELECT id FROM carts WHERE tenant=$1 AND id=$2 FOR UPDATE",
        tenant,
        cart
    )
    .fetch_one(&mut *tx)
    .await?;
    let n = sqlx::query!(
        "DELETE FROM session_signal_events WHERE tenant=$1 AND session=$2 AND purpose='analytics'",
        tenant,
        hash(cart)
    )
    .execute(&mut *tx)
    .await?
    .rows_affected();
    tx.commit().await?;
    Ok(n)
}

/// What a shopper may see of their own observations.
pub async fn export(a: &State, tenant: &str, cart: &str) -> Result<Value> {
    let rows=sqlx::query!(r#"SELECT id,kind,product_id,dimensions,created_at::text AS "time!" FROM session_signal_events WHERE tenant=$1 AND session=$2 AND purpose='analytics' AND expires_at>now() ORDER BY created_at DESC,id LIMIT 101"#,
        tenant, hash(cart)).fetch_all(&a.db).await?;
    Ok(
        json!({"events":rows.iter().take(100).map(|r|json!({"id":r.id,"kind":r.kind,"productId":r.product_id,"dimensions":r.dimensions,"time":r.time})).collect::<Vec<_>>(),"truncated":rows.len()>100}),
    )
}

/// The sales channel of the latest observation of a session.
pub async fn journey_channel(a: &State, t: &str, session: &str) -> Result<String> {
    let channel = sqlx::query_scalar!(r#"SELECT channel_id AS "channel_id!" FROM session_signal_events WHERE tenant=$1 AND session=$2 AND purpose='analytics' AND expires_at>now() ORDER BY created_at DESC LIMIT 1"#,
        t, session).fetch_optional(&a.db).await?.ok_or(Error(StatusCode::NOT_FOUND,"Behavioral journey not found".into()))?;
    Ok(channel)
}

/// One session under the current policy of its channel, and what the reader may open from it.
pub struct Journey<'a> {
    pub session: &'a str,
    pub channel: &'a str,
    pub retention_days: i32,
    pub version: String,
    pub customers: bool,
    pub orders: bool,
}

// SQL composition uses only bundled static clauses; every request value remains a bind parameter.
/// The bounded event sequence of one session with its order, if any.
pub async fn journey(a: &State, t: &str, j: Journey<'_>) -> Result<Value> {
    let q = format!(
        "WITH events AS ({}) SELECT e.id,e.kind,e.product_id,e.customer_id,e.dimensions,e.created_at::text AS time,coalesce(c.email,o.data->'orderCustomer'->>'email') AS customer_ref,o.id AS order_id,o.data->>'orderNumber' AS order_number FROM events e LEFT JOIN orders o ON o.tenant=e.tenant AND o.cart_id=e.cart_id LEFT JOIN customers c ON c.tenant=e.tenant AND c.id=e.customer_id WHERE e.session=$5 ORDER BY e.created_at,e.id LIMIT 101",
        ELIGIBLE
    );
    // dynamic SQL: composed with the shared ELIGIBLE consent clause, which studio reports reuse.
    let rows = sqlx::query(sqlx::AssertSqlSafe(q.as_str()))
        .bind(t)
        .bind(j.channel)
        .bind(j.retention_days)
        .bind(j.version)
        .bind(j.session)
        .fetch_all(&a.db)
        .await?;
    if rows.is_empty() {
        return Err(Error(
            StatusCode::NOT_FOUND,
            "Behavioral journey not found".into(),
        ));
    }
    Ok(
        json!({"session":j.session,"salesChannelId":j.channel,"truncated":rows.len()>100,"canOpenCustomers":j.customers,"canOpenOrders":j.orders,"events":rows.iter().take(100).map(|r|json!({"id":r.get::<String,_>("id"),"kind":r.get::<String,_>("kind"),"productId":r.get::<Option<String>,_>("product_id"),"customerId":r.get::<Option<String>,_>("customer_id"),"customerRef":if j.customers{r.get::<Option<String>,_>("customer_ref")}else{None},"dimensions":r.get::<Value,_>("dimensions"),"time":r.get::<String,_>("time"),"orderId":if j.orders{r.get::<Option<String>,_>("order_id")}else{None},"orderNumber":if j.orders{r.get::<Option<String>,_>("order_number")}else{None}})).collect::<Vec<_>>()}),
    )
}

/// A channel report request under the channel's current policy.
pub struct Report<'a> {
    pub channel: &'a str,
    pub days: i32,
    pub version: &'a str,
    pub locale: &'a str,
    pub chain: &'a [String],
    pub main_locale: &'a str,
    pub retention_days: u32,
    pub enabled: bool,
    pub events_per_hour: u32,
    pub pii: bool,
    pub geo_configured: bool,
}

// SQL composition uses only bundled static clauses; every request value remains a bind parameter.
/// Indexed channel report and a derived semantic graph; observations never imply causation or model learning.
pub async fn report(a: &State, t: &str, r: Report<'_>) -> Result<Value> {
    let exists = sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM sales_channels WHERE tenant=$1 AND id=$2) AS "exists!""#,
        t,
        r.channel
    )
    .fetch_one(&a.db)
    .await?;
    if !exists {
        return Err(Error(
            StatusCode::NOT_FOUND,
            "Sales channel not found".into(),
        ));
    }
    let (channel, days, version, locale, chain, main) = (
        r.channel,
        r.days,
        r.version,
        r.locale,
        r.chain,
        r.main_locale,
    );
    let mut tx = a.db.begin_read_snapshot().await?;
    let prefix = format!("WITH events AS ({}) ", ELIGIBLE);
    let q = format!(
        "{prefix} SELECT count(*)::bigint AS events,count(DISTINCT session)::bigint AS contexts,count(DISTINCT session) FILTER(WHERE customer_id IS NOT NULL)::bigint AS identified,count(DISTINCT session) FILTER(WHERE kind='product_view')::bigint AS viewed,count(DISTINCT session) FILTER(WHERE kind='cart_add')::bigint AS added,count(DISTINCT session) FILTER(WHERE kind='checkout_start')::bigint AS checkout,(SELECT count(DISTINCT o.id) FROM orders o JOIN events e ON e.tenant=o.tenant AND e.cart_id=o.cart_id WHERE o.created_at>=now()-make_interval(days=>$3))::bigint AS ordered FROM events"
    );
    // dynamic SQL (this and the report queries below): composed with the shared ELIGIBLE consent clause.
    let row = sqlx::query(sqlx::AssertSqlSafe(q.as_str()))
        .bind(t)
        .bind(channel)
        .bind(days)
        .bind(version)
        .fetch_one(&mut *tx)
        .await?;
    let totals = json!({"events":row.get::<i64,_>("events"),"contexts":row.get::<i64,_>("contexts"),"identified":row.get::<i64,_>("identified"),"viewed":row.get::<i64,_>("viewed"),"added":row.get::<i64,_>("added"),"checkout":row.get::<i64,_>("checkout"),"ordered":row.get::<i64,_>("ordered")});
    let q = format!(
        "{prefix} SELECT dimension, label,count(*)::bigint AS events,count(DISTINCT session)::bigint AS contexts FROM events CROSS JOIN LATERAL (VALUES ('browser',coalesce(dimensions->>'browser','Other')),('viewport',coalesce(dimensions->>'viewport','unknown')),('country',coalesce(dimensions->>'country','unknown')),('geoCountry',coalesce(dimensions->>'geoCountry','unknown')),('referrer',coalesce(dimensions->>'referrerHost','direct')),('kind',kind),('locale',coalesce(dimensions->>'locale','unknown'))) d(dimension,label) GROUP BY dimension,label ORDER BY contexts DESC,label LIMIT 100"
    );
    // dynamic SQL: composed with the shared ELIGIBLE consent clause.
    let dimensions=sqlx::query(sqlx::AssertSqlSafe(q.as_str())).bind(t).bind(channel).bind(days).bind(version).fetch_all(&mut *tx).await?.iter().map(|r|json!({"dimension":r.get::<String,_>("dimension"),"label":r.get::<String,_>("label"),"events":r.get::<i64,_>("events"),"contexts":r.get::<i64,_>("contexts")})).collect::<Vec<_>>();
    let q = format!(
        "{prefix} SELECT (created_at AT TIME ZONE 'UTC')::date::text AS day,count(*)::bigint AS events,count(DISTINCT session)::bigint AS contexts FROM events GROUP BY day ORDER BY day"
    );
    // dynamic SQL: composed with the shared ELIGIBLE consent clause.
    let daily=sqlx::query(sqlx::AssertSqlSafe(q.as_str())).bind(t).bind(channel).bind(days).bind(version).fetch_all(&mut *tx).await?.iter().map(|r|json!({"day":r.get::<String,_>("day"),"events":r.get::<i64,_>("events"),"contexts":r.get::<i64,_>("contexts")})).collect::<Vec<_>>();
    let q = format!(
        "{prefix} SELECT e.product_id,p.name,p.category,count(*) FILTER(WHERE kind='product_view')::bigint AS views,count(*) FILTER(WHERE kind='cart_add')::bigint AS adds,count(*) FILTER(WHERE kind='product_question')::bigint AS questions,count(DISTINCT session)::bigint AS contexts FROM events e JOIN products p ON p.tenant=e.tenant AND p.id=e.product_id GROUP BY e.product_id,p.name,p.category ORDER BY contexts DESC,e.product_id LIMIT 20"
    );
    // dynamic SQL: composed with the shared ELIGIBLE consent clause.
    let rows = sqlx::query(sqlx::AssertSqlSafe(q.as_str()))
        .bind(t)
        .bind(channel)
        .bind(days)
        .bind(version)
        .fetch_all(&mut *tx)
        .await?;
    let mut products = rows
        .iter()
        .map(|r| ProductActivity {
            id: r.get("product_id"),
            name: r.get("name"),
            category: r.get("category"),
            views: r.get("views"),
            adds: r.get("adds"),
            questions: r.get("questions"),
            contexts: r.get("contexts"),
        })
        .collect::<Vec<_>>();
    let ids = products.iter().map(|p| p.id.clone()).collect::<Vec<_>>();
    let translations=sqlx::query!(r#"SELECT product_id,name AS "name!" FROM product_translations WHERE tenant=$1 AND product_id=ANY($2) AND language_id=ANY($3) AND name IS NOT NULL ORDER BY array_position($3,language_id)"#,
        t, &ids, chain).fetch_all(&mut *tx).await?;
    for product in &mut products {
        if let Some(r) = translations.iter().find(|r| r.product_id == product.id) {
            product.name.clone_from(&r.name);
        }
    }
    let q = format!(
        "{prefix} SELECT product_id,coalesce(dimensions->>'browser','Other') AS browser,count(DISTINCT session)::bigint AS contexts FROM events WHERE product_id=ANY($5) GROUP BY product_id,browser ORDER BY contexts DESC LIMIT 60"
    );
    // dynamic SQL: composed with the shared ELIGIBLE consent clause.
    let edges=sqlx::query(sqlx::AssertSqlSafe(q.as_str())).bind(t).bind(channel).bind(days).bind(version).bind(&ids).fetch_all(&mut *tx).await?.iter().map(|r|json!({"source":format!("browser:{}",r.get::<String,_>("browser")),"target":format!("product:{}",r.get::<String,_>("product_id")),"kind":"observed","count":r.get::<i64,_>("contexts")})).collect::<Vec<_>>();
    let relations=sqlx::query!("SELECT source_id,target_id,kind,state FROM knowledge_relations WHERE tenant=$1 AND source_id=ANY($2) AND kind='SERVES' AND state IN ('confirmed','evidenced') AND (valid_from IS NULL OR valid_from<=now()) AND (valid_until IS NULL OR valid_until>now()) ORDER BY source_id,target_id LIMIT 60",
        t, &ids).fetch_all(&mut *tx).await?.iter().map(|r|json!({"source":format!("product:{}",r.source_id),"target":format!("intent:{}",r.target_id),"kind":r.kind,"state":r.state})).collect::<Vec<_>>();
    let mut journeys = vec![];
    if r.pii {
        let q = format!(
            "{prefix} SELECT session,max(created_at)::text AS last,count(*)::bigint AS events,bool_or(customer_id IS NOT NULL) AS identified FROM events GROUP BY session ORDER BY max(created_at) DESC LIMIT 20"
        );
        // dynamic SQL: composed with the shared ELIGIBLE consent clause.
        journeys=sqlx::query(sqlx::AssertSqlSafe(q.as_str())).bind(t).bind(channel).bind(days).bind(version).fetch_all(&mut *tx).await?.iter().map(|r|json!({"session":r.get::<String,_>("session"),"last":r.get::<String,_>("last"),"events":r.get::<i64,_>("events"),"identified":r.get::<bool,_>("identified")})).collect();
    }
    let channels = sqlx::query!(r#"SELECT id,data->'name' AS "name!" FROM sales_channels WHERE tenant=$1 ORDER BY id LIMIT 100"#,
        t).fetch_all(&mut *tx).await?.iter().map(|r| {
            let names = &r.name;
            let name = [locale, locale.split('-').next().unwrap_or(locale), main, main.split('-').next().unwrap_or(main)].iter().find_map(|key| names[*key].as_str());
            json!({"id":r.id,"name":name.unwrap_or(r.id.as_str())})
        }).collect::<Vec<_>>();
    tx.commit().await?;
    Ok(
        json!({"salesChannelId":channel,"channels":channels,"days":days,"retentionDays":r.retention_days,"enabled":r.enabled,"eventsPerHour":r.events_per_hour,"totals":totals,"dimensions":dimensions,"daily":daily,"products":products,"graph":{"observations":edges,"semantic":relations},"journeys":journeys,"canInspectJourneys":r.pii,"basis":"consent-bound cart contexts; client observations, native orders; not unique humans or causal attribution","modelLearning":false,"geoConfigured":r.geo_configured}),
    )
}

/// Report activity of one product.
#[derive(serde::Serialize)]
struct ProductActivity {
    id: String,
    name: String,
    category: String,
    views: i64,
    adds: i64,
    questions: i64,
    contexts: i64,
}

/// Removes a bounded batch of expired observations.
pub async fn retire(a: &State) -> Result<()> {
    sqlx::query!("DELETE FROM session_signal_events WHERE (tenant,id) IN (SELECT tenant,id FROM session_signal_events WHERE purpose='analytics' AND expires_at<=now() ORDER BY expires_at LIMIT 1000 FOR UPDATE SKIP LOCKED)").execute(&a.db).await?;
    Ok(())
}
