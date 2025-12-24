import argparse
import csv
import random
from datetime import datetime, timedelta, timezone
from pathlib import Path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Generate synthetic OHLCV (kbar) data and write it to a CSV file."
    )
    parser.add_argument(
        "--length",
        "-n",
        type=int,
        required=True,
        help="Number of rows to generate.",
    )
    parser.add_argument(
        "--output",
        "-o",
        type=Path,
        default=Path("data/random_kbar.csv"),
        help="Output CSV path (default: data/random_kbar.csv).",
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


def ensure_output_path(path: Path) -> Path:
    if not path.parent.exists():
        path.parent.mkdir(parents=True, exist_ok=True)
    return path


def generate_rows(length: int, interval_seconds: int) -> list[dict]:
    start_time = datetime.now(timezone.utc) - timedelta(seconds=interval_seconds * length)
    price = 100.0
    rows = []

    for i in range(length):
        ts = start_time + timedelta(seconds=interval_seconds * i)
        drift = random.uniform(-0.02, 0.02)
        open_price = price
        close_price = max(0.01, open_price * (1 + drift))

        span = open_price * random.uniform(0.001, 0.01)
        high = max(open_price, close_price) + span
        low = max(0.01, min(open_price, close_price) - span)

        volume = random.uniform(50.0, 5_000.0)

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


def write_csv(path: Path, rows: list[dict]) -> None:
    fieldnames = ["timestamp", "open", "high", "low", "close", "volume"]
    with path.open("w", newline="", encoding="utf-8") as f:
        writer = csv.DictWriter(f, fieldnames=fieldnames)
        writer.writeheader()
        writer.writerows(rows)


def main() -> None:
    args = parse_args()
    if args.length <= 0:
        raise SystemExit("length must be positive")
    if args.interval_seconds <= 0:
        raise SystemExit("interval-seconds must be positive")

    if args.seed is not None:
        random.seed(args.seed)

    output_path = ensure_output_path(args.output)
    rows = generate_rows(args.length, args.interval_seconds)
    write_csv(output_path, rows)
    print(f"Wrote {len(rows)} rows to {output_path}")


if __name__ == "__main__":
    main()
