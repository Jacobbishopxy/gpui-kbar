mod error;
mod load;
mod resample;
mod store;
mod types;

pub use error::LoadError;
pub use load::{load_csv, load_parquet};
pub use resample::{bounds, resample};
pub use store::{DuckDbStore, StorageMode, StoreError};
pub use types::{Candle, ColumnMapping, Interval, LoadOptions};

#[cfg(test)]
mod tests {
    use super::*;
    use polars::datatypes::TimeUnit;
    use polars::prelude::{
        DataFrame, DataType, Int64Chunked, IntoSeries, NamedFrom, ParquetWriter, Series,
    };
    use std::fs;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};
    use time::{OffsetDateTime, format_description::well_known::Rfc3339};

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

    #[test]
    fn bounds_and_resample() {
        let candles = vec![
            Candle {
                timestamp: OffsetDateTime::from_unix_timestamp(0).unwrap(),
                open: 1.0,
                high: 2.0,
                low: 0.5,
                close: 1.5,
                volume: 10.0,
            },
            Candle {
                timestamp: OffsetDateTime::from_unix_timestamp(30).unwrap(),
                open: 1.5,
                high: 3.0,
                low: 1.0,
                close: 2.5,
                volume: 15.0,
            },
            Candle {
                timestamp: OffsetDateTime::from_unix_timestamp(90).unwrap(),
                open: 2.0,
                high: 4.0,
                low: 1.5,
                close: 3.5,
                volume: 20.0,
            },
        ];

        assert_eq!(bounds(&candles), Some((0.5, 4.0)));

        let resampled = resample(&candles, Interval::Minute(1));
        assert_eq!(resampled.len(), 2);

        assert_eq!(resampled[0].open, 1.0);
        assert_eq!(resampled[0].high, 3.0);
        assert_eq!(resampled[0].low, 0.5);
        assert_eq!(resampled[0].close, 2.5);
        assert_eq!(resampled[0].volume, 25.0);

        assert_eq!(resampled[1].open, 2.0);
        assert_eq!(resampled[1].high, 4.0);
        assert_eq!(resampled[1].low, 1.5);
        assert_eq!(resampled[1].close, 3.5);
        assert_eq!(resampled[1].volume, 20.0);
    }
}
