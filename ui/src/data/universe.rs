use std::path::Path;

use csv::StringRecord;

#[derive(Clone, Debug)]
pub struct SymbolSearchEntry {
    pub filters: Vec<String>,
    pub badge: String,
    pub symbol: String,
    pub name: String,
    pub market: String,
    pub venue: String,
}

fn parse_filters(raw: &str) -> Vec<String> {
    raw.split([';', ','])
        .filter_map(|p| {
            let trimmed = p.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        })
        .collect()
}

pub fn load_universe(path: &str) -> Result<Vec<SymbolSearchEntry>, String> {
    let csv_path = Path::new(path);
    if !csv_path.exists() {
        return Err(format!("universe file not found at {path}"));
    }

    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_path(csv_path)
        .map_err(|e| format!("failed to read universe {path}: {e}"))?;

    let headers = reader
        .headers()
        .map_err(|e| format!("failed to read universe headers: {e}"))?
        .clone();

    let mut entries = Vec::new();
    for row in reader.records() {
        let record = row.map_err(|e| format!("failed to parse universe row: {e}"))?;
        if let Some(entry) = record_to_entry(&headers, &record) {
            entries.push(entry);
        }
    }
    if entries.is_empty() {
        return Err("universe has no rows".to_string());
    }
    Ok(entries)
}

fn record_to_entry(headers: &StringRecord, record: &StringRecord) -> Option<SymbolSearchEntry> {
    let get = |key: &str| -> String {
        headers
            .iter()
            .position(|h| h.eq_ignore_ascii_case(key))
            .and_then(|idx| record.get(idx))
            .unwrap_or_default()
            .trim()
            .to_string()
    };

    let symbol = get("symbol");
    if symbol.is_empty() {
        return None;
    }

    Some(SymbolSearchEntry {
        filters: parse_filters(&get("filters")),
        badge: get("badge"),
        symbol,
        name: get("name"),
        market: get("market"),
        venue: get("venue"),
    })
}
