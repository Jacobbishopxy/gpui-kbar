use std::fs::{OpenOptions, create_dir_all};
use std::io::{LineWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

const LOADING_LOG_DIR: &str = "tmp";
const LOADING_LOG_BASENAME: &str = "loading_spinner";

fn log_path() -> PathBuf {
    let ts_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let pid = std::process::id();
    let filename = format!("{LOADING_LOG_BASENAME}_{ts_ms}_pid{pid}.log");
    Path::new(LOADING_LOG_DIR).join(filename)
}

fn loading_log_file() -> Option<&'static Mutex<LineWriter<std::fs::File>>> {
    static LOG_FILE: OnceLock<Option<Mutex<LineWriter<std::fs::File>>>> = OnceLock::new();

    LOG_FILE
        .get_or_init(|| {
            let path = log_path();
            if let Some(parent) = path.parent() {
                if let Err(err) = create_dir_all(parent) {
                    eprintln!("[log] failed to create log dir {:?}: {err}", parent);
                    return None;
                }
            }
            match OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&path)
            {
                Ok(file) => Some(Mutex::new(LineWriter::new(file))),
                Err(err) => {
                    eprintln!("[log] failed to open log file {:?}: {err}", path);
                    None
                }
            }
        })
        .as_ref()
}

/// Log loading-related diagnostics to stdout and a persisted file.
pub fn log_loading(message: impl AsRef<str>) {
    let message = message.as_ref();
    println!("{message}");
    if let Some(file) = loading_log_file() {
        if let Ok(mut guard) = file.lock() {
            let _ = writeln!(guard, "{message}");
        }
    }
}
