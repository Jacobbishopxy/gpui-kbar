use anyhow::Result;

fn main() -> Result<()> {
    fn parse_arg_u64(name: &str) -> Option<u64> {
        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            if arg == name {
                return args.next().and_then(|v| v.parse::<u64>().ok());
            }
        }
        None
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

    let initial_symbol = parse_arg_string("--symbol");

    let step_secs = parse_arg_u64("--step-secs").map(|v| v as i64);
    let preset = parse_arg_string("--preset");
    let n_from_preset = preset.as_deref().and_then(preset_n);
    let n = n_from_preset.or_else(|| parse_arg_u64("--n").map(|v| v as usize));

    let perf = n.map(|n| ui::PerfOptions {
        n,
        step_secs: step_secs.unwrap_or(60),
    });

    ui::launch_runtime_with_options(ui::RuntimeOptions {
        initial_symbol,
        perf,
    });
    Ok(())
}
