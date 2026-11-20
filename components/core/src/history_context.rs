//! Transaction-local actor, source and reason that the history triggers record with every change.
use crate::shop_scope::header;
use crate::{RequestContext, Result};

pub async fn context(
    conn: &mut sqlx::PgConnection,
    h: &RequestContext,
    source: &str,
) -> Result<()> {
    sqlx::query("SELECT set_config('veyndo.actor',$1,true),set_config('veyndo.source',$2,true),set_config('veyndo.reason',$3,true)")
 .bind(header(h,"x-rac-user").unwrap_or(""))
 .bind(source).bind(header(h,"x-rac-history-reason").unwrap_or(""))
 .execute(conn).await?;
    Ok(())
}
pub async fn customer_context(conn: &mut sqlx::PgConnection, email: &str) -> Result<()> {
    sqlx::query(
        "SELECT set_config('veyndo.actor',$1,true),set_config('veyndo.source','customer',true)",
    )
    .bind(format!("customer:{email}"))
    .execute(conn)
    .await?;
    Ok(())
}
