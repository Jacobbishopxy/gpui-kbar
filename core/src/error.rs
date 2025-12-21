use polars::prelude::PolarsError;
use thiserror::Error;

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
