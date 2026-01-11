use duckdb::Connection;
use std::path::Path;

// DuckDB's Windows build depends on Restart Manager symbols; linking here
// ensures we pull in the right system library when building this example.
#[allow(non_snake_case)]
mod restart_manager_link {
    #[link(name = "Rstrtmgr")]
    unsafe extern "system" {}
}

fn main() -> anyhow::Result<()> {
    let config_path = Path::new("data/config.duckdb");
    let legacy_path = Path::new("data/cache.duckdb");
    let path = if config_path.exists() {
        config_path
    } else {
        legacy_path
    };
    if !path.exists() {
        eprintln!(
            "cache file not found at {} (expected config at {}, legacy at {})",
            path.display(),
            config_path.display(),
            legacy_path.display()
        );
        return Ok(());
    }

    let conn = Connection::open(path)?;

    println!("# session_state");
    let mut stmt = conn.prepare("SELECT key, value FROM session_state ORDER BY key ASC")?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let key: String = row.get(0)?;
        let value: String = row.get(1)?;
        println!("{key} = {value}");
    }

    println!("\n# watchlist");
    let mut stmt = conn.prepare("SELECT symbol FROM watchlist ORDER BY symbol ASC")?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let sym: String = row.get(0)?;
        println!("{sym}");
    }

    Ok(())
}
