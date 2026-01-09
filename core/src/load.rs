use crate::{Candle, ColumnMapping, LoadError, LoadOptions};
use polars::datatypes::TimeUnit;
use polars::prelude::PlPathRef;
use polars::prelude::*;
use std::path::Path;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

pub fn load_csv(path: impl AsRef<Path>, options: LoadOptions) -> Result<Vec<Candle>, LoadError> {
    let pl_path = PlPathRef::from_local_path(path.as_ref()).into_owned();
    let columns = &options.columns;
    let lf = LazyCsvReader::new(pl_path)
        .with_has_header(true)
        // Avoid scanning the entire file to infer types.
        .with_infer_schema_length(Some(1_024))
        // Try to parse ISO-ish timestamps eagerly (e.g. RFC3339).
        .with_try_parse_dates(true);
    let mut lf = lf.finish()?;
    ensure_columns(&mut lf, columns)?;
    let df = lf
        .select([
            col(&columns.timestamp),
            col(&columns.open),
            col(&columns.high),
            col(&columns.low),
            col(&columns.close),
            col(&columns.volume),
        ])
        .collect()?;
    parse_frame(df, &options.columns)
}

pub fn load_parquet(
    path: impl AsRef<Path>,
    options: LoadOptions,
) -> Result<Vec<Candle>, LoadError> {
    let pl_path = PlPathRef::from_local_path(path.as_ref()).into_owned();
    let columns = &options.columns;
    let mut lf = LazyFrame::scan_parquet(pl_path, ScanArgsParquet::default())?;
    ensure_columns(&mut lf, columns)?;
    let df = lf
        .select([
            col(&columns.timestamp),
            col(&columns.open),
            col(&columns.high),
            col(&columns.low),
            col(&columns.close),
            col(&columns.volume),
        ])
        .collect()?;
    parse_frame(df, &options.columns)
}

fn ensure_columns(lf: &mut LazyFrame, columns: &ColumnMapping) -> Result<(), LoadError> {
    let schema = lf.collect_schema()?;
    for required in [
        &columns.timestamp,
        &columns.open,
        &columns.high,
        &columns.low,
        &columns.close,
        &columns.volume,
    ] {
        if schema.get(required.as_str()).is_none() {
            return Err(LoadError::MissingColumn(required.clone()));
        }
    }
    Ok(())
}

fn parse_frame(df: DataFrame, columns: &ColumnMapping) -> Result<Vec<Candle>, LoadError> {
    let ts = df
        .column(&columns.timestamp)
        .map_err(|_| LoadError::MissingColumn(columns.timestamp.clone()))?;

    let open = float64_col(&df, &columns.open)?;
    let high = float64_col(&df, &columns.high)?;
    let low = float64_col(&df, &columns.low)?;
    let close = float64_col(&df, &columns.close)?;
    let volume = float64_col(&df, &columns.volume)?;
    let numeric = NumericCols {
        columns,
        open: &open,
        high: &high,
        low: &low,
        close: &close,
        volume: &volume,
    };

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
    match ts.dtype() {
        DataType::Datetime(unit, _) => {
            let unit = *unit;
            let dt = ts
                .datetime()
                .expect("polars datetime dtype should have datetime accessor")
                .clone();
            for row in 0..len {
                let ts_raw = dt
                    .phys
                    .get(row)
                    .ok_or_else(|| LoadError::UnsupportedTimestamp {
                        row,
                        value: "null".to_string(),
                    })?;
                build_row(
                    &mut candles,
                    row,
                    from_timestamp(ts_raw, unit, row)?,
                    &numeric,
                )?;
            }
        }
        DataType::Date => {
            let dates = ts
                .date()
                .expect("polars date dtype should have date accessor")
                .clone();
            for row in 0..len {
                let days = dates
                    .phys
                    .get(row)
                    .ok_or_else(|| LoadError::UnsupportedTimestamp {
                        row,
                        value: "null".to_string(),
                    })?;
                let secs = days as i64 * 86_400;
                let timestamp = OffsetDateTime::from_unix_timestamp(secs).map_err(|_| {
                    LoadError::UnsupportedTimestamp {
                        row,
                        value: format!("days since epoch: {days}"),
                    }
                })?;
                build_row(&mut candles, row, timestamp, &numeric)?;
            }
        }
        DataType::Int64 => {
            let secs = ts
                .i64()
                .expect("polars i64 dtype should have i64 accessor")
                .clone();
            for row in 0..len {
                let secs = secs
                    .get(row)
                    .ok_or_else(|| LoadError::UnsupportedTimestamp {
                        row,
                        value: "null".to_string(),
                    })?;
                let timestamp = OffsetDateTime::from_unix_timestamp(secs).map_err(|_| {
                    LoadError::UnsupportedTimestamp {
                        row,
                        value: secs.to_string(),
                    }
                })?;
                build_row(&mut candles, row, timestamp, &numeric)?;
            }
        }
        DataType::String => {
            let strings = ts
                .str()
                .expect("polars string dtype should have string accessor")
                .clone();
            for row in 0..len {
                let s = strings
                    .get(row)
                    .ok_or_else(|| LoadError::UnsupportedTimestamp {
                        row,
                        value: "null".to_string(),
                    })?;
                let timestamp = OffsetDateTime::parse(s, &Rfc3339).map_err(|err| {
                    LoadError::UnsupportedTimestamp {
                        row,
                        value: format!("{s} ({err})"),
                    }
                })?;
                build_row(&mut candles, row, timestamp, &numeric)?;
            }
        }
        _other => {
            // Preserve legacy behavior for unexpected types.
            for row in 0..len {
                let timestamp = to_datetime(ts.get(row)?, row)?;
                build_row(&mut candles, row, timestamp, &numeric)?;
            }
        }
    }

    Ok(candles)
}

