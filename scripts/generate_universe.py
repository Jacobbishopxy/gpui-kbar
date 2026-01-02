from __future__ import annotations

import argparse
import csv
import random
from pathlib import Path
from typing import Iterable

# Filters correspond to the chips in ui/src/chart/view/overlays/symbol_search.rs.
FIELDNAMES = ["filters", "badge", "symbol", "name", "market", "venue"]

INSTRUMENTS = [
    {
        "filters": ["Indices"],
        "badge": "100",
        "symbol": "NDQ",
        "name": "US 100 Index",
        "market": "index cfd",
        "venue": "TVC",
    },
    {
        "filters": ["Funds"],
        "badge": "ETF",
        "symbol": "NDQ",
        "name": "BetaShares NASDAQ 100 ETF",
        "market": "fund etf",
        "venue": "ASX",
    },
    {
        "filters": ["Funds"],
        "badge": "ETF",
        "symbol": "NDQ",
        "name": "Invesco QQQ Trust Series I",
        "market": "fund etf",
        "venue": "TRADEGATE",
    },
    {
        "filters": ["Funds"],
        "badge": "ETF",
        "symbol": "NDQ",
        "name": "Invesco QQQ Trust Series I",
        "market": "fund etf",
        "venue": "BER",
    },
    {
        "filters": ["Funds"],
        "badge": "ETF",
        "symbol": "NDQ",
        "name": "Invesco QQQ Trust Series I",
        "market": "fund etf",
        "venue": "HAM",
    },
    {
        "filters": ["Indices"],
        "badge": "100",
        "symbol": "NDQM",
        "name": "NASDAQ 100 Index (NDX)",
        "market": "index cfd",
        "venue": "FXOpen",
    },
    {
        "filters": ["Funds"],
        "badge": "CASH",
        "symbol": "NDQ100",
        "name": "Nasdaq Cash",
        "market": "index cfd",
        "venue": "Eightcap",
    },
    {
        "filters": ["Options"],
        "badge": "CW",
        "symbol": "NDQCC",
        "name": "Cititwarrants 36.2423 NDQ 07-Jun-35 Instal Mini",
        "market": "warrant",
        "venue": "CHIXAU",
    },
    {
        "filters": ["Crypto"],
        "badge": "CR",
        "symbol": "NDQUSD",
        "name": "Nasdaq666",
        "market": "spot crypto",
        "venue": "CRYPTO",
    },
    {
        "filters": ["Funds"],
        "badge": "3L",
        "symbol": "NDQ3L",
        "name": "SG Issuer SA Exchange Traded Product 2022-03-18",
        "market": "fund etf",
        "venue": "Euronext Paris",
    },
    {
        "filters": ["Funds"],
        "badge": "3S",
        "symbol": "NDQ3S",
        "name": "SG Issuer SA War 2022- Without fixed mat on ...",
        "market": "fund etf",
        "venue": "Euronext Paris",
    },
    {
        "filters": ["Forex", "Indices"],
        "badge": "USD",
        "symbol": "NDQUSD",
        "name": "US Tech (NDQ) / US Dollar",
        "market": "index cfd",
        "venue": "easyMarkets",
    },
    {
        "filters": ["Indices"],
        "badge": "SPX",
        "symbol": "SPX",
        "name": "S&P 500 Index",
        "market": "index",
        "venue": "CBOE",
    },
    {
        "filters": ["Indices"],
        "badge": "DJI",
        "symbol": "DJI",
        "name": "Dow Jones Industrial Average",
        "market": "index",
        "venue": "INDEXDJX",
    },
    {
        "filters": ["Indices"],
        "badge": "DAX",
        "symbol": "DAX",
        "name": "Germany 40 Index",
        "market": "index",
        "venue": "XETRA",
    },
    {
        "filters": ["Funds"],
        "badge": "ETF",
        "symbol": "SPY",
        "name": "SPDR S&P 500 ETF Trust",
        "market": "fund etf",
        "venue": "NYSE Arca",
    },
    {
        "filters": ["Funds"],
        "badge": "ETF",
        "symbol": "QQQ",
        "name": "Invesco QQQ Trust Series I",
        "market": "fund etf",
        "venue": "NASDAQ",
    },
    {
        "filters": ["Funds"],
        "badge": "ETF",
        "symbol": "VOO",
        "name": "Vanguard S&P 500 ETF",
        "market": "fund etf",
        "venue": "NYSE Arca",
    },
    {
        "filters": ["Stocks"],
        "badge": "STK",
        "symbol": "AAPL",
        "name": "Apple Inc.",
        "market": "equity",
        "venue": "NASDAQ",
    },
    {
        "filters": ["Stocks"],
        "badge": "STK",
        "symbol": "TSLA",
        "name": "Tesla, Inc.",
        "market": "equity",
        "venue": "NASDAQ",
    },
    {
        "filters": ["Stocks"],
        "badge": "STK",
        "symbol": "NFLX",
        "name": "Netflix, Inc.",
        "market": "equity",
        "venue": "NASDAQ",
    },
    {
        "filters": ["Stocks"],
        "badge": "STK",
        "symbol": "MSFT",
        "name": "Microsoft Corporation",
        "market": "equity",
        "venue": "NASDAQ",
    },
    {
        "filters": ["Stocks"],
        "badge": "STK",
        "symbol": "NVDA",
        "name": "NVIDIA Corporation",
        "market": "equity",
        "venue": "NASDAQ",
    },
    {
        "filters": ["Futures"],
        "badge": "FUT",
        "symbol": "ES1!",
        "name": "E-mini S&P 500 Futures",
        "market": "futures",
        "venue": "CME",
    },
    {
        "filters": ["Futures"],
        "badge": "FUT",
        "symbol": "NQ1!",
        "name": "E-mini NASDAQ 100 Futures",
        "market": "futures",
        "venue": "CME",
    },
    {
        "filters": ["Futures"],
        "badge": "FUT",
        "symbol": "CL1!",
        "name": "Crude Oil WTI Futures",
        "market": "futures",
        "venue": "NYMEX",
    },
    {
        "filters": ["Futures"],
        "badge": "FUT",
        "symbol": "GC1!",
        "name": "Gold Futures",
        "market": "futures",
        "venue": "COMEX",
    },
    {
        "filters": ["Futures"],
        "badge": "FUT",
        "symbol": "USOIL",
        "name": "WTI Crude Oil Spot",
        "market": "energy cfd",
        "venue": "TVC",
    },
    {
        "filters": ["Forex"],
        "badge": "FX",
        "symbol": "EURUSD",
        "name": "Euro / US Dollar",
        "market": "forex",
        "venue": "FX",
    },
    {
        "filters": ["Forex"],
        "badge": "FX",
        "symbol": "USDJPY",
        "name": "US Dollar / Japanese Yen",
        "market": "forex",
        "venue": "FX",
    },
    {
        "filters": ["Forex"],
        "badge": "FX",
        "symbol": "GBPUSD",
        "name": "British Pound / US Dollar",
        "market": "forex",
        "venue": "FX",
    },
    {
        "filters": ["Forex"],
        "badge": "FX",
        "symbol": "AUDUSD",
        "name": "Australian Dollar / US Dollar",
        "market": "forex",
        "venue": "FX",
    },
    {
        "filters": ["Crypto"],
        "badge": "CR",
        "symbol": "BTCUSD",
        "name": "Bitcoin / US Dollar",
        "market": "spot crypto",
        "venue": "CRYPTO",
    },
    {
        "filters": ["Crypto"],
        "badge": "CR",
        "symbol": "ETHUSD",
        "name": "Ethereum / US Dollar",
        "market": "spot crypto",
        "venue": "CRYPTO",
    },
    {
        "filters": ["Crypto"],
        "badge": "CR",
        "symbol": "SOLUSD",
        "name": "Solana / US Dollar",
        "market": "spot crypto",
        "venue": "CRYPTO",
    },
    {
        "filters": ["Bonds"],
        "badge": "BND",
        "symbol": "US10Y",
        "name": "US 10 Year Treasury Yield",
        "market": "bond yield",
        "venue": "TVC",
    },
    {
        "filters": ["Bonds"],
        "badge": "BND",
        "symbol": "US02Y",
        "name": "US 2 Year Treasury Yield",
        "market": "bond yield",
        "venue": "TVC",
    },
    {
        "filters": ["Economy"],
        "badge": "ECO",
        "symbol": "USGDP",
        "name": "United States GDP QoQ",
        "market": "economy",
        "venue": "FRED",
    },
    {
        "filters": ["Economy"],
        "badge": "ECO",
        "symbol": "USCPI",
        "name": "United States CPI YoY",
        "market": "economy",
        "venue": "FRED",
    },
    {
        "filters": ["Options"],
        "badge": "OPT",
        "symbol": "AAPL250117C00200000",
        "name": "AAPL 17-Jan-2025 200 Call",
        "market": "equity option",
        "venue": "OPRA",
    },
    {
        "filters": ["Options"],
        "badge": "OPT",
        "symbol": "TSLA250117P00150000",
        "name": "TSLA 17-Jan-2025 150 Put",
        "market": "equity option",
        "venue": "OPRA",
    },
]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Generate a symbol search universe CSV used by the overlay demo."
    )
    parser.add_argument(
        "--output",
        "-o",
        type=Path,
        default=Path("data/universe.csv"),
        help="Path to write the CSV (default: data/universe.csv).",
    )
    parser.add_argument(
        "--limit",
        type=int,
        default=None,
        help="Optional max rows to write after ordering/shuffling.",
    )
    parser.add_argument(
        "--shuffle",
        action="store_true",
        help="Shuffle rows instead of sorting by filter/symbol/venue.",
    )
    parser.add_argument(
        "--seed",
        type=int,
        default=None,
        help="Random seed used when --shuffle is set.",
    )
    return parser.parse_args()


