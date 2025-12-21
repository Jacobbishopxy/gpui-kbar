use polars::datatypes::TimeUnit;
use polars::prelude::*;
use std::path::Path;
use thiserror::Error;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

#[derive(Debug, Clone, PartialEq)]
pub struct Candle {
    pub timestamp: OffsetDateTime,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

#[derive(Debug, Clone)]
pub struct ColumnMapping {
    pub timestamp: String,
    pub open: String,
    pub high: String,
    pub low: String,
    pub close: String,
    pub volume: String,
}

impl Default for ColumnMapping {
    fn default() -> Self {
        Self {
            timestamp: "timestamp".into(),
            open: "open".into(),
            high: "high".into(),
            low: "low".into(),
            close: "close".into(),
            volume: "volume".into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LoadOptions {
    pub columns: ColumnMapping,
}

impl Default for LoadOptions {
    fn default() -> Self {
        Self {
            columns: ColumnMapping::default(),
        }
    }
}

#[derive(Debug, Error)]
pub enum LoadError {
    #[error("column '{0}' not found")]
    MissingColumn(String),
    #[error("column lengths are inconsistent")]
    LengthMismatch,
    #[error("low > high at row {row} (low={low}, high={high})")]
    InvertedRange { row: usize, low: f64, high: f64 },
    #[error("unsupported timestamp at row {row}: {value}")]
    UnsupportedTimestamp { row: usize, value: String },
    #[error("invalid numeric value in column '{column}' at row {row}: {value}")]
    InvalidNumber {
        column: String,
        row: usize,
        value: String,
    },
    #[error(transparent)]
    Polars(#[from] PolarsError),
}

pub fn load_csv(path: impl AsRef<Path>, options: LoadOptions) -> Result<Vec<Candle>, LoadError> {
    let pl_path = PlPathRef::from_local_path(path.as_ref()).into_owned();
    let lf = LazyCsvReader::new(pl_path).with_has_header(true);
    let df = lf.finish()?.collect()?;
    parse_frame(df, &options.columns)
}

pub fn load_parquet(
    path: impl AsRef<Path>,
    options: LoadOptions,
) -> Result<Vec<Candle>, LoadError> {
    let pl_path = PlPathRef::from_local_path(path.as_ref()).into_owned();
    let lf = LazyFrame::scan_parquet(pl_path, ScanArgsParquet::default())?;
    let df = lf.collect()?;
    parse_frame(df, &options.columns)
}

fn parse_frame(df: DataFrame, columns: &ColumnMapping) -> Result<Vec<Candle>, LoadError> {
    let ts = df
        .column(&columns.timestamp)
        .map_err(|_| LoadError::MissingColumn(columns.timestamp.clone()))?;
    let open = df
        .column(&columns.open)
        .map_err(|_| LoadError::MissingColumn(columns.open.clone()))?;
    let high = df
        .column(&columns.high)
        .map_err(|_| LoadError::MissingColumn(columns.high.clone()))?;
    let low = df
        .column(&columns.low)
        .map_err(|_| LoadError::MissingColumn(columns.low.clone()))?;
    let close = df
        .column(&columns.close)
        .map_err(|_| LoadError::MissingColumn(columns.close.clone()))?;
    let volume = df
        .column(&columns.volume)
        .map_err(|_| LoadError::MissingColumn(columns.volume.clone()))?;

    let len = ts.len();
    if open.len() != len
        || high.len() != len
        || low.len() != len
        || close.len() != len
        || volume.len() != len
    {
        return Err(LoadError::LengthMismatch);
    }

    let mut candles = Vec::with_capacity(len);
    for idx in 0..len {
        let timestamp = to_datetime(ts.get(idx)?, idx)?;
        let open = to_f64(open.get(idx)?, &columns.open, idx)?;
        let high = to_f64(high.get(idx)?, &columns.high, idx)?;
        let low = to_f64(low.get(idx)?, &columns.low, idx)?;
        let close = to_f64(close.get(idx)?, &columns.close, idx)?;
        let volume = to_f64(volume.get(idx)?, &columns.volume, idx)?;

        if low > high {
            return Err(LoadError::InvertedRange { row: idx, low, high });
        }

        candles.push(Candle {
            timestamp,
            open,
            high,
            low,
            close,
            volume,
        });
    }

    Ok(candles)
}

fn to_datetime(value: AnyValue, row: usize) -> Result<OffsetDateTime, LoadError> {
    match value {
        AnyValue::Datetime(ts, unit, _) => from_timestamp(ts, unit, row),
        AnyValue::Date(days) => {
            let secs = days as i64 * 86_400;
            OffsetDateTime::from_unix_timestamp(secs).map_err(|_| LoadError::UnsupportedTimestamp {
                row,
                value: format!("days since epoch: {days}"),
            })
        }
        AnyValue::Int64(secs) => OffsetDateTime::from_unix_timestamp(secs).map_err(|_| {
            LoadError::UnsupportedTimestamp {
                row,
                value: secs.to_string(),
            }
        }),
        AnyValue::String(s) => OffsetDateTime::parse(s, &Rfc3339).map_err(|err| {
            LoadError::UnsupportedTimestamp {
                row,
                value: format!("{s} ({err})"),
            }
        }),
        AnyValue::StringOwned(s) => to_datetime(AnyValue::String(&s), row),
        other => Err(LoadError::UnsupportedTimestamp {
            row,
            value: format!("{other:?}"),
        }),
    }
}

fn from_timestamp(
    value: i64,
    unit: TimeUnit,
    row: usize,
) -> Result<OffsetDateTime, LoadError> {
    let nanos = match unit {
        TimeUnit::Nanoseconds => value,
        TimeUnit::Microseconds => value * 1_000,
        TimeUnit::Milliseconds => value * 1_000_000,
    };
    OffsetDateTime::from_unix_timestamp_nanos(nanos as i128).map_err(|_| LoadError::UnsupportedTimestamp {
        row,
        value: format!("{value} ({unit:?})"),
    })
}

fn to_f64(value: AnyValue, column: &str, row: usize) -> Result<f64, LoadError> {
    match value {
        AnyValue::Float64(v) => Ok(v),
        AnyValue::Float32(v) => Ok(v as f64),
        AnyValue::Int64(v) => Ok(v as f64),
        AnyValue::Int32(v) => Ok(v as f64),
        AnyValue::UInt64(v) => Ok(v as f64),
        AnyValue::UInt32(v) => Ok(v as f64),
        AnyValue::String(s) => s.parse::<f64>().map_err(|_| LoadError::InvalidNumber {
            column: column.to_string(),
            row,
            value: s.to_string(),
        }),
        AnyValue::StringOwned(s) => to_f64(AnyValue::String(&s), column, row),
        other => Err(LoadError::InvalidNumber {
            column: column.to_string(),
            row,
            value: format!("{other:?}"),
        }),
    }
}

pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::prelude::{DataFrame, DataType, Int64Chunked, ParquetWriter, Series};
    use std::fs;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(ext: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("gpui-kbar-{nonce}.{ext}"))
    }

    fn sample_csv() -> String {
        [
            "timestamp,open,high,low,close,volume",
            "2024-01-01T00:00:00Z,1.0,2.0,0.5,1.5,100",
            "2024-01-01T00:01:00Z,1.5,2.5,1.0,2.0,150",
            "2024-01-01T00:02:00Z,2.0,3.0,1.5,2.5,200",
        ]
        .join("\n")
    }

    #[test]
    fn load_csv_with_defaults() {
        let path = temp_path("csv");
        fs::write(&path, sample_csv()).unwrap();

        let candles = load_csv(&path, LoadOptions::default()).unwrap();
        fs::remove_file(&path).ok();

        assert_eq!(candles.len(), 3);
        assert_eq!(candles[0].open, 1.0);
        assert_eq!(candles[1].close, 2.0);
        assert_eq!(
            candles[2].timestamp,
            OffsetDateTime::parse("2024-01-01T00:02:00Z", &Rfc3339).unwrap()
        );
    }

    #[test]
    fn errors_on_missing_column() {
        let path = temp_path("csv");
        fs::write(
            &path,
            "timestamp,open,high,low,close\n2024-01-01T00:00:00Z,1,2,0.5,1.5\n",
        )
        .unwrap();

        let err = load_csv(&path, LoadOptions::default()).unwrap_err();
        fs::remove_file(&path).ok();

        match err {
            LoadError::MissingColumn(name) => assert_eq!(name, "volume"),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    fn write_parquet_fixture(path: &Path) {
        let ts_ms: Series = Int64Chunked::new(
            "timestamp".into(),
            &[1_704_300_000_000i64, 1_704_300_060_000, 1_704_300_120_000],
        )
        .into_series()
        .cast(&DataType::Datetime(TimeUnit::Milliseconds, None))
        .unwrap();

        let mut df = DataFrame::new(vec![
            ts_ms.into(),
            Series::new("open".into(), &[1.0_f64, 1.5, 2.0]).into(),
            Series::new("high".into(), &[2.0_f64, 2.5, 3.0]).into(),
            Series::new("low".into(), &[0.5_f64, 1.0, 1.5]).into(),
            Series::new("close".into(), &[1.5_f64, 2.0, 2.5]).into(),
            Series::new("volume".into(), &[100.0_f64, 150.0, 200.0]).into(),
        ])
        .unwrap();

        let mut file = fs::File::create(path).unwrap();
        ParquetWriter::new(&mut file).finish(&mut df).unwrap();
    }

    #[test]
    fn load_parquet_datetime_series() {
        let path = temp_path("parquet");
        write_parquet_fixture(&path);

        let candles = load_parquet(&path, LoadOptions::default()).unwrap();
        fs::remove_file(&path).ok();

        assert_eq!(candles.len(), 3);
        assert_eq!(candles[0].high, 2.0);
        assert_eq!(
            candles[0].timestamp,
            OffsetDateTime::from_unix_timestamp(1_704_300_000).unwrap()
        );
        assert_eq!(candles[2].volume, 200.0);
    }
}
