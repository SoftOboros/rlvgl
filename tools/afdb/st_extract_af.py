#!/usr/bin/env python3
"""Convert a CSV of STM32 pin functions to a compact AF JSON database.

The CSV is expected to have columns: mcu,pin,signal,af. Each row describes
that `pin` can serve `signal` with alternate-function number `af` on `mcu`.

Usage:
  st_extract_af.py --db pins.csv --out af.json

The output JSON structure is nested:
{
  "<MCU>": {
    "<PIN>": { "<SIGNAL>": <AF>, ... },
    ...
  },
  ...
}
"""

import argparse
import csv
import json
from typing import Dict


def main() -> None:
    parser = argparse.ArgumentParser(description="Build STM32 AF JSON database from CSV")
    parser.add_argument("--db", required=True, help="Input CSV extracted from CubeMX DB")
    parser.add_argument("--out", required=True, help="Output JSON path")
    args = parser.parse_args()

    db: Dict[str, Dict[str, Dict[str, int]]] = {}
    with open(args.db, newline="") as f:
        reader = csv.DictReader(f)
        for row in reader:
            mcu = row["mcu"].strip()
            pin = row["pin"].strip()
            signal = row["signal"].strip()
            af = int(row["af"].strip())
            db.setdefault(mcu, {}).setdefault(pin, {})[signal] = af

    with open(args.out, "w") as f:
        json.dump(db, f, indent=2, sort_keys=True)

    print(f"Wrote AF DB → {args.out}")


if __name__ == "__main__":
    main()
