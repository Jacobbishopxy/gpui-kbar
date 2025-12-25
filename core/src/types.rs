use time::{Duration, OffsetDateTime};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interval {
    Second(u32),
    Minute(u32),
    Hour(u32),
    Day(u32),
}

impl Interval {
    pub fn as_duration(&self) -> Duration {
        match *self {
            Interval::Second(n) => Duration::seconds(n.into()),
            Interval::Minute(n) => Duration::minutes(n.into()),
            Interval::Hour(n) => Duration::hours(n.into()),
            Interval::Day(n) => Duration::days(n.into()),
        }
    }
}
