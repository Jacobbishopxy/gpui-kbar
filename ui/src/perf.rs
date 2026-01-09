use core::Candle;
use time::{Duration, OffsetDateTime};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PerfSpec {
    pub n: usize,
    pub step_secs: i64,
}

impl PerfSpec {
    pub fn normalized(self) -> Self {
        Self {
            n: self.n.max(1),
            step_secs: self.step_secs.max(1),
        }
    }
}

pub fn perf_source(spec: PerfSpec) -> String {
    let spec = spec.normalized();
    format!("__PERF__ n={} step={}s", spec.n, spec.step_secs)
}

fn parse_field_usize(source: &str, key: &str) -> Option<usize> {
    let idx = source.find(key)?;
    let rest = &source[idx + key.len()..];
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    digits.parse().ok()
}

fn parse_field_i64(source: &str, key: &str) -> Option<i64> {
    let idx = source.find(key)?;
    let rest = &source[idx + key.len()..];
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    digits.parse().ok()
}

pub fn parse_perf_source(source: &str) -> Option<PerfSpec> {
    if !source.starts_with("__PERF__") {
        return None;
    }
    let n = parse_field_usize(source, "n=")?;
    let step_secs = parse_field_i64(source, "step=")?;
    Some(PerfSpec { n, step_secs }.normalized())
}

pub fn format_perf_n(n: usize) -> String {
    if n >= 1_000_000 && n % 1_000_000 == 0 {
        format!("{}M", n / 1_000_000)
    } else if n >= 1_000 && n % 1_000 == 0 {
        format!("{}k", n / 1_000)
    } else {
        n.to_string()
    }
}

pub fn perf_label(spec: PerfSpec) -> String {
    let spec = spec.normalized();
    format!("Perf {}", format_perf_n(spec.n))
}

pub fn generate_perf_candles(spec: PerfSpec) -> Vec<Candle> {
    let spec = spec.normalized();
    let n = spec.n;
    let step_secs = spec.step_secs;

    let mut state = 0x1234_5678_9abc_def0u64;
    let mut next_f64 = || {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let bits = (state >> 12) | 0x3ff0_0000_0000_0000;
        let f = f64::from_bits(bits) - 1.0;
        f.clamp(0.0, 1.0)
    };

    let start_ts =
        OffsetDateTime::now_utc() - Duration::seconds(step_secs.saturating_mul(n as i64));

    let mut price = 100.0_f64;
    let mut candles = Vec::with_capacity(n);
    for i in 0..n {
        let t = start_ts + Duration::seconds(step_secs.saturating_mul(i as i64));
        let delta = (next_f64() - 0.5) * 0.8;
        let open = price;
        let close = price + delta;
        let high = open.max(close) + next_f64() * 0.4;
        let low = open.min(close) - next_f64() * 0.4;
        let volume = (next_f64() * 1500.0).max(1.0);
        candles.push(Candle {
            timestamp: t,
            open,
            high,
            low,
            close,
            volume,
        });
        price = close.max(1.0);
    }

    candles
}
