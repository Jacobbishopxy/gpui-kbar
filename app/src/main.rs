use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use core::{Interval, LoadOptions, load_csv, load_parquet};

mod input;
use input::{InputFormat, detect_format};

#[derive(Parser, Debug)]
#[command(name = "gpui-kbar")]
struct Args {
    /// Path to the CSV or Parquet file containing OHLCV data.
    path: PathBuf,

    /// Explicitly set the file format. If omitted, inferred from extension.
    #[arg(long, value_enum)]
    format: Option<InputFormat>,

    /// Resample interval (e.g. 3s, 10s, 1m, 1h, 1d). If omitted, raw data is used.
    #[arg(long, value_parser = parse_interval)]
    interval: Option<Interval>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let load_result: Result<Vec<_>, String> = (|| {
        let format = args
            .format
            .or_else(|| detect_format(&args.path))
            .ok_or_else(|| "could not determine file format (use --format)".to_string())?;

        let candles = match format {
            InputFormat::Csv => load_csv(&args.path, LoadOptions::default()),
            InputFormat::Parquet => load_parquet(&args.path, LoadOptions::default()),
        }
        .map_err(|e| format!("failed to load {}: {e}", args.path.display()))?;

        if candles.is_empty() {
            return Err(format!("no candles loaded from {}", args.path.display()));
        }

        Ok(candles)
    })();

    let meta = ui::ChartMeta {
        source: args.path.display().to_string(),
        initial_interval: args.interval,
    };

    ui::launch_chart(load_result, meta);
    Ok(())
}

fn parse_interval(raw: &str) -> Result<Interval, String> {
    let trimmed = raw.trim().to_ascii_lowercase();
    if trimmed.is_empty() {
        return Err("interval cannot be empty".into());
    }

    let (number, unit) = trimmed.split_at(trimmed.len().saturating_sub(1));

    let amount: u32 = number
        .parse()
        .map_err(|_| format!("invalid interval amount: {number}"))?;

    match unit {
        "s" => Ok(Interval::Second(amount)),
        "m" => Ok(Interval::Minute(amount)),
        "h" => Ok(Interval::Hour(amount)),
        "d" => Ok(Interval::Day(amount)),
        other => Err(format!("unsupported interval unit: {other} (use s/m/h/d)")),
    }
}
