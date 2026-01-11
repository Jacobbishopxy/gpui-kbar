use std::path::{Path, PathBuf};

use duckdb::{Connection, params, params_from_iter};
use std::collections::HashSet;
use thiserror::Error;
use time::{
    OffsetDateTime,
    error::{Format, Parse},
    format_description::well_known::Rfc3339,
};

use crate::Candle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UniverseRow {
    pub filters: String,
    pub badge: String,
    pub symbol: String,
    pub name: String,
    pub market: String,
    pub venue: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageMode {
    Memory,
    Disk,
    Both,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataRange {
    All,
    From(OffsetDateTime),
    Until(OffsetDateTime),
    Between {
        start: OffsetDateTime,
        end: OffsetDateTime,
    },
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct UserSession {
    pub active_source: Option<String>,
    pub interval: Option<String>,
    pub range_index: Option<usize>,
    pub replay_mode: Option<bool>,
    pub watchlist: Vec<String>,
    pub view_offset: Option<f32>,
    pub zoom: Option<f32>,
    pub perf_mode: Option<bool>,
    pub perf_n: Option<usize>,
    pub perf_step_secs: Option<i64>,
    pub live_mode: Option<bool>,
    pub live_pub: Option<String>,
    pub chunk_rep: Option<String>,
    pub live_source_id: Option<String>,
    pub live_interval: Option<String>,
}

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("no storage backend available")]
    NoBackend,
    #[error("disk path is required to use disk-backed storage")]
    MissingDiskPath,
    #[error("duckdb error: {0}")]
    DuckDb(#[from] duckdb::Error),
    #[error("failed to parse timestamp '{value}': {source}")]
    TimeParse { value: String, source: Parse },
    #[error("failed to format timestamp: {0}")]
    TimeFormat(#[from] Format),
}

#[derive(Default)]
struct StoreBackend {
    disk_path: Option<PathBuf>,
    memory: Option<Connection>,
    disk: Option<Connection>,
}

pub struct DuckDbStore {
    mode: StorageMode,
    shared_disk: Option<Connection>,
    config: StoreBackend,
    data: StoreBackend,
}

impl DuckDbStore {
    pub fn mode(&self) -> StorageMode {
        self.mode
    }

    pub fn new(path: impl AsRef<Path>, mode: StorageMode) -> Result<Self, StoreError> {
        Self::new_split(path.as_ref(), path.as_ref(), mode)
    }

    pub fn new_split(
        config_path: impl AsRef<Path>,
        data_path: impl AsRef<Path>,
        mode: StorageMode,
    ) -> Result<Self, StoreError> {
        let mut store = Self {
            mode,
            shared_disk: None,
            config: StoreBackend {
                disk_path: Some(config_path.as_ref().to_path_buf()),
                ..Default::default()
            },
            data: StoreBackend {
                disk_path: Some(data_path.as_ref().to_path_buf()),
                ..Default::default()
            },
        };

        store.reconfigure(mode)?;
        Ok(store)
    }

    pub fn set_disk_path(&mut self, path: impl AsRef<Path>) {
        let path = path.as_ref().to_path_buf();
        self.config.disk_path = Some(path.clone());
        self.data.disk_path = Some(path);
    }

    pub fn set_mode(&mut self, mode: StorageMode) -> Result<(), StoreError> {
        self.reconfigure(mode)?;
        Ok(())
    }

    fn open_config_memory(&self) -> Result<Connection, StoreError> {
        let conn = Connection::open_in_memory()?;
        init_config_schema(&conn)?;
        Ok(conn)
    }

    fn open_data_memory(&self) -> Result<Connection, StoreError> {
        let conn = Connection::open_in_memory()?;
        init_data_schema(&conn)?;
        Ok(conn)
    }

    fn open_config_disk(&self, path: &Path) -> Result<Connection, StoreError> {
        let conn = Connection::open(path)?;
        init_config_schema(&conn)?;
        Ok(conn)
    }

    fn open_data_disk(&self, path: &Path) -> Result<Connection, StoreError> {
        let conn = Connection::open(path)?;
        init_data_schema(&conn)?;
        Ok(conn)
    }

    fn reconfigure(&mut self, mode: StorageMode) -> Result<(), StoreError> {
        let disk_required = matches!(mode, StorageMode::Disk | StorageMode::Both);
        if disk_required && (self.config.disk_path.is_none() || self.data.disk_path.is_none()) {
            return Err(StoreError::MissingDiskPath);
        }

        self.shared_disk = None;
        self.config.memory = None;
        self.config.disk = None;
        self.data.memory = None;
        self.data.disk = None;

        match mode {
            StorageMode::Memory => {
                self.config.memory = Some(self.open_config_memory()?);
                self.data.memory = Some(self.open_data_memory()?);
            }
            StorageMode::Disk => {
                let config_path = self.config.disk_path.clone().expect("checked above");
                let data_path = self.data.disk_path.clone().expect("checked above");
                if config_path == data_path {
                    let conn = Connection::open(&config_path)?;
                    init_config_schema(&conn)?;
                    init_data_schema(&conn)?;
                    self.shared_disk = Some(conn);
                } else {
                    self.config.disk = Some(self.open_config_disk(&config_path)?);
                    self.data.disk = Some(self.open_data_disk(&data_path)?);
                }
            }
            StorageMode::Both => {
                let config_path = self.config.disk_path.clone().expect("checked above");
                let data_path = self.data.disk_path.clone().expect("checked above");
                self.config.memory = Some(self.open_config_memory()?);
                self.data.memory = Some(self.open_data_memory()?);
                if config_path == data_path {
                    let conn = Connection::open(&config_path)?;
                    init_config_schema(&conn)?;
                    init_data_schema(&conn)?;
                    self.shared_disk = Some(conn);
                } else {
                    self.config.disk = Some(self.open_config_disk(&config_path)?);
                    self.data.disk = Some(self.open_data_disk(&data_path)?);
                }
            }
        }

        self.mode = mode;

        Ok(())
    }

    fn config_connections(&self) -> impl Iterator<Item = &Connection> {
        self.config
            .memory
            .iter()
            .chain(self.config.disk.iter())
            .chain(self.shared_disk.iter())
    }

    fn data_connections(&self) -> impl Iterator<Item = &Connection> {
        self.data
            .memory
            .iter()
            .chain(self.data.disk.iter())
            .chain(self.shared_disk.iter())
    }

    pub fn migrate_legacy_cache_to_split(
        legacy_cache: impl AsRef<Path>,
        config_path: impl AsRef<Path>,
        data_path: impl AsRef<Path>,
    ) -> Result<(), StoreError> {
        let legacy_cache = legacy_cache.as_ref();
        let config_path = config_path.as_ref();
        let data_path = data_path.as_ref();

        if !legacy_cache.exists() {
            return Ok(());
        }

        let should_migrate_config = !config_path.exists();
        let should_migrate_data = !data_path.exists();
        if !should_migrate_config && !should_migrate_data {
            return Ok(());
        }

        if should_migrate_config {
            let conn = Connection::open(config_path)?;
            init_config_schema(&conn)?;
            attach_and_copy(
                &conn,
                legacy_cache,
                &[
                    "INSERT INTO session_state SELECT * FROM legacy.session_state",
                    "INSERT INTO watchlist SELECT * FROM legacy.watchlist",
                ],
            )?;
        }

        if should_migrate_data {
            let conn = Connection::open(data_path)?;
            init_data_schema(&conn)?;
            attach_and_copy(
                &conn,
                legacy_cache,
                &[
                    "INSERT INTO candles SELECT * FROM legacy.candles",
                    "INSERT INTO indicator_values SELECT * FROM legacy.indicator_values",
                ],
            )?;
        }

        Ok(())
    }

    pub fn ensure_universe_loaded(&self, csv_path: impl AsRef<Path>) -> Result<(), StoreError> {
        let csv_path = csv_path.as_ref();
        if !csv_path.exists() {
            return Ok(());
        }
        let sql_path = csv_path.to_string_lossy().replace('\'', "''");

        for conn in self.data_connections() {
            let mut stmt = conn.prepare("SELECT COUNT(*) FROM universe")?;
            let mut rows = stmt.query([])?;
            let count: i64 = rows.next()?.and_then(|row| row.get(0).ok()).unwrap_or(0);
            if count > 0 {
                continue;
            }
            conn.execute_batch(&format!(
                "INSERT INTO universe
                 SELECT filters, badge, symbol, name, market, venue
                 FROM read_csv_auto('{sql_path}', HEADER=true);"
            ))?;
        }

        Ok(())
    }

    pub fn load_universe_rows(&self) -> Result<Vec<UniverseRow>, StoreError> {
        let mut out = Vec::new();
        for conn in self.data_connections() {
            let mut stmt = conn.prepare(
                "SELECT filters, badge, symbol, name, market, venue
                 FROM universe
                 ORDER BY symbol ASC",
            )?;
            let mut rows = stmt.query([])?;
            while let Some(row) = rows.next()? {
                out.push(UniverseRow {
                    filters: row.get(0)?,
                    badge: row.get(1)?,
                    symbol: row.get(2)?,
                    name: row.get(3)?,
                    market: row.get(4)?,
                    venue: row.get(5)?,
                });
            }
            if !out.is_empty() {
                return Ok(out);
            }
        }
        Ok(out)
    }

    pub fn write_candles(&self, symbol: &str, candles: &[Candle]) -> Result<(), StoreError> {
        if self.data_connections().count() == 0 {
            return Err(StoreError::NoBackend);
        }

        let deduped = dedup_by_timestamp(candles);

        for conn in self.data_connections() {
            // Replace any existing rows for this symbol to avoid duplicates when reloading.
            //
            // Use an appender for bulk insertion; row-by-row `execute` is noticeably slower
            // for 100k+ candles and can hurt UI load times.
            conn.execute_batch("BEGIN TRANSACTION")?;
            let result: Result<(), StoreError> = (|| {
                conn.execute("DELETE FROM candles WHERE symbol = ?", params![symbol])?;

                let mut app = conn.appender("candles")?;
                for candle in &deduped {
                    let ts = candle.timestamp.format(&Rfc3339)?;
                    app.append_row(params![
                        symbol,
                        ts,
                        candle.open,
                        candle.high,
                        candle.low,
                        candle.close,
                        candle.volume
                    ])?;
                }
                app.flush()?;

                Ok(())
            })();

            match result {
                Ok(()) => conn.execute_batch("COMMIT")?,
                Err(err) => {
                    let _ = conn.execute_batch("ROLLBACK");
                    return Err(err);
                }
            }
        }

        Ok(())
    }

    /// Appends candle rows for `symbol` without clearing the full symbol history.
    ///
    /// Existing rows in the appended time span are deleted first to avoid duplicate timestamps.
    pub fn append_candles(&self, symbol: &str, candles: &[Candle]) -> Result<(), StoreError> {
        if self.data_connections().count() == 0 {
            return Err(StoreError::NoBackend);
        }
        if candles.is_empty() {
            return Ok(());
        }

        let deduped = dedup_by_timestamp(candles);
        let (min_ts, max_ts) = deduped.iter().fold(
            (deduped[0].timestamp, deduped[0].timestamp),
            |(min_ts, max_ts), candle| (min_ts.min(candle.timestamp), max_ts.max(candle.timestamp)),
        );
        let min_ts = min_ts.format(&Rfc3339)?;
        let max_ts = max_ts.format(&Rfc3339)?;

        for conn in self.data_connections() {
            conn.execute_batch("BEGIN TRANSACTION")?;
            let result: Result<(), StoreError> = (|| {
                conn.execute(
                    "DELETE FROM candles WHERE symbol = ? AND timestamp >= ? AND timestamp <= ?",
                    params![symbol, min_ts, max_ts],
                )?;

                let mut app = conn.appender("candles")?;
                for candle in &deduped {
                    let ts = candle.timestamp.format(&Rfc3339)?;
                    app.append_row(params![
                        symbol,
                        ts,
                        candle.open,
                        candle.high,
                        candle.low,
                        candle.close,
                        candle.volume
                    ])?;
                }
                app.flush()?;

                Ok(())
            })();

            match result {
                Ok(()) => conn.execute_batch("COMMIT")?,
                Err(err) => {
                    let _ = conn.execute_batch("ROLLBACK");
                    return Err(err);
                }
            }
        }

        Ok(())
    }

    pub fn load_candles(
        &self,
        symbol: &str,
        range: Option<DataRange>,
    ) -> Result<Vec<Candle>, StoreError> {
        let mut result = Vec::new();
        for conn in self.data_connections() {
            let mut conditions = vec!["symbol = ?".to_string()];
            let mut params: Vec<String> = vec![symbol.to_string()];

            if let Some(range) = &range {
                match range {
                    DataRange::All => {}
                    DataRange::From(start) => {
                        conditions.push("timestamp >= ?".to_string());
                        params.push(start.format(&Rfc3339)?);
                    }
                    DataRange::Until(end) => {
                        conditions.push("timestamp <= ?".to_string());
                        params.push(end.format(&Rfc3339)?);
                    }
                    DataRange::Between { start, end } => {
                        conditions.push("timestamp >= ?".to_string());
                        conditions.push("timestamp <= ?".to_string());
                        params.push(start.format(&Rfc3339)?);
                        params.push(end.format(&Rfc3339)?);
                    }
                }
            }

            let query = format!(
                "SELECT timestamp, open, high, low, close, volume
                 FROM candles
                 WHERE {}
                 ORDER BY timestamp ASC",
                conditions.join(" AND ")
            );
            let mut stmt = conn.prepare(&query)?;
            let mut rows = stmt.query(params_from_iter(params.clone()))?;
            while let Some(row) = rows.next()? {
                let ts_str: String = row.get(0)?;
                let ts = OffsetDateTime::parse(&ts_str, &Rfc3339).map_err(|source| {
                    StoreError::TimeParse {
                        value: ts_str.clone(),
                        source,
                    }
                })?;
                let open: f64 = row.get(1)?;
                let high: f64 = row.get(2)?;
                let low: f64 = row.get(3)?;
                let close: f64 = row.get(4)?;
                let volume: f64 = row.get(5)?;
                result.push(Candle {
                    timestamp: ts,
                    open,
                    high,
                    low,
                    close,
                    volume,
                });
            }

            if !result.is_empty() {
                return Ok(dedup_by_timestamp(&result));
            }
        }

        Ok(dedup_by_timestamp(&result))
    }

    pub fn write_indicator_values(
        &self,
        symbol: &str,
        indicator: &str,
        values: &[(OffsetDateTime, f64)],
    ) -> Result<(), StoreError> {
        if self.data_connections().count() == 0 {
            return Err(StoreError::NoBackend);
        }

        for conn in self.data_connections() {
            let mut stmt = conn.prepare(
                "INSERT INTO indicator_values (symbol, indicator, timestamp, value)
                 VALUES (?1, ?2, ?3, ?4)",
            )?;
            for (ts, value) in values {
                let ts = ts.format(&Rfc3339)?;
                stmt.execute(params![symbol, indicator, ts, *value])?;
            }
        }

        Ok(())
    }

    pub fn load_indicator_values(
        &self,
        symbol: &str,
        indicator: &str,
        range: Option<DataRange>,
    ) -> Result<Vec<(OffsetDateTime, f64)>, StoreError> {
        let mut result = Vec::new();
        for conn in self.data_connections() {
            let mut conditions = vec!["symbol = ?".to_string(), "indicator = ?".to_string()];
            let mut params: Vec<String> = vec![symbol.to_string(), indicator.to_string()];

            if let Some(range) = &range {
                match range {
                    DataRange::All => {}
                    DataRange::From(start) => {
                        conditions.push("timestamp >= ?".to_string());
                        params.push(start.format(&Rfc3339)?);
                    }
                    DataRange::Until(end) => {
                        conditions.push("timestamp <= ?".to_string());
                        params.push(end.format(&Rfc3339)?);
                    }
                    DataRange::Between { start, end } => {
                        conditions.push("timestamp >= ?".to_string());
                        conditions.push("timestamp <= ?".to_string());
                        params.push(start.format(&Rfc3339)?);
                        params.push(end.format(&Rfc3339)?);
                    }
                }
            }

            let query = format!(
                "SELECT timestamp, value
                 FROM indicator_values
                 WHERE {}
                 ORDER BY timestamp ASC",
                conditions.join(" AND ")
            );
            let mut stmt = conn.prepare(&query)?;
            let mut rows = stmt.query(params_from_iter(params.clone()))?;
            while let Some(row) = rows.next()? {
                let ts_str: String = row.get(0)?;
                let ts = OffsetDateTime::parse(&ts_str, &Rfc3339).map_err(|source| {
                    StoreError::TimeParse {
                        value: ts_str.clone(),
                        source,
                    }
                })?;
                let value: f64 = row.get(1)?;
                result.push((ts, value));
            }

            if !result.is_empty() {
                return Ok(result);
            }
        }

        Ok(result)
    }

    pub fn set_session_value(&self, key: &str, value: &str) -> Result<(), StoreError> {
        if self.config_connections().count() == 0 {
            return Err(StoreError::NoBackend);
        }
        for conn in self.config_connections() {
            conn.execute(
                "INSERT INTO session_state(key, value) VALUES (?, ?)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![key, value],
            )?;
        }
        Ok(())
    }

    pub fn get_session_value(&self, key: &str) -> Result<Option<String>, StoreError> {
        for conn in self.config_connections() {
            let mut stmt = conn.prepare("SELECT value FROM session_state WHERE key = ? LIMIT 1")?;
            let mut rows = stmt.query([key])?;
            if let Some(row) = rows.next()? {
                let v: String = row.get(0)?;
                return Ok(Some(v));
            }
        }
        Ok(None)
    }

    pub fn set_watchlist(&self, symbols: &[String]) -> Result<(), StoreError> {
        if self.config_connections().count() == 0 {
            return Err(StoreError::NoBackend);
        }
        for conn in self.config_connections() {
            let tx = conn.unchecked_transaction()?;
            tx.execute("DELETE FROM watchlist", [])?;
            for sym in symbols {
                tx.execute("INSERT INTO watchlist(symbol) VALUES (?)", params![sym])?;
            }
            tx.commit()?;
        }
        Ok(())
    }

    pub fn get_watchlist(&self) -> Result<Vec<String>, StoreError> {
        let mut out = Vec::new();
        for conn in self.config_connections() {
            let mut stmt = conn.prepare("SELECT symbol FROM watchlist ORDER BY symbol ASC")?;
            let mut rows = stmt.query([])?;
            while let Some(row) = rows.next()? {
                let sym: String = row.get(0)?;
                out.push(sym);
            }
            if !out.is_empty() {
                return Ok(out);
            }
        }
        Ok(out)
    }

    pub fn load_user_session(&self) -> Result<UserSession, StoreError> {
        let active_source = self.get_session_value("active_source")?;
        let interval = self.get_session_value("interval")?;
        let range_index = self
            .get_session_value("range_index")?
            .and_then(|r| r.parse::<usize>().ok());
        let replay_mode = self.get_session_value("replay_mode")?.map(|v| v == "true");
        let watchlist = self.get_watchlist()?;
        let view_offset = self
            .get_session_value("view_offset")?
            .and_then(|v| v.parse::<f32>().ok());
        let zoom = self
            .get_session_value("zoom")?
            .and_then(|v| v.parse::<f32>().ok());
        let perf_mode = self.get_session_value("perf_mode")?.map(|v| v == "true");
        let perf_n = self
            .get_session_value("perf_n")?
            .and_then(|v| v.parse::<usize>().ok());
        let perf_step_secs = self
            .get_session_value("perf_step_secs")?
            .and_then(|v| v.parse::<i64>().ok());
        let live_mode = self.get_session_value("live_mode")?.map(|v| v == "true");
        let live_pub = self.get_session_value("live_pub")?;
        let chunk_rep = self.get_session_value("chunk_rep")?;
        let live_source_id = self.get_session_value("live_source_id")?;
        let live_interval = self.get_session_value("live_interval")?;

        Ok(UserSession {
            active_source,
            interval,
            range_index,
            replay_mode,
            watchlist,
            view_offset,
            zoom,
            perf_mode,
            perf_n,
            perf_step_secs,
            live_mode,
            live_pub,
            chunk_rep,
            live_source_id,
            live_interval,
        })
    }
}

fn init_config_schema(conn: &Connection) -> Result<(), StoreError> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS session_state (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS watchlist (
            symbol TEXT PRIMARY KEY
        );
        ",
    )?;
    Ok(())
}

fn init_data_schema(conn: &Connection) -> Result<(), StoreError> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS candles (
            symbol TEXT NOT NULL,
            timestamp TEXT NOT NULL,
            open DOUBLE NOT NULL,
            high DOUBLE NOT NULL,
            low DOUBLE NOT NULL,
            close DOUBLE NOT NULL,
            volume DOUBLE NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_candles_symbol_ts ON candles(symbol, timestamp);

        CREATE TABLE IF NOT EXISTS indicator_values (
            symbol TEXT NOT NULL,
            indicator TEXT NOT NULL,
            timestamp TEXT NOT NULL,
            value DOUBLE NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_indicator_symbol_ts ON indicator_values(symbol, indicator, timestamp);

        CREATE TABLE IF NOT EXISTS universe (
            filters TEXT NOT NULL,
            badge TEXT NOT NULL,
            symbol TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            market TEXT NOT NULL,
            venue TEXT NOT NULL
        );
        ",
    )?;
    Ok(())
}

fn attach_and_copy(
    conn: &Connection,
    legacy_path: &Path,
    statements: &[&str],
) -> Result<(), StoreError> {
    let legacy_path = legacy_path.to_string_lossy().replace('\'', "''");
    conn.execute_batch(&format!("ATTACH '{legacy_path}' AS legacy;"))?;
    for statement in statements {
        let _ = conn.execute_batch(statement);
    }
    conn.execute_batch("DETACH legacy;")?;
    Ok(())
}

fn dedup_by_timestamp(candles: &[Candle]) -> Vec<Candle> {
    // Keep the last occurrence for any timestamp to favor freshest data.
    let mut seen = HashSet::new();
    let mut unique = Vec::with_capacity(candles.len());
    for candle in candles.iter().rev() {
        if seen.insert(candle.timestamp) {
            unique.push(candle.clone());
        }
    }
    unique.reverse();
    unique
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};
    use time::macros::datetime;

    fn temp_path() -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("gpui-kbar-store-{nonce}.duckdb"))
    }

    fn sample_candles() -> Vec<Candle> {
        vec![
            Candle {
                timestamp: datetime!(2024-01-01 00:00:00 UTC),
                open: 1.0,
                high: 2.0,
                low: 0.5,
                close: 1.5,
                volume: 10.0,
            },
            Candle {
                timestamp: datetime!(2024-01-01 00:01:00 UTC),
                open: 1.5,
                high: 2.5,
                low: 1.0,
                close: 2.0,
                volume: 15.0,
            },
            Candle {
                timestamp: datetime!(2024-01-01 00:02:00 UTC),
                open: 2.0,
                high: 3.0,
                low: 1.5,
                close: 2.5,
                volume: 12.0,
            },
        ]
    }

    #[test]
    fn append_candles_keeps_existing_history() {
        let store = DuckDbStore::new(temp_path(), StorageMode::Memory).unwrap();
        store.write_candles("SYM", &sample_candles()).unwrap();

        let appended = vec![Candle {
            timestamp: datetime!(2024-01-01 00:03:00 UTC),
            open: 2.5,
            high: 3.5,
            low: 2.0,
            close: 3.0,
            volume: 20.0,
        }];
        store.append_candles("SYM", &appended).unwrap();

        let loaded = store.load_candles("SYM", None).unwrap();
        assert_eq!(loaded.len(), 4);
        assert_eq!(loaded.last().unwrap().close, 3.0);
    }

    #[test]
    fn roundtrip_memory() {
        let store = DuckDbStore::new(temp_path(), StorageMode::Memory).unwrap();
        let candles = sample_candles();
        store.write_candles("SYM", &candles).unwrap();
        let loaded = store.load_candles("SYM", None).unwrap();
        assert_eq!(loaded.len(), candles.len());
        assert_eq!(loaded[0].open, 1.0);
        assert_eq!(loaded[1].close, 2.0);
    }

    #[test]
    fn switching_modes_keeps_working() {
        let path = temp_path();
        let mut store = DuckDbStore::new(&path, StorageMode::Memory).unwrap();
        store.set_mode(StorageMode::Both).unwrap();
        let candles = sample_candles();
        store.write_candles("SWITCH", &candles).unwrap();
        let loaded = store.load_candles("SWITCH", None).unwrap();
        assert_eq!(loaded.len(), candles.len());
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn roundtrip_disk_and_indicators() {
        let path = temp_path();
        let store = DuckDbStore::new(&path, StorageMode::Disk).unwrap();
        let candles = sample_candles();
        store.write_candles("ABC", &candles).unwrap();

        let indicator_points = vec![
            (datetime!(2024-01-01 00:00:00 UTC), 10.0),
            (datetime!(2024-01-01 00:01:00 UTC), 11.5),
            (datetime!(2024-01-01 00:02:00 UTC), 12.0),
        ];
        store
            .write_indicator_values("ABC", "SMA", &indicator_points)
            .unwrap();

        let loaded = store.load_candles("ABC", None).unwrap();
        assert_eq!(loaded.len(), 3);
        let loaded_indicators = store.load_indicator_values("ABC", "SMA", None).unwrap();
        assert_eq!(loaded_indicators.len(), 3);
        assert_eq!(loaded_indicators[0].1, 10.0);

        store
            .set_session_value("active_source", "ABC")
            .expect("set session");
        let saved = store
            .get_session_value("active_source")
            .expect("get session");
        assert_eq!(saved, Some("ABC".to_string()));

        std::fs::remove_file(path).ok();
    }

    #[test]
    fn range_filters() {
        let store = DuckDbStore::new(temp_path(), StorageMode::Memory).unwrap();
        let candles = sample_candles();
        store.write_candles("RANGE", &candles).unwrap();

        let from = store
            .load_candles(
                "RANGE",
                Some(DataRange::From(datetime!(2024-01-01 00:01:00 UTC))),
            )
            .unwrap();
        assert_eq!(from.len(), 2);

        let until = store
            .load_candles(
                "RANGE",
                Some(DataRange::Until(datetime!(2024-01-01 00:01:00 UTC))),
            )
            .unwrap();
        assert_eq!(until.len(), 2);

        let between = store
            .load_candles(
                "RANGE",
                Some(DataRange::Between {
                    start: datetime!(2024-01-01 00:01:00 UTC),
                    end: datetime!(2024-01-01 00:02:00 UTC),
                }),
            )
            .unwrap();
        assert_eq!(between.len(), 2);
    }

    #[test]
    fn writing_same_symbol_replaces_existing_rows() {
        let store = DuckDbStore::new(temp_path(), StorageMode::Memory).unwrap();
        let mut candles = sample_candles();
        let mut newer = candles[0].clone();
        newer.open = 9.99;
        candles.push(newer);

        store.write_candles("DUP", &candles).unwrap();
        store.write_candles("DUP", &candles).unwrap();

        let loaded = store.load_candles("DUP", None).unwrap();
        // Duplicates for the same timestamp are removed and the newest value wins.
        assert_eq!(loaded.len(), sample_candles().len());
        assert_eq!(loaded[0].timestamp, candles[0].timestamp);
        assert!((loaded[0].open - 9.99).abs() < f64::EPSILON);
    }

    #[test]
    fn load_user_session_collects_all_values() {
        let store = DuckDbStore::new(temp_path(), StorageMode::Memory).unwrap();
        store
            .set_session_value("active_source", "AAPL")
            .expect("active source");
        store.set_session_value("interval", "5m").expect("interval");
        store.set_session_value("range_index", "3").expect("range");
        store
            .set_session_value("replay_mode", "true")
            .expect("replay");
        store
            .set_session_value("view_offset", "5.5")
            .expect("view_offset");
        store.set_session_value("zoom", "2.5").expect("zoom");
        store
            .set_session_value("perf_mode", "true")
            .expect("perf_mode");
        store.set_session_value("perf_n", "200000").expect("perf_n");
        store
            .set_session_value("perf_step_secs", "60")
            .expect("perf_step_secs");
        store
            .set_session_value("live_mode", "true")
            .expect("live_mode");
        store
            .set_session_value("live_pub", "tcp://127.0.0.1:5556")
            .expect("live_pub");
        store
            .set_session_value("chunk_rep", "tcp://127.0.0.1:5557")
            .expect("chunk_rep");
        store
            .set_session_value("live_source_id", "SIM")
            .expect("live_source_id");
        store
            .set_session_value("live_interval", "1s")
            .expect("live_interval");
        store
            .set_watchlist(&["TSLA".to_string(), "AAPL".to_string()])
            .expect("watchlist");

        let session = store.load_user_session().expect("session");
        assert_eq!(session.active_source.as_deref(), Some("AAPL"));
        assert_eq!(session.interval.as_deref(), Some("5m"));
        assert_eq!(session.range_index, Some(3));
        assert_eq!(session.replay_mode, Some(true));
        assert_eq!(
            session.watchlist,
            vec!["AAPL".to_string(), "TSLA".to_string()]
        );
        assert!(
            session
                .view_offset
                .map(|v| (v - 5.5).abs() < f32::EPSILON)
                .unwrap_or(false)
        );
        assert!(
            session
                .zoom
                .map(|v| (v - 2.5).abs() < f32::EPSILON)
                .unwrap_or(false)
        );
        assert_eq!(session.perf_mode, Some(true));
        assert_eq!(session.perf_n, Some(200000));
        assert_eq!(session.perf_step_secs, Some(60));
        assert_eq!(session.live_mode, Some(true));
        assert_eq!(session.live_pub.as_deref(), Some("tcp://127.0.0.1:5556"));
        assert_eq!(session.chunk_rep.as_deref(), Some("tcp://127.0.0.1:5557"));
        assert_eq!(session.live_source_id.as_deref(), Some("SIM"));
        assert_eq!(session.live_interval.as_deref(), Some("1s"));
    }
}
