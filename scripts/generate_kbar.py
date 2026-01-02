import argparse
import csv
import random
from datetime import datetime, timedelta, timezone
from pathlib import Path
from typing import Iterable

UNIVERSE_FIELDS = ["filters", "badge", "symbol", "name", "market", "venue"]
MAPPING_FIELDS = ["symbol", "name", "exchange", "source", "filters", "badge", "market", "venue"]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Generate synthetic OHLCV (kbar) data for every symbol in universe.csv."
    )
    parser.add_argument(
        "--universe",
        type=Path,
        default=Path("data/universe.csv"),
        help="Universe CSV path (default: data/universe.csv).",
    )
    parser.add_argument(
        "--out-dir",
        type=Path,
        default=Path("data/candles"),
        help="Directory to write per-symbol candles (default: data/candles).",
    )
    parser.add_argument(
        "--mapping-output",
        type=Path,
        default=Path("data/mapping.csv"),
        help="Path to write symbol-to-candles mapping (default: data/mapping.csv).",
    )
    parser.add_argument(
        "--length",
        "-n",
        type=int,
        required=True,
        help="Number of rows to generate per symbol.",
    )
    parser.add_argument(
        "--interval-seconds",
        "-i",
        type=int,
        default=60,
        help="Seconds between candles (default: 60).",
    )
    parser.add_argument(
        "--seed",
        type=int,
        default=None,
        help="Optional random seed for reproducible output.",
    )
    return parser.parse_args()


def ensure_dir(path: Path) -> Path:
    path.mkdir(parents=True, exist_ok=True)
    return path


def read_universe(path: Path) -> list[dict]:
    if not path.exists():
        raise SystemExit(f"universe file not found: {path}")
    rows = []
    with path.open("r", newline="", encoding="utf-8") as f:
        reader = csv.DictReader(f)
        missing = [col for col in UNIVERSE_FIELDS if col not in reader.fieldnames]
        if missing:
            raise SystemExit(f"universe is missing columns: {', '.join(missing)}")
        for row in reader:
            if not row.get("symbol"):
                continue
            rows.append(row)
    if not rows:
        raise SystemExit(f"no symbols found in universe: {path}")
    return rows


def unique_symbols(universe_rows: Iterable[dict]) -> dict[str, dict]:
    deduped: dict[str, dict] = {}
    for row in universe_rows:
        symbol = row["symbol"].strip()
        if not symbol:
            continue
        deduped.setdefault(symbol, row)
    return deduped


def generate_rows(length: int, interval_seconds: int, rng: random.Random) -> list[dict]:
    start_time = datetime.now(timezone.utc) - timedelta(seconds=interval_seconds * length)
    price = 100.0
    rows = []

    for i in range(length):
        ts = start_time + timedelta(seconds=interval_seconds * i)
        drift = rng.uniform(-0.02, 0.02)
        open_price = price
        close_price = max(0.01, open_price * (1 + drift))

        span = open_price * rng.uniform(0.001, 0.01)
        high = max(open_price, close_price) + span
        low = max(0.01, min(open_price, close_price) - span)

        volume = rng.uniform(50.0, 5_000.0)

        rows.append(
            {
                "timestamp": ts.isoformat().replace("+00:00", "Z"),
                "open": f"{open_price:.6f}",
                "high": f"{high:.6f}",
                "low": f"{low:.6f}",
                "close": f"{close_price:.6f}",
                "volume": f"{volume:.2f}",
            }
        )
        price = close_price

    return rows


def write_candles(path: Path, rows: list[dict]) -> None:
    ensure_dir(path.parent)
    fieldnames = ["timestamp", "open", "high", "low", "close", "volume"]
    with path.open("w", newline="", encoding="utf-8") as f:
        writer = csv.DictWriter(f, fieldnames=fieldnames)
        writer.writeheader()
        writer.writerows(rows)


def write_mapping(path: Path, rows: list[dict]) -> None:
    ensure_dir(path.parent)
    with path.open("w", newline="", encoding="utf-8") as f:
        writer = csv.DictWriter(f, fieldnames=MAPPING_FIELDS)
        writer.writeheader()
        writer.writerows(rows)


def mapping_path_for(output_file: Path) -> str:
    if output_file.is_absolute():
        return str(output_file)
    # UI crate resolves paths relative to its manifest dir, so prefix with .. from repo root.
    return str(Path("..") / output_file).replace("\\", "/")


def main() -> None:
    args = parse_args()
    if args.length <= 0:
        raise SystemExit("length must be positive")
    if args.interval_seconds <= 0:
        raise SystemExit("interval-seconds must be positive")

    rng = random.Random(args.seed)

    universe_rows = read_universe(args.universe)
    symbols = unique_symbols(universe_rows)
    mapping_rows = []
    for idx, (symbol, meta) in enumerate(symbols.items(), start=1):
        output_file = args.out_dir / f"{symbol}.csv"
        candles = generate_rows(args.length, args.interval_seconds, rng)
        write_candles(output_file, candles)
        mapping_rows.append(
            {
                "symbol": symbol,
                "name": meta.get("name", ""),
                "exchange": meta.get("venue", "") or meta.get("exchange", ""),
                "source": mapping_path_for(output_file),
                "filters": meta.get("filters", ""),
                "badge": meta.get("badge", ""),
                "market": meta.get("market", ""),
                "venue": meta.get("venue", ""),
            }
        )
        print(f"[{idx}/{len(symbols)}] Wrote {len(candles)} rows for {symbol} -> {output_file}")

    write_mapping(args.mapping_output, mapping_rows)
    print(f"Wrote mapping for {len(mapping_rows)} symbols to {args.mapping_output}")


if __name__ == "__main__":
    main()
