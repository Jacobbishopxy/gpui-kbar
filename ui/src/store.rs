use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use core::{DuckDbStore, StorageMode};

/// Helper for constructing a shared DuckDbStore in UI code.
pub fn default_store() -> Option<Arc<Mutex<DuckDbStore>>> {
    let legacy = PathBuf::from("data/cache.duckdb");
    let config = PathBuf::from("data/config.duckdb");
    let data = PathBuf::from("data/data.duckdb");

    let _ = DuckDbStore::migrate_legacy_cache_to_split(&legacy, &config, &data);

    let store = DuckDbStore::new_split(&config, &data, StorageMode::Both).ok()?;
    let store = Arc::new(Mutex::new(store));
    if let Ok(guard) = store.lock() {
        let _ = guard.ensure_universe_loaded("data/universe.csv");
    }
    Some(store)
}
