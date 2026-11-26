//! Migration-only authority rollout commands; role passwords never pass through command arguments or output.
pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(2).collect();
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect(&std::env::var("DATABASE_URL")?)
        .await?;
    match args.first().map(String::as_str) {
        Some("configure") if args.len()==3 => {
            crate::store::configure_scope(&pool, &args[1], &args[2]).await?;
            println!("Database authority roles configured; enforcement remains unchanged");
        }
        Some("activate") if args.len()==1 => {
            crate::store::activate_scope(&pool).await?;
            println!("Authenticated database scope enforced");
        }
        Some("rotate") if args.len()==1 => {
            crate::store::rotate_scope_key(&pool).await?;
            println!("Database scope key rotated; retiring key has a ten minute verification overlap");
        }
        Some("status") if args.len()==1 => {
            let status = crate::store::scope_status(&pool).await?;
            println!("{status}");
        }
        _ => return Err("Usage: --database-authority configure SYSTEM_LOGIN TENANT_LOGIN | activate | rotate | status".into()),
    }
    pool.close().await;
    Ok(())
}
