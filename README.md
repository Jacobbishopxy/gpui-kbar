# GPUI KBar

- generate kbar sample data: `uv run scripts/generate_kbar.py -n 500 -o data/sample.csv -i 300 --seed 42`
- run debug: `cargo run -p app --bin debug .\data\sample.csv`
- run runtime app: `cargo run -p app --bin runtime`
