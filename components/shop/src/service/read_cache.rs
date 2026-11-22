//! The process-wide read cache of shop settings and the language registry. Every hit still probes the authoritative SQL version; mutations and checkout locks never read through it.
use crate::domain::defaults::decode;
use crate::domain::patch;
use crate::types::settings::Settings;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use veyndo_core::cache::Cache;
use veyndo_core::{Error, Result, State, StatusCode, bad};

type Scope = (String, String);
/// Rule conditions inside channel overrides are checked by their owner; the server registers it at startup.
pub type Conditions = fn(&Value) -> Result<()>;

struct SettingsEntry {
    version: String,
    value: Arc<Settings>,
    revision: i64,
}
struct LanguageEntry {
    version: String,
    rows: Arc<Vec<LanguageRow>>,
}

/// One language of the global registry.
#[derive(Clone, Deserialize)]
pub struct LanguageRow {
    pub id: String,
    pub locale: String,
    pub parent_id: Option<String>,
}

/// Decoded entries with their versions and the counters an operator reads.
pub struct Reads {
    settings: Mutex<Cache<Scope, SettingsEntry>>,
    languages: Mutex<Cache<(), LanguageEntry>>,
    invalidations: AtomicU64,
    probes: AtomicU64,
    hits: AtomicU64,
    loads: AtomicU64,
    memo_hits: AtomicU64,
    enabled: bool,
}
impl Reads {
    fn new(enabled: bool) -> Self {
        Self {
            settings: Mutex::new(Cache::new(256, 32 * 1024 * 1024)),
            languages: Mutex::new(Cache::new(1, 1024 * 1024)),
            invalidations: AtomicU64::new(0),
            probes: AtomicU64::new(0),
            hits: AtomicU64::new(0),
            loads: AtomicU64::new(0),
            memo_hits: AtomicU64::new(0),
            enabled,
        }
    }
    /// Drops the decoded settings of one shop after a committed change.
    pub fn invalidate(&self, tenant: &str) {
        self.settings
            .lock()
            .unwrap()
            .remove_where(|(t, _)| t == tenant);
        self.invalidations.fetch_add(1, Ordering::Relaxed);
    }
    /// Drops every decoded entry, for example after the change listener reconnected.
    pub fn clear(&self) {
        self.settings.lock().unwrap().remove_where(|_| true);
        self.languages.lock().unwrap().remove_where(|_| true);
    }
    /// The counters and budget of the cache for operator metrics.
    pub fn snapshot(&self) -> Value {
        let (entries, bytes) = self.settings.lock().unwrap().usage();
        json!({"eventInvalidations":self.invalidations.load(Ordering::Relaxed),"enabled":self.enabled,"versionProbes":self.probes.load(Ordering::Relaxed),
            "decodedHits":self.hits.load(Ordering::Relaxed),"payloadLoads":self.loads.load(Ordering::Relaxed),
            "requestMemoHits":self.memo_hits.load(Ordering::Relaxed),"settingsEntries":entries,
            "estimatedSettingsBytes":bytes,"maxSettingsEntries":256,"settingsBudgetBytes":32*1024*1024})
    }
}

static READS: OnceLock<Reads> = OnceLock::new();
static CONDITIONS: OnceLock<Conditions> = OnceLock::new();

/// The cache of this process; `READ_CONTEXT_CACHE=false` turns decoding reuse off.
pub fn reads() -> &'static Reads {
    READS.get_or_init(|| Reads::new(std::env::var("READ_CONTEXT_CACHE").as_deref() != Ok("false")))
}

/// Registers the rule-condition check that channel overrides pass through; the first registration wins.
pub fn register_conditions(check: Conditions) {
    let _ = CONDITIONS.set(check);
}

/// The registered rule-condition check; before registration every override is refused.
pub fn conditions(v: &Value) -> Result<()> {
    match CONDITIONS.get() {
        Some(check) => check(v),
        None => Err(bad("Rule conditions are unavailable")),
    }
}

#[derive(Default)]
struct Memo {
    settings: HashMap<Scope, Arc<SettingsEntry>>,
    languages: Option<Arc<LanguageEntry>>,
}
tokio::task_local! { static MEMO: RefCell<Memo>; }

/// Runs a read-only request with a per-request memo, so repeated reads skip even the version probe.
pub async fn read_scope<F: std::future::Future>(future: F) -> F::Output {
    if MEMO.try_with(|_| ()).is_ok() {
        future.await
    } else {
        MEMO.scope(RefCell::new(Memo::default()), future).await
    }
}

