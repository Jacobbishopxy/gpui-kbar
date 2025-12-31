use std::path::PathBuf;

use core::{DuckDbStore, StorageMode};

/// Helper for constructing a shared DuckDbStore in UI code.
pub fn default_store() -> Option<DuckDbStore> {
    let path = PathBuf::from("data/cache.duckdb");
    DuckDbStore::new(&path, StorageMode::Both).ok()
}
