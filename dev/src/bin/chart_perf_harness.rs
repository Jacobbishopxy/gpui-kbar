use kbar_core::Candle;
use time::{Duration, OffsetDateTime};
use ui::{ChartMeta, launch_chart};

fn parse_arg_u64(name: &str, default: u64) -> u64 {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == name {
            if let Some(value) = args.next() {
                if let Ok(v) = value.parse::<u64>() {
                    return v;
                }
            }
        }
    }
    default
}

fn parse_arg_string(name: &str) -> Option<String> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == name {
            return args.next();
        }
    }
    None
}

fn preset_n(preset: &str) -> Option<usize> {
    match preset.to_ascii_lowercase().as_str() {
        "50k" | "small" => Some(50_000),
        "200k" | "medium" => Some(200_000),
        "1m" | "1000000" | "large" => Some(1_000_000),
        _ => None,
    }
}

fn generate_candles(n: usize, step_secs: i64) -> Vec<Candle> {
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

fn main() {
    let step_secs = parse_arg_u64("--step-secs", 60) as i64;
    let preset = parse_arg_string("--preset");
    let n = preset
        .as_deref()
        .and_then(preset_n)
        .unwrap_or_else(|| parse_arg_u64("--n", 200_000) as usize);

    let candles = generate_candles(n, step_secs);
    let meta = ChartMeta {
        source: format!("__PERF__ n={n} step={step_secs}s"),
        initial_interval: None,
    };

    launch_chart(Ok(candles), meta);
}
