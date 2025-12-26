use std::path::Path;

use clap::ValueEnum;

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum InputFormat {
    Csv,
    Parquet,
}

pub fn detect_format(path: &Path) -> Option<InputFormat> {
    let ext = path.extension()?.to_string_lossy().to_ascii_lowercase();
    match ext.as_str() {
        "csv" => Some(InputFormat::Csv),
        "parquet" | "parq" => Some(InputFormat::Parquet),
        _ => None,
    }
}