def normalize_filters(filters: Iterable[str]) -> str:
    seen: list[str] = []
    for raw in filters:
        value = raw.strip()
        if value and value not in seen:
            seen.append(value)
    return ";".join(seen)


def build_rows() -> list[dict[str, str]]:
    rows: list[dict[str, str]] = []
    for instrument in INSTRUMENTS:
        rows.append(
            {
                "filters": normalize_filters(instrument["filters"]),
                "badge": instrument["badge"],
                "symbol": instrument["symbol"],
                "name": instrument["name"],
                "market": instrument["market"],
                "venue": instrument["venue"],
            }
        )
    return rows


def order_rows(
    rows: list[dict[str, str]], shuffle: bool, seed: int | None
) -> list[dict[str, str]]:
    ordered = rows.copy()
    if shuffle:
        rng = random.Random(seed)
        rng.shuffle(ordered)
    else:
        ordered.sort(key=lambda r: (r["filters"], r["symbol"], r["venue"], r["badge"]))
    return ordered


def limit_rows(rows: list[dict[str, str]], limit: int | None) -> list[dict[str, str]]:
    if limit is None:
        return rows
    if limit <= 0:
        raise SystemExit("limit must be positive")
    return rows[:limit]


def ensure_output_path(path: Path) -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    return path


def write_csv(path: Path, rows: Iterable[dict[str, str]]) -> None:
    with path.open("w", newline="", encoding="utf-8") as f:
        writer = csv.DictWriter(f, fieldnames=FIELDNAMES)
        writer.writeheader()
        writer.writerows(rows)


def summarize(rows: Iterable[dict[str, str]]) -> str:
    counts: dict[str, int] = {}
    for row in rows:
        for flt in row["filters"].split(";"):
            if not flt:
                continue
            counts[flt] = counts.get(flt, 0) + 1
    return ", ".join(f"{key}:{counts[key]}" for key in sorted(counts))


def main() -> None:
    args = parse_args()
    rows = build_rows()
    ordered = order_rows(rows, args.shuffle, args.seed)
    limited = limit_rows(ordered, args.limit)
    output_path = ensure_output_path(args.output)
    write_csv(output_path, limited)
    summary = summarize(limited)
    print(f"Wrote {len(limited)} rows to {output_path} ({summary})")


if __name__ == "__main__":
    main()