/// The settings of a sales channel and the shop revision; the base settings for `default`.
pub async fn settings(a: &State, tenant: &str, channel: &str) -> Result<(Arc<Settings>, i64)> {
    let reads = reads();
    let key = (tenant.to_owned(), channel.to_owned());
    if reads.enabled
        && let Some(entry) = MEMO
            .try_with(|memo| memo.borrow().settings.get(&key).cloned())
            .ok()
            .flatten()
    {
        reads.memo_hits.fetch_add(1, Ordering::Relaxed);
        return Ok((entry.value.clone(), entry.revision));
    }
    let cached = if reads.enabled {
        reads.settings.lock().unwrap().get(&key)
    } else {
        None
    };
    let known = cached.as_ref().map(|e| e.version.as_str());
    reads.probes.fetch_add(1, Ordering::Relaxed);
    let row = sqlx::query(include_str!("read_cache_settings.sql"))
        .bind(tenant)
        .bind(channel)
        .bind(known)
        .fetch_optional(&a.db)
        .await?
        .ok_or(Error(StatusCode::NOT_FOUND, "Workspace not found".into()))?;
    if !row.get::<bool, _>("channel_exists") {
        return Err(Error(
            StatusCode::NOT_FOUND,
            "Sales channel not found".into(),
        ));
    }
    let version: String = row.get("version");
    let revision: i64 = row.get("revision");
    let entry = if let Some(cached) = cached.filter(|e| e.version == version) {
        reads.hits.fetch_add(1, Ordering::Relaxed);
        cached
    } else {
        reads.loads.fetch_add(1, Ordering::Relaxed);
        let base = decode(row.get("data"))?;
        let overrides: Option<Value> = row.get("patch");
        let value = if channel == "default" {
            base
        } else {
            patch::resolve(&base, overrides.unwrap_or(json!([])), &conditions)?
        };
        let weight = serde_json::to_vec(&value)
            .map_err(|_| bad("Invalid settings model"))?
            .len()
            .saturating_mul(3)
            .saturating_add(key.0.len() + key.1.len() + 512);
        let entry = Arc::new(SettingsEntry {
            version,
            value: Arc::new(value),
            revision,
        });
        if reads.enabled {
            reads
                .settings
                .lock()
                .unwrap()
                .insert(key.clone(), entry.clone(), weight);
        }
        entry
    };
    if reads.enabled {
        let _ = MEMO.try_with(|memo| {
            let mut memo = memo.borrow_mut();
            if memo.settings.len() < 8 {
                memo.settings.insert(key, entry.clone());
            }
        });
    }
    Ok((entry.value.clone(), entry.revision))
}

/// The base settings of a shop.
pub async fn config(a: &State, tenant: &str) -> Result<(Arc<Settings>, i64)> {
    settings(a, tenant, "default").await
}

/// The global language registry, versioned in the same transaction as every registry change.
pub async fn languages(a: &State) -> Result<Arc<Vec<LanguageRow>>> {
    let reads = reads();
    if reads.enabled
        && let Some(entry) = MEMO
            .try_with(|memo| memo.borrow().languages.clone())
            .ok()
            .flatten()
    {
        reads.memo_hits.fetch_add(1, Ordering::Relaxed);
        return Ok(entry.rows.clone());
    }
    let cached = if reads.enabled {
        reads.languages.lock().unwrap().get(&())
    } else {
        None
    };
    reads.probes.fetch_add(1, Ordering::Relaxed);
    let row = sqlx::query("SELECT version::text AS version,CASE WHEN version::text IS DISTINCT FROM $1 THEN (SELECT coalesce(jsonb_agg(to_jsonb(l) ORDER BY locale),'[]'::jsonb) FROM (SELECT id,locale,parent_id FROM languages) l) END AS data FROM read_context_versions WHERE namespace='languages'")
        .bind(cached.as_ref().map(|e| e.version.as_str())).fetch_one(&a.db).await?;
    let version: String = row.get("version");
    let entry = if let Some(cached) = cached.filter(|e| e.version == version) {
        reads.hits.fetch_add(1, Ordering::Relaxed);
        cached
    } else {
        reads.loads.fetch_add(1, Ordering::Relaxed);
        let data: Value = row.get("data");
        let weight = data.to_string().len().saturating_mul(3);
        let rows = serde_json::from_value(data).map_err(|_| bad("Invalid language registry"))?;
        let entry = Arc::new(LanguageEntry {
            version,
            rows: Arc::new(rows),
        });
        if reads.enabled {
            reads
                .languages
                .lock()
                .unwrap()
                .insert((), entry.clone(), weight);
        }
        entry
    };
    if reads.enabled {
        let _ = MEMO.try_with(|memo| memo.borrow_mut().languages = Some(entry.clone()));
    }
    Ok(entry.rows.clone())
}

#[cfg(test)]
mod tests {
    use super::{Reads, conditions, read_scope};
    use serde_json::json;

    #[test]
    fn the_snapshot_reports_the_budget_and_a_cleared_cache() {
        let reads = Reads::new(true);
        reads.invalidate("demo");
        reads.clear();
        let snapshot = reads.snapshot();
        assert_eq!(snapshot["enabled"], true);
        assert_eq!(snapshot["eventInvalidations"], 1);
        assert_eq!(snapshot["settingsEntries"], 0);
        assert_eq!(snapshot["maxSettingsEntries"], 256);
        assert_eq!(Reads::new(false).snapshot()["enabled"], false);
    }

    #[test]
    fn overrides_are_refused_until_the_condition_owner_registered() {
        assert!(conditions(&json!({"type":"always"})).is_err());
    }

    #[tokio::test]
    async fn read_scopes_nest_without_a_second_memo() {
        let value = read_scope(async { read_scope(async { 7 }).await }).await;
        assert_eq!(value, 7);
    }
}
