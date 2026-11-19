//! Bounded restart-safe fixed-price materialization with frozen FX, product locks and atomic checkpoints across replicas.
use super::*;
async fn batch(a: &App) -> Result<bool> {
    let mut tx = a.db.begin().await?;
    let Some(row)=sqlx::query("SELECT *,created_at::text AS snapshot_started FROM currency_price_jobs WHERE state='queued' ORDER BY created_at FOR UPDATE SKIP LOCKED LIMIT 1").fetch_optional(&mut *tx).await? else{return Ok(false)};
    let t: String = row.get("tenant");
    let id: String = row.get("id");
    let data: Value = row.get("data");
    sqlx::query("SAVEPOINT price_batch")
        .execute(&mut *tx)
        .await?;
    let result: Result<()> = async {
    let mut actor = RequestContext::new();
    if let Some(value) = data["actor"].as_str() {
        actor.insert(
            "x-rac-user",
            value.parse().map_err(|_| bad("Invalid saved actor"))?,
        );
    }
    history::context(&mut tx, &actor, "currency-price-job").await?;
    let cfg: Config =
        serde_json::from_value(data["config"].clone()).map_err(|_| bad("Invalid price job"))?;
    let d = cfg
        .definitions
        .iter()
        .find(|d| Some(d.code.as_str()) == data["currency"].as_str())
        .ok_or(bad("Missing price job currency"))?;
    let rows=sqlx::query("SELECT * FROM products WHERE tenant=$1 AND id>$2 AND id<=$3 AND created_at<=$4::text::timestamptz ORDER BY id LIMIT 100 FOR UPDATE").bind(&t).bind(row.get::<String,_>("cursor")).bind(row.get::<String,_>("ceiling")).bind(row.get::<String,_>("snapshot_started")).fetch_all(&mut *tx).await?;
    let mut cursor = row.get::<String, _>("cursor");
    for row in &rows {
        let p = product(row);
        cursor = p.id.clone();
        if data["overwrite"] != true && !p.extra["currencyPrices"][&d.code].is_null() {
            continue;
        }
        let price = amount(
            convert_from(
                p.price,
                p.extra["priceCurrency"]
                    .as_str()
                    .unwrap_or(&cfg.pricing_currency),
                &cfg,
                d,
            )?,
            &d.code,
            d.scale,
        )?
        .decimal();
        let mut prices = json!({"price":price,"generatedAt":chrono::Utc::now().to_rfc3339()});
        for (key, value) in [
            ("listPrice", p.list_price),
            ("regulationPrice", p.regulation_price),
        ] {
            if let Some(value) = value {
                prices[key] = json!(
                    amount(
                        convert_from(
                            value,
                            p.extra["priceCurrency"]
                                .as_str()
                                .unwrap_or(&cfg.pricing_currency),
                            &cfg,
                            d
                        )?,
                        &d.code,
                        d.scale
                    )?
                    .decimal()
                );
            }
        }
        sqlx::query("UPDATE products SET extra=jsonb_set(extra,'{currencyPrices}',coalesce(extra->'currencyPrices','{}') || jsonb_build_object($1::text,$2::jsonb)),revision=revision+1 WHERE tenant=$3 AND id=$4").bind(&d.code).bind(prices).bind(&t).bind(&p.id).execute(&mut *tx).await?;
    }
    sqlx::query("UPDATE currency_price_jobs SET cursor=$1,processed=processed+$2,state=$3 WHERE tenant=$4 AND id=$5").bind(cursor).bind(rows.len() as i64).bind(if rows.len()<100{"completed"}else{"queued"}).bind(&t).bind(&id).execute(&mut *tx).await?;
        Ok(())
    }.await;
    if let Err(error) = result {
        if error.0.is_server_error() {
            return Err(error);
        }
        sqlx::query("ROLLBACK TO SAVEPOINT price_batch")
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE currency_price_jobs SET state='failed',data=jsonb_set(data,'{error}',$1) WHERE tenant=$2 AND id=$3").bind(json!(error.1)).bind(&t).bind(&id).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(true)
}
pub(crate) fn worker(a: &App) {
    if !["all", "flow-worker"].contains(&env::var("PROCESS_ROLE").unwrap_or("all".into()).as_str())
    {
        return;
    }
    let a = a.clone();
    tokio::spawn(veyndo::tenant_scope::scoped(
        veyndo::tenant_scope::Scope::System,
        async move {
            let mut ticks = 0_u64;
            let mut cursor = String::new();
            let mut refreshing = true;
            loop {
                if let Err(e) = batch(&a).await {
                    eprintln!("currency job error: {}", e.1);
                }
                if ticks.is_multiple_of(3600) {
                    refreshing = true;
                    cursor.clear();
                }
                if refreshing && ticks.is_multiple_of(60)
                  && let Ok(rows)=sqlx::query("SELECT tenant,revision FROM commerce_settings WHERE data->'currencies'->>'autoRefresh'='true' AND tenant>$1 ORDER BY tenant LIMIT 50").bind(&cursor).fetch_all(&a.db).await {
                    refreshing=rows.len()==50;
                    for row in rows {cursor=row.get("tenant");if let Err(error)=rates::refresh(&a,&cursor,row.get("revision"),&RequestContext::new()).await {eprintln!("currency rate refresh retained previous rates: {}",error.1);}}
                }
                ticks += 1;
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        },
    ));
}
