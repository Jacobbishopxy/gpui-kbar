use crate::{Candle, ColumnMapping, LoadError, LoadOptions};
use polars::datatypes::TimeUnit;
use polars::prelude::*;
use polars::prelude::PlPathRef;
use std::path::Path;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

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