struct NumericCols<'a> {
    columns: &'a ColumnMapping,
    open: &'a Float64Chunked,
    high: &'a Float64Chunked,
    low: &'a Float64Chunked,
    close: &'a Float64Chunked,
    volume: &'a Float64Chunked,
}

fn float64_col(df: &DataFrame, name: &str) -> Result<Float64Chunked, LoadError> {
    let s = df
        .column(name)
        .map_err(|_| LoadError::MissingColumn(name.to_string()))?;
    let cast = match s.dtype() {
        DataType::Float64 => s.clone(),
        _ => s.cast(&DataType::Float64)?,
    };
    Ok(cast.f64().expect("cast above ensures Float64").clone())
}

fn build_row(
    out: &mut Vec<Candle>,
    row: usize,
    timestamp: OffsetDateTime,
    numeric: &NumericCols<'_>,
) -> Result<(), LoadError> {
    let open = numeric
        .open
        .get(row)
        .ok_or_else(|| LoadError::InvalidNumber {
            column: numeric.columns.open.clone(),
            row,
            value: "null".to_string(),
        })?;
    let high = numeric
        .high
        .get(row)
        .ok_or_else(|| LoadError::InvalidNumber {
            column: numeric.columns.high.clone(),
            row,
            value: "null".to_string(),
        })?;
    let low = numeric
        .low
        .get(row)
        .ok_or_else(|| LoadError::InvalidNumber {
            column: numeric.columns.low.clone(),
            row,
            value: "null".to_string(),
        })?;
    let close = numeric
        .close
        .get(row)
        .ok_or_else(|| LoadError::InvalidNumber {
            column: numeric.columns.close.clone(),
            row,
            value: "null".to_string(),
        })?;
    let volume = numeric
        .volume
        .get(row)
        .ok_or_else(|| LoadError::InvalidNumber {
            column: numeric.columns.volume.clone(),
            row,
            value: "null".to_string(),
        })?;

    if low > high {
        return Err(LoadError::InvertedRange { row, low, high });
    }

    out.push(Candle {
        timestamp,
        open,
        high,
        low,
        close,
        volume,
    });

    Ok(())
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
        AnyValue::Int64(secs) => {
            OffsetDateTime::from_unix_timestamp(secs).map_err(|_| LoadError::UnsupportedTimestamp {
                row,
                value: secs.to_string(),
            })
        }
        AnyValue::String(s) => {
            OffsetDateTime::parse(s, &Rfc3339).map_err(|err| LoadError::UnsupportedTimestamp {
                row,
                value: format!("{s} ({err})"),
            })
        }
        AnyValue::StringOwned(s) => to_datetime(AnyValue::String(&s), row),
        other => Err(LoadError::UnsupportedTimestamp {
            row,
            value: format!("{other:?}"),
        }),
    }
}

fn from_timestamp(value: i64, unit: TimeUnit, row: usize) -> Result<OffsetDateTime, LoadError> {
    let nanos = match unit {
        TimeUnit::Nanoseconds => value,
        TimeUnit::Microseconds => value * 1_000,
        TimeUnit::Milliseconds => value * 1_000_000,
    };
    OffsetDateTime::from_unix_timestamp_nanos(nanos as i128).map_err(|_| {
        LoadError::UnsupportedTimestamp {
            row,
            value: format!("{value} ({unit:?})"),
        }
    })
}
