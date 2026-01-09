use ui::perf::{PerfSpec, generate_perf_candles, perf_source};
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

fn main() {
    let step_secs = parse_arg_u64("--step-secs", 60) as i64;
    let preset = parse_arg_string("--preset");
    let n = preset
        .as_deref()
        .and_then(preset_n)
        .unwrap_or_else(|| parse_arg_u64("--n", 200_000) as usize);

    let spec = PerfSpec { n, step_secs };
    let candles = generate_perf_candles(spec);
    let meta = ChartMeta {
        source: perf_source(spec),
        initial_interval: None,
    };

    launch_chart(Ok(candles), meta);
}
