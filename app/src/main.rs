use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::{Parser, ValueEnum};
use core::{Interval, LoadOptions, load_csv, load_parquet};

#[derive(Parser, Debug)]
#[command(name = "gpui-kbar")]
struct Args {
    /// Path to the CSV or Parquet file containing OHLCV data.
    path: PathBuf,

    /// Explicitly set the file format. If omitted, inferred from extension.
    #[arg(long, value_enum)]
    format: Option<InputFormat>,

    /// Resample interval (e.g. 1m, 5m, 1h, 1d). If omitted, raw data is used.
    #[arg(long, value_parser = parse_interval)]
    interval: Option<Interval>,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum InputFormat {
    Csv,
    Parquet,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let format = args
        .format
        .or_else(|| detect_format(&args.path))
        .context("could not determine file format (use --format)")?;

    let candles = match format {
        InputFormat::Csv => load_csv(&args.path, LoadOptions::default()),
        InputFormat::Parquet => load_parquet(&args.path, LoadOptions::default()),
    }
    .with_context(|| format!("failed to load {}", args.path.display()))?;

    if candles.is_empty() {
        bail!("no candles loaded from {}", args.path.display());
    }

    let meta = ui::ChartMeta {
        source: args.path.display().to_string(),
        initial_interval: args.interval,
    };

    ui::launch_chart(candles, meta);
    Ok(())
}

fn detect_format(path: &Path) -> Option<InputFormat> {
    let ext = path.extension()?.to_string_lossy().to_ascii_lowercase();
    match ext.as_str() {
        "csv" => Some(InputFormat::Csv),
        "parquet" | "parq" => Some(InputFormat::Parquet),
        _ => None,
    }
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
        "m" => Ok(Interval::Minute(amount)),
        "h" => Ok(Interval::Hour(amount)),
        "d" => Ok(Interval::Day(amount)),
        other => Err(format!("unsupported interval unit: {other} (use m/h/d)")),
    }
}
