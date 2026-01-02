use std::{collections::HashMap, fs};

#[derive(Clone, Debug)]
pub struct SymbolMeta {
    pub symbol: String,
    pub name: String,
    pub source: String,
    pub exchange: String,
}

pub fn load_symbols(path: &str) -> Result<HashMap<String, SymbolMeta>, String> {
    let contents =
        fs::read_to_string(path).map_err(|e| format!("failed to read symbols csv: {e}"))?;
    let mut out = HashMap::new();
    for line in contents.lines().skip(1) {
        let mut parts = line.split(',');
        let sym = parts.next().unwrap_or_default().trim();
        if sym.is_empty() {
            continue;
        }
        let name = parts.next().unwrap_or_default().trim().to_string();
        let source = parts.next().unwrap_or_default().trim().to_string();
        let exchange = parts.next().unwrap_or_default().trim().to_string();

        out.insert(
            sym.to_string(),
            SymbolMeta {
                symbol: sym.to_string(),
                name,
                source,
                exchange,
            },
        );
    }

    Ok(out)
}
