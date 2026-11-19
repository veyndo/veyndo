//! Database deployment values read once per process; an invalid value stops the start.
use std::{env, sync::OnceLock};

pub struct Config {
    pub strict: bool,
    pub transaction_pooling: bool,
    pub listener_url: String,
    pub pool_max: u32,
    pub pool_min: u32,
    pub wait_ms: u32,
    pub connection_budget: u32,
}

pub fn get() -> &'static Config {
    static CONFIG: OnceLock<Config> = OnceLock::new();
    CONFIG.get_or_init(|| {
        let pool_max = number("DB_POOL_MAX", 20, 1, 256);
        let pool_min = number("DB_POOL_MIN", 0, 0, pool_max);
        let connection_budget = number("DB_CLUSTER_CONNECTION_BUDGET", 80, 4, 100000);
        let processes = number("DB_MAX_PROCESSES", 1, 1, 1024);
        assert!(
            (pool_max + 2).saturating_mul(processes) <= connection_budget,
            "Pool + listeners exceeds cluster connection budget"
        );
        let pooler_mode = env::var("DB_POOLER_MODE").unwrap_or("session".into());
        assert!(
            ["session", "transaction"].contains(&pooler_mode.as_str()),
            "Invalid DB_POOLER_MODE"
        );
        let transaction_pooling = pooler_mode == "transaction";
        let listener_url = if transaction_pooling {
            env::var("DATABASE_LISTENER_URL")
                .ok()
                .filter(|s| !s.is_empty())
                .expect(
                    "Transaction pooling requires a direct/session DATABASE_LISTENER_URL for LISTEN",
                )
        } else {
            env::var("DATABASE_LISTENER_URL")
                .ok()
                .filter(|s| !s.is_empty())
                .ok_or(env::VarError::NotPresent)
                .or_else(|_| env::var("DATABASE_RUNTIME_URL"))
                .or_else(|_| env::var("DATABASE_URL"))
                .unwrap_or_default()
        };
        Config {
            strict: flag("DB_RLS_REQUIRED", false),
            transaction_pooling,
            listener_url,
            pool_max,
            pool_min,
            wait_ms: number("DB_POOL_WAIT_MS", 5000, 100, 60000),
            connection_budget,
        }
    })
}

pub fn number(key: &str, default: u32, min: u32, max: u32) -> u32 {
    let n = env::var(key)
        .map(|v| v.parse().unwrap_or_else(|_| panic!("Invalid {key}")))
        .unwrap_or(default);
    assert!(
        (min..=max).contains(&n),
        "Invalid {key}: expected {min}..{max}"
    );
    n
}

pub fn flag(key: &str, default: bool) -> bool {
    match env::var(key).as_deref() {
        Ok("true") => true,
        Ok("false") => false,
        Err(_) => default,
        _ => panic!("Invalid boolean {key}"),
    }
}
