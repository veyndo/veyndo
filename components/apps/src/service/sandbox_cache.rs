//! Bounded module compilation cache. Prepare outside commerce locks; verify the digest under the lock.
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use veyndo_apps::sandbox::Sandbox;
use veyndo_core::{Error, Result, StatusCode, bad, hash};
/// At most 128 compiled modules, four cold compilations at a time.
pub struct Cache {
    entries: Mutex<HashMap<String, (u64, Arc<Sandbox>)>>,
    clock: std::sync::atomic::AtomicU64,
    slots: Arc<tokio::sync::Semaphore>,
    compiling: [tokio::sync::Mutex<()>; 32],
}
impl Default for Cache {
    fn default() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
            clock: std::sync::atomic::AtomicU64::new(0),
            slots: Arc::new(tokio::sync::Semaphore::new(4)),
            compiling: std::array::from_fn(|_| tokio::sync::Mutex::new(())),
        }
    }
}
impl Cache {
    /// The cached module of a key, only if its digest still matches.
    pub fn get(&self, tenant: &str, digest: &str) -> Option<Arc<Sandbox>> {
        let mut entries = self.entries.lock().unwrap();
        let (tick, sandbox) = entries.get_mut(tenant)?;
        if sandbox.digest() != digest {
            return None;
        }
        *tick = self
            .clock
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Some(sandbox.clone())
    }
    /// Stores a module, evicting the least recently used key when full.
    pub fn insert(&self, tenant: String, sandbox: Arc<Sandbox>) {
        let mut entries = self.entries.lock().unwrap();
        if !entries.contains_key(&tenant) && entries.len() >= 128 {
            let oldest = entries
                .iter()
                .min_by_key(|(_, v)| v.0)
                .map(|(k, _)| k.clone())
                .unwrap();
            entries.remove(&oldest);
        }
        let tick = self
            .clock
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        entries.insert(tenant, (tick, sandbox));
    }
    /// Compiles a module once per digest within five seconds.
    pub async fn compile(&self, key: &str, source: String) -> Result<Arc<Sandbox>> {
        let digest = hash(&source);
        if let Some(cached) = self.get(key, &digest) {
            return Ok(cached);
        }
        let slot = self.slots.clone().try_acquire_owned().map_err(|_| {
            Error(
                StatusCode::TOO_MANY_REQUESTS,
                "Policy compilation busy; retry shortly".into(),
            )
        })?;
        // Fixed stripes deduplicate cold compilation without an unbounded per-key lock map.
        let stripe = digest.as_bytes()[0] as usize % self.compiling.len();
        let _key = self.compiling[stripe].lock().await;
        if let Some(cached) = self.get(key, &digest) {
            return Ok(cached);
        }
        let compiled = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            tokio::task::spawn_blocking(move || {
                let _slot = slot;
                Sandbox::new(&source)
            }),
        )
        .await
        .map_err(|_| bad("Policy compilation exceeded 5 seconds; simplify the module"))?
        .map_err(|_| bad("Policy compilation failed"))?
        .map_err(bad)?;
        let sandbox = Arc::new(compiled);
        self.insert(key.into(), sandbox.clone());
        Ok(sandbox)
    }
}
#[cfg(test)]
mod tests {
    use super::Cache;
    use std::sync::Arc;
    use veyndo_apps::sandbox::Sandbox;
    #[test]
    fn cache_is_bounded_and_never_reuses_another_digest() {
        let c = Cache::default();
        let s = Arc::new(Sandbox::new("(module)").unwrap());
        for n in 0..200 {
            c.insert(format!("tenant-{n}"), s.clone());
        }
        assert_eq!(c.entries.lock().unwrap().len(), 128);
        assert!(c.get("tenant-199", "wrong").is_none());
        assert!(c.get("tenant-199", s.digest()).is_some());
    }

    /// An upgrade needs no restart: the next call compiles the new module, while a call that already
    /// holds the old module finishes on it.
    #[tokio::test]
    async fn an_upgrade_swaps_the_module_while_a_held_call_finishes_on_the_old_one() {
        let v1 =
            include_str!("../../../../extensions/apps/engraving/configuration.wat").to_string();
        let v2 = v1.replace(
            "if (result i64) local.get $fee else",
            "if (result i64) local.get $fee i64.const 2 i64.mul else",
        );
        assert_ne!(v1, v2);
        let c = Cache::default();
        let in_flight = c.compile("shop:engraving", v1).await.unwrap();
        let swapped = c.compile("shop:engraving", v2).await.unwrap();
        assert_ne!(in_flight.digest(), swapped.digest());
        assert!(c.get("shop:engraving", in_flight.digest()).is_none());
        assert_eq!(in_flight.contribution(300, 3).unwrap(), 300);
        assert_eq!(swapped.contribution(300, 3).unwrap(), 600);
        assert!(
            Arc::strong_count(&in_flight) == 1,
            "the cache no longer retains the old module"
        );
    }
}
