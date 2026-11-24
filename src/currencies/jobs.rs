//! The background loop that runs queued price jobs and refreshes automatic rates; the batch itself is owned by the pricing component.
use super::rates;
use crate::{App, RequestContext, env};
use axum::extract::FromRef;
use veyndo_pricing::ports::Pricing;
use veyndo_pricing::service::price_job_worker::batch;
pub(crate) fn worker(a: &App) {
    if !["all", "flow-worker"].contains(&env::var("PROCESS_ROLE").unwrap_or("all".into()).as_str())
    {
        return;
    }
    let a = a.clone();
    let pricing = Pricing::from_ref(&a);
    tokio::spawn(veyndo::tenant_scope::scoped(
        veyndo::tenant_scope::Scope::System,
        async move {
            let mut ticks = 0_u64;
            let mut cursor = String::new();
            let mut refreshing = true;
            loop {
                if let Err(e) = batch(&pricing).await {
                    eprintln!("currency job error: {}", e.1);
                }
                if ticks.is_multiple_of(3600) {
                    refreshing = true;
                    cursor.clear();
                }
                if refreshing
                    && ticks.is_multiple_of(60)
                    && let Ok(rows) = veyndo_shop::store::settings::auto_refresh(&a, &cursor).await
                {
                    refreshing = rows.len() == 50;
                    for (tenant, revision) in rows {
                        cursor = tenant;
                        if let Err(error) =
                            rates::refresh(&a, &cursor, revision, &RequestContext::new()).await
                        {
                            eprintln!("currency rate refresh retained previous rates: {}", error.1);
                        }
                    }
                }
                ticks += 1;
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        },
    ));
}
