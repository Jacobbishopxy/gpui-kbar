use std::path::{Path, PathBuf};

use duckdb::{Connection, params, params_from_iter};
use thiserror::Error;
use time::{
    OffsetDateTime,
    error::{Format, Parse},
    format_description::well_known::Rfc3339,
};

use crate::Candle;

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

pub struct DuckDbStore {
    mode: StorageMode,
    disk_path: Option<PathBuf>,
    memory: Option<Connection>,
    disk: Option<Connection>,
}

impl DuckDbStore {
    pub fn mode(&self) -> StorageMode {
        self.mode
    }

    pub fn new(path: impl AsRef<Path>, mode: StorageMode) -> Result<Self, StoreError> {
        let mut store = Self {
            mode,
            disk_path: Some(path.as_ref().to_path_buf()),
            memory: None,
            disk: None,
        };

        store.reconfigure(mode, Some(path))?;

        Ok(store)
    }

    pub fn set_disk_path(&mut self, path: impl AsRef<Path>) {
        self.disk_path = Some(path.as_ref().to_path_buf());
    }

    pub fn set_mode(&mut self, mode: StorageMode) -> Result<(), StoreError> {
        self.reconfigure(mode, Option::<PathBuf>::None)?;
        Ok(())
    }

    fn open_memory(&self) -> Result<Connection, StoreError> {
        let conn = Connection::open_in_memory()?;
        init_schema(&conn)?;
        Ok(conn)
    }

    fn open_disk(&self, path: &Path) -> Result<Connection, StoreError> {
        let conn = Connection::open(path)?;
        init_schema(&conn)?;
        Ok(conn)
    }

    fn reconfigure(
        &mut self,
        mode: StorageMode,
        override_path: Option<impl AsRef<Path>>,
    ) -> Result<(), StoreError> {
        let disk_path = override_path
            .map(|p| p.as_ref().to_path_buf())
            .or_else(|| self.disk_path.clone());

        let disk_required = matches!(mode, StorageMode::Disk | StorageMode::Both);
        if disk_required && disk_path.is_none() {
            return Err(StoreError::MissingDiskPath);
        }

        self.memory = None;
        self.disk = None;

        match mode {
            StorageMode::Memory => {
                self.memory = Some(self.open_memory()?);
            }
            StorageMode::Disk => {
                let path = disk_path.expect("checked above");
                self.disk_path = Some(path.clone());
                self.disk = Some(self.open_disk(&path)?);
            }
            StorageMode::Both => {
                let path = disk_path.expect("checked above");
                self.disk_path = Some(path.clone());
                self.memory = Some(self.open_memory()?);
                self.disk = Some(self.open_disk(&path)?);
            }
        }

        self.mode = mode;

        Ok(())
    }

    fn connections(&self) -> impl Iterator<Item = &Connection> {
        self.memory.iter().chain(self.disk.iter())
    }

    pub fn write_candles(&self, symbol: &str, candles: &[Candle]) -> Result<(), StoreError> {
        if self.connections().count() == 0 {
            return Err(StoreError::NoBackend);
        }

        for conn in self.connections() {
            let mut stmt = conn.prepare(
                "INSERT INTO candles (symbol, timestamp, open, high, low, close, volume)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )?;
            for candle in candles {
                let ts = candle.timestamp.format(&Rfc3339)?;
                stmt.execute(params![
                    symbol,
                    ts,
                    candle.open,
                    candle.high,
                    candle.low,
                    candle.close,
                    candle.volume
                ])?;
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
        for conn in self.connections() {
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
                return Ok(result);
            }
        }

        Ok(result)
    }

    pub fn write_indicator_values(
        &self,
        symbol: &str,
        indicator: &str,
        values: &[(OffsetDateTime, f64)],
    ) -> Result<(), StoreError> {
        if self.connections().count() == 0 {
            return Err(StoreError::NoBackend);
        }

        for conn in self.connections() {
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
        for conn in self.connections() {
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
        if self.connections().count() == 0 {
            return Err(StoreError::NoBackend);
        }
        for conn in self.connections() {
            conn.execute(
                "INSERT INTO session_state(key, value) VALUES (?, ?)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![key, value],
            )?;
        }
        Ok(())
    }

    pub fn get_session_value(&self, key: &str) -> Result<Option<String>, StoreError> {
        for conn in self.connections() {
            let mut stmt = conn.prepare("SELECT value FROM session_state WHERE key = ? LIMIT 1")?;
            let mut rows = stmt.query([key])?;
            if let Some(row) = rows.next()? {
                let v: String = row.get(0)?;
                return Ok(Some(v));
            }
        }
        Ok(None)
    }
}

fn init_schema(conn: &Connection) -> Result<(), StoreError> {
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

        CREATE TABLE IF NOT EXISTS session_state (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        ",
    )?;
    Ok(())
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
}
