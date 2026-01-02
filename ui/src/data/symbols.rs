use std::{collections::HashMap, path::Path};

use csv::StringRecord;

#[derive(Clone, Debug)]
pub struct SymbolMeta {
    pub symbol: String,
    pub name: String,
    pub source: String,
    pub exchange: String,
    pub badge: String,
    pub market: String,
    pub filters: Vec<String>,
    pub venue: String,
}

pub fn load_symbols(path: &str) -> Result<HashMap<String, SymbolMeta>, String> {
    let csv_path = Path::new(path);
    if !csv_path.exists() {
        return Err(format!("symbol mapping file not found at {path}"));
    }

    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_path(csv_path)
        .map_err(|e| format!("failed to read symbols csv {path}: {e}"))?;

    let headers = reader
        .headers()
        .map_err(|e| format!("failed to read symbols header: {e}"))?
        .clone();

    let mut out = HashMap::new();
    for record in reader.records() {
        let record = record.map_err(|e| format!("failed to parse symbols row: {e}"))?;
        if let Some(meta) = record_to_meta(&headers, &record) {
            out.insert(meta.symbol.clone(), meta);
        }
    }

    Ok(out)
}

fn record_to_meta(headers: &StringRecord, record: &StringRecord) -> Option<SymbolMeta> {
    let get = |key: &str| -> String {
        headers
            .iter()
            .position(|h| h.eq_ignore_ascii_case(key))
            .and_then(|idx| record.get(idx))
            .unwrap_or_default()
            .trim_matches('"')
            .trim()
            .to_string()
    };

    let symbol = get("symbol");
    if symbol.is_empty() {
        return None;
    }

    let filters_raw = get("filters");
    let filters = filters_raw
        .split([';', ','])
        .filter_map(|s| {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        })
        .collect::<Vec<_>>();

    let badge = get("badge");
    let market = get("market");
    let venue = get("venue");

    let exchange = if !get("exchange").is_empty() {
        get("exchange")
    } else {
        venue.clone()
    };

    let source = if !get("source").is_empty() {
        get("source")
    } else if headers.len() >= 3 {
        // Legacy format: symbol,name,source,exchange
        headers
            .iter()
            .position(|h| h.eq_ignore_ascii_case("source"))
            .and_then(|idx| record.get(idx))
            .unwrap_or_default()
            .trim()
            .to_string()
    } else {
        String::new()
    };

    Some(SymbolMeta {
        symbol,
        name: get("name"),
        source,
        exchange,
        badge,
        market,
        filters,
        venue,
    })
}
