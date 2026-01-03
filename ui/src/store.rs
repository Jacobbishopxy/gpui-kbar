use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use core::{DuckDbStore, StorageMode};

/// Helper for constructing a shared DuckDbStore in UI code.
pub fn default_store() -> Option<Arc<Mutex<DuckDbStore>>> {
    let path = PathBuf::from("data/cache.duckdb");
    DuckDbStore::new(&path, StorageMode::Both)
        .ok()
        .map(|store| Arc::new(Mutex::new(store)))
}
