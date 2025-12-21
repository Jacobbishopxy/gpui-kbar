use crate::{Candle, Interval};
use time::{Duration, OffsetDateTime};

pub fn bounds(candles: &[Candle]) -> Option<(f64, f64)> {
    if candles.is_empty() {
        return None;
    }
    let mut min = f64::MAX;
    let mut max = f64::MIN;
    for c in candles {
        min = min.min(c.low);
        max = max.max(c.high);
    }
    Some((min, max))
}

pub fn resample(candles: &[Candle], interval: Interval) -> Vec<Candle> {
    if candles.is_empty() {
        return Vec::new();
    }

    let duration = interval.as_duration();
    let mut out: Vec<Candle> = Vec::new();
    let mut bucket_start = align_timestamp(candles[0].timestamp, duration);
    let mut bucket_end = bucket_start + duration;

    let mut acc_open = candles[0].open;
    let mut acc_high = candles[0].high;
    let mut acc_low = candles[0].low;
    let mut acc_close = candles[0].close;
    let mut acc_volume = candles[0].volume;

    for c in candles.iter().skip(1) {
        if c.timestamp < bucket_end {
            acc_high = acc_high.max(c.high);
            acc_low = acc_low.min(c.low);
            acc_close = c.close;
            acc_volume += c.volume;
        } else {
            out.push(Candle {
                timestamp: bucket_start,
                open: acc_open,
                high: acc_high,
                low: acc_low,
                close: acc_close,
                volume: acc_volume,
            });

            bucket_start = align_timestamp(c.timestamp, duration);
            bucket_end = bucket_start + duration;
            acc_open = c.open;
            acc_high = c.high;
            acc_low = c.low;
            acc_close = c.close;
            acc_volume = c.volume;
        }
    }

    out.push(Candle {
        timestamp: bucket_start,
        open: acc_open,
        high: acc_high,
        low: acc_low,
        close: acc_close,
        volume: acc_volume,
    });

    out
}

fn align_timestamp(ts: OffsetDateTime, duration: Duration) -> OffsetDateTime {
    if duration.is_zero() {
        return ts;
    }
    let nanos = duration.whole_nanoseconds();
    let ts_nanos = ts.unix_timestamp_nanos();
    let bucket = ts_nanos - (ts_nanos % nanos);
    OffsetDateTime::from_unix_timestamp_nanos(bucket).unwrap_or(ts)
}
